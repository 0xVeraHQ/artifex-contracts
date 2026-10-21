#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env,
};

#[contracttype]
#[derive(Clone)]
pub struct PieceDeposit {
    pub payer: Address,
    pub token: Address,
    pub amount: i128,
    pub consumed: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Platform,
    Usdc,
    XlmFee,
    UsdcFee,
    XlmBal,
    UsdcBal,
    /// Optional cert registry allowed to consume piece deposits.
    CertRegistry,
    Deposit(BytesN<32>),
    Inited,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    /// `xlm_fee` stroops (default demo 5 XLM = 50_000_000), `usdc_fee` in USDC base units (1e7 = 1 USDC).
    pub fn init(env: Env, platform: Address, usdc: Address, xlm_fee: i128, usdc_fee: i128) {
        if env.storage().instance().has(&DataKey::Inited) {
            panic!("already initialized");
        }
        platform.require_auth();
        if xlm_fee <= 0 || usdc_fee <= 0 {
            panic!("fees must be positive");
        }
        env.storage().instance().set(&DataKey::Platform, &platform);
        env.storage().instance().set(&DataKey::Usdc, &usdc);
        env.storage().instance().set(&DataKey::XlmFee, &xlm_fee);
        env.storage().instance().set(&DataKey::UsdcFee, &usdc_fee);
        env.storage().instance().set(&DataKey::XlmBal, &0i128);
        env.storage().instance().set(&DataKey::UsdcBal, &0i128);
        env.storage().instance().set(&DataKey::Inited, &true);
    }

    /// Wire cert registry (v2) so finalize can consume piece-scoped deposits.
    pub fn set_cert_registry(env: Env, caller: Address, cert_registry: Address) {
        caller.require_auth();
        Self::require_platform(&env, &caller);
        env.storage()
            .instance()
            .set(&DataKey::CertRegistry, &cert_registry);
    }

    pub fn set_fee(env: Env, caller: Address, xlm_fee: i128, usdc_fee: i128) {
        caller.require_auth();
        Self::require_platform(&env, &caller);
        if xlm_fee <= 0 || usdc_fee <= 0 {
            panic!("fees must be positive");
        }
        env.storage().instance().set(&DataKey::XlmFee, &xlm_fee);
        env.storage().instance().set(&DataKey::UsdcFee, &usdc_fee);
    }

    /// Legacy unscoped collect (not refundable / not piece-linked). Prefer `deposit_*`.
    pub fn collect_xlm(env: Env, payer: Address, xlm: Address) {
        payer.require_auth();
        let fee: i128 = env.storage().instance().get(&DataKey::XlmFee).unwrap();
        let client = token::Client::new(&env, &xlm);
        client.transfer(&payer, &env.current_contract_address(), &fee);
        let bal: i128 = env.storage().instance().get(&DataKey::XlmBal).unwrap_or(0);
        env.storage().instance().set(&DataKey::XlmBal, &(bal + fee));
        env.events()
            .publish((symbol_short!("fee_xlm"), payer), fee);
    }

    /// Legacy unscoped collect (not refundable / not piece-linked). Prefer `deposit_*`.
    pub fn collect_usdc(env: Env, payer: Address) {
        payer.require_auth();
        let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
        let fee: i128 = env.storage().instance().get(&DataKey::UsdcFee).unwrap();
        let client = token::Client::new(&env, &usdc);
        client.transfer(&payer, &env.current_contract_address(), &fee);
        let bal: i128 = env.storage().instance().get(&DataKey::UsdcBal).unwrap_or(0);
        env.storage().instance().set(&DataKey::UsdcBal, &(bal + fee));
        env.events()
            .publish((symbol_short!("fee_usdc"), payer), fee);
    }

    /// Piece-scoped XLM fee escrow — refundable until consumed by cert finalize.
    pub fn deposit_xlm(env: Env, payer: Address, xlm: Address, piece_id: BytesN<32>) {
        payer.require_auth();
        Self::deposit_token(
            &env,
            payer,
            xlm,
            env.storage().instance().get(&DataKey::XlmFee).unwrap(),
            piece_id,
            true,
        );
    }

    /// Piece-scoped USDC fee escrow — refundable until consumed by cert finalize.
    pub fn deposit_usdc(env: Env, payer: Address, piece_id: BytesN<32>) {
        payer.require_auth();
        let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
        let fee: i128 = env.storage().instance().get(&DataKey::UsdcFee).unwrap();
        Self::deposit_token(&env, payer, usdc, fee, piece_id, false);
    }

