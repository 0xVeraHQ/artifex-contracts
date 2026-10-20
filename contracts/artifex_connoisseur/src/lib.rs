#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Auth(BytesN<32>, Address),
    Platform,
    Inited,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn init(env: Env, platform: Address) {
        if env.storage().instance().has(&DataKey::Inited) {
            panic!("already initialized");
        }
        platform.require_auth();
        env.storage().instance().set(&DataKey::Platform, &platform);
        env.storage().instance().set(&DataKey::Inited, &true);
    }

    /// Connoisseur authorizes a piece (`piece_id` = sha256 bytes) for deceased-author cert paths.
    pub fn authorize(env: Env, connoisseur: Address, piece_id: BytesN<32>) {
        connoisseur.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Auth(piece_id.clone(), connoisseur.clone()), &true);
        env.events()
            .publish((symbol_short!("cnn_auth"), connoisseur), piece_id);
    }

    pub fn revoke_auth(env: Env, caller: Address, connoisseur: Address, piece_id: BytesN<32>) {
        caller.require_auth();
        let platform: Address = env.storage().instance().get(&DataKey::Platform).unwrap();
        if caller != connoisseur && caller != platform {
            panic!("unauthorized");
        }
        env.storage()
            .persistent()
            .remove(&DataKey::Auth(piece_id.clone(), connoisseur.clone()));
        env.events()
            .publish((symbol_short!("cnn_rvk"), connoisseur), piece_id);
    }

    pub fn is_authorized(env: Env, connoisseur: Address, piece_id: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Auth(piece_id, connoisseur))
            .unwrap_or(false)
    }
}

mod test;
