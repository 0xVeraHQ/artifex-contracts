#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env,
};

pub const ASSET_XLM: u32 = 1;
pub const ASSET_USDC: u32 = 2;

#[contracttype]
#[derive(Clone)]
pub struct Deposit {
    pub payer: Address,
    pub asset: u32,
    pub amount: i128,
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
    /// Cert registry (or other) allowed to consume piece-scoped deposits.
    Consumer,
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

    /// Bind cert registry (or platform ops contract) that may consume deposits on finalize.
    pub fn set_consumer(env: Env, caller: Address, consumer: Address) {
        caller.require_auth();
        Self::require_platform(&env, &caller);
        env.storage().instance().set(&DataKey::Consumer, &consumer);
    }

    /// Read the address allowed to `consume_deposit` (normally the active cert_registry).
    pub fn get_consumer(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Consumer)
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

    /// Unscoped collect (legacy). Prefer `deposit_xlm` so fees are refundable until finalize.
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

    /// Escrow XLM fee for a piece. Refundable until consumed by finalize.
    pub fn deposit_xlm(env: Env, payer: Address, xlm: Address, piece_id: BytesN<32>) {
        payer.require_auth();
        if env
            .storage()
            .persistent()
            .has(&DataKey::Deposit(piece_id.clone()))
        {
            panic!("deposit exists");
        }
        let fee: i128 = env.storage().instance().get(&DataKey::XlmFee).unwrap();
        let client = token::Client::new(&env, &xlm);
        client.transfer(&payer, &env.current_contract_address(), &fee);
        let bal: i128 = env.storage().instance().get(&DataKey::XlmBal).unwrap_or(0);
        env.storage().instance().set(&DataKey::XlmBal, &(bal + fee));
        let dep = Deposit {
            payer: payer.clone(),
            asset: ASSET_XLM,
            amount: fee,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Deposit(piece_id.clone()), &dep);
        env.events()
            .publish((symbol_short!("dep_xlm"), payer), piece_id);
    }

    /// Escrow USDC fee for a piece. Refundable until consumed by finalize.
    pub fn deposit_usdc(env: Env, payer: Address, piece_id: BytesN<32>) {
        payer.require_auth();
        if env
            .storage()
            .persistent()
            .has(&DataKey::Deposit(piece_id.clone()))
        {
            panic!("deposit exists");
        }
        let usdc: Address = env.storage().instance().get(&DataKey::Usdc).unwrap();
        let fee: i128 = env.storage().instance().get(&DataKey::UsdcFee).unwrap();
        let client = token::Client::new(&env, &usdc);
        client.transfer(&payer, &env.current_contract_address(), &fee);
        let bal: i128 = env.storage().instance().get(&DataKey::UsdcBal).unwrap_or(0);
        env.storage().instance().set(&DataKey::UsdcBal, &(bal + fee));
        let dep = Deposit {
            payer: payer.clone(),
            asset: ASSET_USDC,
            amount: fee,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Deposit(piece_id.clone()), &dep);
        env.events()
            .publish((symbol_short!("dep_usdc"), payer), piece_id);
    }

    pub fn has_deposit(env: Env, piece_id: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Deposit(piece_id))
    }

    pub fn get_deposit(env: Env, piece_id: BytesN<32>) -> Deposit {
        env.storage()
            .persistent()
            .get(&DataKey::Deposit(piece_id))
            .expect("no deposit")
    }

    /// Consume escrow on successful finalize. Caller must be the bound consumer (cert registry).
    pub fn consume_deposit(env: Env, piece_id: BytesN<32>) {
        let consumer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Consumer)
            .expect("no consumer");
        consumer.require_auth();
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Deposit(piece_id.clone()))
        {
            panic!("no deposit");
        }
        env.storage()
            .persistent()
            .remove(&DataKey::Deposit(piece_id.clone()));
        env.events()
            .publish((symbol_short!("dep_take"), piece_id), true);
    }

    /// Refund escrow to original payer. Allowed for payer or platform while deposit exists.
    pub fn refund_deposit(env: Env, caller: Address, piece_id: BytesN<32>, xlm: Address) {
        caller.require_auth();
        let dep: Deposit = env
            .storage()
            .persistent()
            .get(&DataKey::Deposit(piece_id.clone()))
            .expect("no deposit");
        let platform: Address = env.storage().instance().get(&DataKey::Platform).unwrap();
        if caller != dep.payer && caller != platform {
            panic!("unauthorized");
        }
        let token_addr = if dep.asset == ASSET_XLM {
            xlm
        } else if dep.asset == ASSET_USDC {
            env.storage().instance().get(&DataKey::Usdc).unwrap()
        } else {
            panic!("bad asset");
        };
        let client = token::Client::new(&env, &token_addr);
        client.transfer(&env.current_contract_address(), &dep.payer, &dep.amount);
        if dep.asset == ASSET_XLM {
            let bal: i128 = env.storage().instance().get(&DataKey::XlmBal).unwrap_or(0);
            env.storage()
                .instance()
                .set(&DataKey::XlmBal, &(bal - dep.amount));
        } else {
            let bal: i128 = env.storage().instance().get(&DataKey::UsdcBal).unwrap_or(0);
            env.storage()
                .instance()
                .set(&DataKey::UsdcBal, &(bal - dep.amount));
        }
        env.storage()
            .persistent()
            .remove(&DataKey::Deposit(piece_id.clone()));
        env.events()
            .publish((symbol_short!("dep_ref"), dep.payer), piece_id);
    }

    /// Documented platform withdraw of consumed (non-escrow) balances.
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

    fn require_platform(env: &Env, caller: &Address) {
        let p: Address = env.storage().instance().get(&DataKey::Platform).unwrap();
        if caller != &p {
            panic!("platform only");
        }
    }
}

mod test;