    fn deposit_token(
        env: &Env,
        payer: Address,
        token_addr: Address,
        fee: i128,
        piece_id: BytesN<32>,
        is_xlm: bool,
    ) {
        if env
            .storage()
            .persistent()
            .has(&DataKey::Deposit(piece_id.clone()))
        {
            let existing: PieceDeposit = env
                .storage()
                .persistent()
                .get(&DataKey::Deposit(piece_id.clone()))
                .unwrap();
            if !existing.consumed {
                panic!("deposit exists");
            }
        }
        let client = token::Client::new(env, &token_addr);
        client.transfer(&payer, &env.current_contract_address(), &fee);
        if is_xlm {
            let bal: i128 = env.storage().instance().get(&DataKey::XlmBal).unwrap_or(0);
            env.storage().instance().set(&DataKey::XlmBal, &(bal + fee));
        } else {
            let bal: i128 = env.storage().instance().get(&DataKey::UsdcBal).unwrap_or(0);
            env.storage().instance().set(&DataKey::UsdcBal, &(bal + fee));
        }
        let dep = PieceDeposit {
            payer: payer.clone(),
            token: token_addr,
            amount: fee,
            consumed: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Deposit(piece_id.clone()), &dep);
        env.events()
            .publish((symbol_short!("fee_dep"), payer), (piece_id, fee));
    }

    /// Refund unconsumed piece deposit to original payer.
    pub fn refund(env: Env, caller: Address, piece_id: BytesN<32>) {
        caller.require_auth();
        let mut dep: PieceDeposit = env
            .storage()
            .persistent()
            .get(&DataKey::Deposit(piece_id.clone()))
            .expect("no deposit");
        if dep.consumed {
            panic!("consumed");
        }
        if caller != dep.payer {
            let platform: Address = env.storage().instance().get(&DataKey::Platform).unwrap();
            if caller != platform {
                panic!("unauthorized");
            }
        }
        let amount = dep.amount;
        let token_addr = dep.token.clone();
        let payer = dep.payer.clone();
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &payer, &amount);
        Self::debit_bal(&env, &token_addr, amount);
        dep.consumed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Deposit(piece_id.clone()), &dep);
        env.events()
            .publish((symbol_short!("fee_rfnd"), payer), (piece_id, amount));
    }

    /// Mark piece deposit consumed (funds stay in collector). Cert registry or platform only.
    pub fn consume(env: Env, caller: Address, piece_id: BytesN<32>) {
        caller.require_auth();
        let platform: Address = env.storage().instance().get(&DataKey::Platform).unwrap();
        let registry: Option<Address> = env.storage().instance().get(&DataKey::CertRegistry);
        let ok = caller == platform || registry.as_ref() == Some(&caller);
        if !ok {
            panic!("unauthorized");
        }
        let mut dep: PieceDeposit = env
            .storage()
            .persistent()
            .get(&DataKey::Deposit(piece_id.clone()))
            .expect("no deposit");
        if dep.consumed {
            panic!("consumed");
        }
        dep.consumed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Deposit(piece_id.clone()), &dep);
        env.events()
            .publish((symbol_short!("fee_use"), piece_id), dep.amount);
    }

    pub fn has_deposit(env: Env, piece_id: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Deposit(piece_id))
            .map(|d: PieceDeposit| !d.consumed)
            .unwrap_or(false)
    }

    pub fn get_deposit(env: Env, piece_id: BytesN<32>) -> PieceDeposit {
        env.storage()
            .persistent()
            .get(&DataKey::Deposit(piece_id))
            .expect("no deposit")
    }

    /// Documented platform withdraw.
    pub fn withdraw(env: Env, caller: Address, token_addr: Address, amount: i128) {
        caller.require_auth();
        Self::require_platform(&env, &caller);
        if amount <= 0 {
            panic!("amount");
        }
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &caller, &amount);
        env.events()
            .publish((symbol_short!("fee_wdraw"), caller), amount);
    }

    pub fn balances(env: Env) -> (i128, i128) {
        let x: i128 = env.storage().instance().get(&DataKey::XlmBal).unwrap_or(0);
        let u: i128 = env.storage().instance().get(&DataKey::UsdcBal).unwrap_or(0);
        (x, u)
    }

    pub fn fees(env: Env) -> (i128, i128) {
        (
            env.storage().instance().get(&DataKey::XlmFee).unwrap(),
            env.storage().instance().get(&DataKey::UsdcFee).unwrap(),
        )
    }

    fn debit_bal(env: &Env, token_addr: &Address, amount: i128) {
        let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
        if token_addr == &usdc {
            let bal: i128 = env.storage().instance().get(&DataKey::UsdcBal).unwrap_or(0);
            if bal < amount {
                panic!("bal");
            }
            env.storage()
                .instance()
                .set(&DataKey::UsdcBal, &(bal - amount));
        } else {
            let bal: i128 = env.storage().instance().get(&DataKey::XlmBal).unwrap_or(0);
            if bal < amount {
                panic!("bal");
            }
            env.storage()
                .instance()
                .set(&DataKey::XlmBal, &(bal - amount));
        }
    }

    fn require_platform(env: &Env, caller: &Address) {
        let p: Address = env.storage().instance().get(&DataKey::Platform).unwrap();
        if caller != &p {
            panic!("platform only");
        }
    }
}

mod test;
