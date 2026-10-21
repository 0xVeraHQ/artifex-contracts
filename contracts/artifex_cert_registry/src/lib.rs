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

/// Fee asset for `finalize_with_fee`: 0 = XLM SAC, 1 = USDC.
pub const FEE_ASSET_XLM: u32 = 0;
pub const FEE_ASSET_USDC: u32 = 1;

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
    /// Prefer `finalize_with_fee` (atomic) or piece `deposit_*` + `finalize_cert` (refundable).
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

    /// Finalize when fee already marked or a piece-scoped deposit exists (then consume it).
    pub fn finalize_cert(env: Env, caller: Address, piece_id: BytesN<32>) {
        caller.require_auth();
        let mut cert: Cert = env
            .storage()
            .persistent()
            .get(&DataKey::Cert(piece_id.clone()))
            .expect("no cert");
        Self::assert_ready_to_finalize(&env, &cert, &piece_id);
        if !cert.fee_paid {
            let fee_addr: Address = env.storage().instance().get(&DataKey::FeeCollector).unwrap();
            let fee = FeeCollectorClient::new(&env, &fee_addr);
            if !fee.has_deposit(&piece_id) {
                panic!("fee required");
            }
            // Registry consumes deposit; auth is the contract itself.
            fee.consume(&env.current_contract_address(), &piece_id);
            cert.fee_paid = true;
        }
        cert.finalized = true;
        env.storage()
            .persistent()
            .set(&DataKey::Cert(piece_id.clone()), &cert);
        env.events()
            .publish((symbol_short!("cert_fin"), caller), piece_id);
    }

    /// Atomic path: validate → collect fee → finalize in one invocation.
    /// If finalize would fail, the whole tx reverts and the fee is not charged.
    /// `fee_asset`: 0 = XLM (`xlm` SAC address required), 1 = USDC (`xlm` ignored).
    pub fn finalize_with_fee(
        env: Env,
        caller: Address,
        piece_id: BytesN<32>,
        fee_asset: u32,
        xlm: Address,
    ) {
        caller.require_auth();
        let mut cert: Cert = env
            .storage()
            .persistent()
            .get(&DataKey::Cert(piece_id.clone()))
            .expect("no cert");
        Self::assert_ready_to_finalize(&env, &cert, &piece_id);

        if !cert.fee_paid {
            let fee_addr: Address = env.storage().instance().get(&DataKey::FeeCollector).unwrap();
            let fee = FeeCollectorClient::new(&env, &fee_addr);
            // Prefer consuming an existing piece deposit; else collect in this same tx.
            if fee.has_deposit(&piece_id) {
                fee.consume(&env.current_contract_address(), &piece_id);
            } else if fee_asset == FEE_ASSET_USDC {
                fee.collect_usdc(&caller);
            } else if fee_asset == FEE_ASSET_XLM {
                fee.collect_xlm(&caller, &xlm);
            } else {
                panic!("bad fee_asset");
            }
            cert.fee_paid = true;
        }

        cert.finalized = true;
        env.storage()
            .persistent()
            .set(&DataKey::Cert(piece_id.clone()), &cert);
        env.events()
            .publish((symbol_short!("cert_fin"), caller), piece_id);
    }

    fn assert_ready_to_finalize(env: &Env, cert: &Cert, piece_id: &BytesN<32>) {
        if cert.finalized {
            panic!("already finalized");
        }
        // All required signers must have cosigned (validator may appear in `required`).
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
            let cnn = ConnoisseurClient::new(env, &cnn_addr);
            let mut auth_ok = false;
            for req in cert.required.iter() {
                if cnn.is_authorized(&req, piece_id) {
                    auth_ok = true;
                    break;
                }
            }
            if !auth_ok {
                panic!("connoisseur not authorized");
            }
        }
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

mod fee_collector_client {
    use super::*;
    use soroban_sdk::contractclient;

    #[contractclient(name = "FeeCollectorClient")]
    pub trait FeeCollectorTrait {
        fn collect_xlm(env: Env, payer: Address, xlm: Address);
        fn collect_usdc(env: Env, payer: Address);
        fn has_deposit(env: Env, piece_id: BytesN<32>) -> bool;
        fn consume(env: Env, caller: Address, piece_id: BytesN<32>);
    }
}
use fee_collector_client::*;

mod test;
