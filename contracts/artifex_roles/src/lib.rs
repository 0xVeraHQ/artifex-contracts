#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env};

/// Wallet roles for Artifex Scale (on-chain source of truth).
pub const ROLE_NONE: u32 = 0;
pub const ROLE_ARTIST: u32 = 1;
pub const ROLE_CONNOISSEUR: u32 = 2;
pub const ROLE_HEIR: u32 = 3;
pub const ROLE_MUSEUM: u32 = 4;
pub const ROLE_FOUNDATION: u32 = 5;
pub const ROLE_GALLERY: u32 = 6;
pub const ROLE_PLATFORM: u32 = 7;
pub const ROLE_COLLECTOR: u32 = 8;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Platform,
    Role(Address),
    Inited,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    /// Initialize with platform wallet. Callable once.
    pub fn init(env: Env, platform: Address) {
        if env.storage().instance().has(&DataKey::Inited) {
            panic!("already initialized");
        }
        platform.require_auth();
        env.storage().instance().set(&DataKey::Platform, &platform);
        env.storage()
            .persistent()
            .set(&DataKey::Role(platform.clone()), &ROLE_PLATFORM);
        env.storage().instance().set(&DataKey::Inited, &true);
        env.storage().instance().extend_ttl(1000, 1000);
    }

    /// Platform assigns a role to a wallet. Documented admin surface.
    pub fn assign_role(env: Env, caller: Address, wallet: Address, role: u32) {
        caller.require_auth();
        Self::require_platform(&env, &caller);
        if role > ROLE_COLLECTOR {
            panic!("invalid role");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Role(wallet.clone()), &role);
        env.events()
            .publish((symbol_short!("role_asgn"), wallet), role);
    }

    pub fn revoke_role(env: Env, caller: Address, wallet: Address) {
        caller.require_auth();
        Self::require_platform(&env, &caller);
        env.storage()
            .persistent()
            .set(&DataKey::Role(wallet.clone()), &ROLE_NONE);
        env.events()
            .publish((symbol_short!("role_rvk"), wallet), ROLE_NONE);
    }

    pub fn get_role(env: Env, wallet: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Role(wallet))
            .unwrap_or(ROLE_NONE)
    }

    pub fn platform(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Platform)
            .expect("not initialized")
    }

    fn require_platform(env: &Env, caller: &Address) {
        let p: Address = env
            .storage()
            .instance()
            .get(&DataKey::Platform)
            .expect("not initialized");
        if caller != &p {
            panic!("platform only");
        }
    }
}

mod test;
