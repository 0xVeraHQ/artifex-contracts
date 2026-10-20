#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Platform,
    Usdc,
    XlmFee,
    UsdcFee,
    XlmBal,
    UsdcBal,
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

    pub fn set_fee(env: Env, caller: Address, xlm_fee: i128, usdc_fee: i128) {
        caller.require_auth();
        Self::require_platform(&env, &caller);
        if xlm_fee <= 0 || usdc_fee <= 0 {
            panic!("fees must be positive");
        }
        env.storage().instance().set(&DataKey::XlmFee, &xlm_fee);
        env.storage().instance().set(&DataKey::UsdcFee, &usdc_fee);
    }

    /// Collect native XLM via SAC (`xlm` = native SAC address).
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

    fn require_platform(env: &Env, caller: &Address) {
        let p: Address = env.storage().instance().get(&DataKey::Platform).unwrap();
        if caller != &p {
            panic!("platform only");
        }
    }
}

mod test;
