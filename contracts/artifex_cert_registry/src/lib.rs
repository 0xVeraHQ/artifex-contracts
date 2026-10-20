#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Vec,
};

/// Certification kinds (Joel table).
pub const CERT_LIVING_AUTHOR: u32 = 1;
pub const CERT_DECEASED_CONNOISSEUR: u32 = 2;
pub const CERT_DECEASED_INSTITUTION: u32 = 3;
pub const CERT_DECEASED_HEIR: u32 = 4;
pub const CERT_PRIOR_DIGITIZATION: u32 = 5;

#[contracttype]
#[derive(Clone)]
pub struct Cert {
    pub piece_id: BytesN<32>,
    pub claim_hash: BytesN<32>,
    pub cert_type: u32,
    pub proposer: Address,
    pub required: Vec<Address>,
    pub signed: Vec<Address>,
    pub fee_paid: bool,
    pub finalized: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Platform,
    Validator,
    FeeCollector,
    Connoisseur,
    Cert(BytesN<32>),
    Inited,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn init(
        env: Env,
        platform: Address,
        validator: Address,
        fee_collector: Address,
        connoisseur: Address,
    ) {
        if env.storage().instance().has(&DataKey::Inited) {
            panic!("already initialized");
        }
        platform.require_auth();
        env.storage().instance().set(&DataKey::Platform, &platform);
        env.storage().instance().set(&DataKey::Validator, &validator);
        env.storage()
            .instance()
            .set(&DataKey::FeeCollector, &fee_collector);
        env.storage()
            .instance()
            .set(&DataKey::Connoisseur, &connoisseur);
        env.storage().instance().set(&DataKey::Inited, &true);
    }

    /// Propose a certificate. `required` must match cert_type rules (enforced lightly for demo).
    pub fn propose_cert(
        env: Env,
        proposer: Address,
        piece_id: BytesN<32>,
        claim_hash: BytesN<32>,
        cert_type: u32,
        required: Vec<Address>,
    ) {
        proposer.require_auth();
        if env.storage().persistent().has(&DataKey::Cert(piece_id.clone())) {
            panic!("cert exists");
        }
        let need = match cert_type {
            CERT_LIVING_AUTHOR => 1u32,
            CERT_DECEASED_CONNOISSEUR
            | CERT_DECEASED_INSTITUTION
            | CERT_DECEASED_HEIR
            | CERT_PRIOR_DIGITIZATION => 2u32,
            _ => panic!("bad cert_type"),
        };
        if required.len() != need {
            panic!("required signer count");
        }
        let signed: Vec<Address> = Vec::new(&env);
        let cert = Cert {
            piece_id: piece_id.clone(),
            claim_hash,
            cert_type,
            proposer: proposer.clone(),
            required,
            signed,
            fee_paid: false,
            finalized: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Cert(piece_id.clone()), &cert);
        env.events()
            .publish((symbol_short!("cert_prop"), proposer), piece_id);
    }

    pub fn cosign(env: Env, signer: Address, piece_id: BytesN<32>) {
        signer.require_auth();
        let mut cert: Cert = env
            .storage()
            .persistent()
            .get(&DataKey::Cert(piece_id.clone()))
            .expect("no cert");
        if cert.finalized {
            panic!("finalized");
        }
        let mut allowed = false;
        for a in cert.required.iter() {
            if a == signer {
                allowed = true;
                break;
            }
        }
        // Platform validator may always co-sign for prior-digitization / backend assist.
        let validator: Address = env.storage().instance().get(&DataKey::Validator).unwrap();
        if signer == validator {
            allowed = true;
        }
        if !allowed {
            panic!("not a required signer");
        }
        for a in cert.signed.iter() {
            if a == signer {
                panic!("already signed");
            }
        }
        cert.signed.push_back(signer.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Cert(piece_id.clone()), &cert);
        env.events()
            .publish((symbol_short!("cert_sign"), signer), piece_id);
    }

    /// Mark fee paid (API/fee_collector integration calls this after collect_* succeeds).
    pub fn mark_fee_paid(env: Env, caller: Address, piece_id: BytesN<32>) {
        caller.require_auth();
        let platform: Address = env.storage().instance().get(&DataKey::Platform).unwrap();
        let fee_c: Address = env.storage().instance().get(&DataKey::FeeCollector).unwrap();
        if caller != platform && caller != fee_c {
            panic!("unauthorized");
        }
        let mut cert: Cert = env
            .storage()
            .persistent()
            .get(&DataKey::Cert(piece_id.clone()))
            .expect("no cert");
        cert.fee_paid = true;
        env.storage()
            .persistent()
            .set(&DataKey::Cert(piece_id.clone()), &cert);
        env.events()
            .publish((symbol_short!("fee_paid"), piece_id), true);
    }

    pub fn finalize_cert(env: Env, caller: Address, piece_id: BytesN<32>) {
        caller.require_auth();
        let mut cert: Cert = env
            .storage()
            .persistent()
            .get(&DataKey::Cert(piece_id.clone()))
            .expect("no cert");
        if cert.finalized {
            panic!("already finalized");
        }
        if !cert.fee_paid {
            panic!("fee required");
        }
        // All required signers present (validator may substitute for prior-digitization).
        for req in cert.required.iter() {
            let mut ok = false;
            for s in cert.signed.iter() {
                if s == req {
                    ok = true;
                    break;
                }
            }
            if !ok {
                panic!("missing signature");
            }
        }
        // Deceased-connoisseur path: at least one required signer must have authorized the piece.
        if cert.cert_type == CERT_DECEASED_CONNOISSEUR {
            let cnn_addr: Address = env.storage().instance().get(&DataKey::Connoisseur).unwrap();
            let cnn = ConnoisseurClient::new(&env, &cnn_addr);
            let mut auth_ok = false;
            for req in cert.required.iter() {
                if cnn.is_authorized(&req, &piece_id) {
                    auth_ok = true;
                    break;
                }
            }
            if !auth_ok {
                panic!("connoisseur not authorized");
            }
        }
        cert.finalized = true;
        env.storage()
            .persistent()
            .set(&DataKey::Cert(piece_id.clone()), &cert);
        env.events()
            .publish((symbol_short!("cert_fin"), caller), piece_id);
    }

    pub fn get_cert(env: Env, piece_id: BytesN<32>) -> Cert {
        env.storage()
            .persistent()
            .get(&DataKey::Cert(piece_id))
            .expect("no cert")
    }

    pub fn is_finalized(env: Env, piece_id: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Cert(piece_id))
            .map(|c: Cert| c.finalized)
            .unwrap_or(false)
    }
}

mod connoisseur_client {
    use super::*;
    use soroban_sdk::contractclient;

    #[contractclient(name = "ConnoisseurClient")]
    pub trait ConnoisseurTrait {
        fn is_authorized(env: Env, connoisseur: Address, piece_id: BytesN<32>) -> bool;
    }
}
use connoisseur_client::*;

mod test;
