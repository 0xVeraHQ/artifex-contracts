#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env, Vec,
};

pub const TYPE_ECONOMIC: u32 = 1;
pub const TYPE_PATRONAGE: u32 = 2;
pub const TYPE_EXHIBITION: u32 = 3;
pub const TYPE_FRACTIONAL_CUSTODY: u32 = 4;

#[contracttype]
#[derive(Clone)]
pub struct DistroPlan {
    pub offer_id: u64,
    pub participation_type: u32,
    pub terms_hash: BytesN<32>,
    pub payment_token: Address,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Participation,
    Plan(u64),
    Inited,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn init(env: Env, admin: Address, participation: Address) {
        if env.storage().instance().has(&DataKey::Inited) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Participation, &participation);
        env.storage().instance().set(&DataKey::Inited, &true);
    }

    pub fn set_terms_hash(
        env: Env,
        caller: Address,
        offer_id: u64,
        participation_type: u32,
        terms_hash: BytesN<32>,
        payment_token: Address,
    ) {
        caller.require_auth();
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if caller != admin {
            panic!("admin only");
        }
        let plan = DistroPlan {
            offer_id,
            participation_type,
            terms_hash,
            payment_token,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Plan(offer_id), &plan);
    }

    /// Distribute `amount` of payment_token from `payer` across `recipients`/`shares` (basis points sum 10000).
    /// Type-specific rule (demo): patronage skips burn eligibility; economic allows later burn.
    pub fn distribute(
        env: Env,
        payer: Address,
        offer_id: u64,
        amount: i128,
        recipients: Vec<Address>,
        shares_bps: Vec<u32>,
    ) {
        payer.require_auth();
        if amount <= 0 {
            panic!("amount");
        }
        if recipients.len() != shares_bps.len() || recipients.is_empty() {
            panic!("len mismatch");
        }
        let plan: DistroPlan = env
            .storage()
            .persistent()
            .get(&DataKey::Plan(offer_id))
            .expect("no plan");
        let mut total_bps: u32 = 0;
        for s in shares_bps.iter() {
            total_bps += s;
        }
        if total_bps != 10_000 {
            panic!("bps must sum 10000");
        }
        let pay = token::Client::new(&env, &plan.payment_token);
        let n = recipients.len();
        for i in 0..n {
            let r = recipients.get(i).unwrap();
            let bps = shares_bps.get(i).unwrap() as i128;
            let share = amount.checked_mul(bps).unwrap() / 10_000;
            if share > 0 {
                pay.transfer(&payer, &r, &share);
            }
        }
        env.events().publish(
            (symbol_short!("distrib"), payer),
            (offer_id, amount, plan.participation_type),
        );
    }

    /// Burn participation units held by `holder` (economic / fractional_custody only in demo).
    pub fn burn_units(env: Env, holder: Address, offer_id: u64, units: i128) {
        holder.require_auth();
        if units <= 0 {
            panic!("units");
        }
        let plan: DistroPlan = env
            .storage()
            .persistent()
            .get(&DataKey::Plan(offer_id))
            .expect("no plan");
        if plan.participation_type != TYPE_ECONOMIC
            && plan.participation_type != TYPE_FRACTIONAL_CUSTODY
        {
            panic!("burn not allowed for type");
        }
        let participation: Address = env.storage().instance().get(&DataKey::Participation).unwrap();
        let part = ParticipationClient::new(&env, &participation);
        // Transfer to this contract then zero by transferring to a burn sink (admin).
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        part.transfer_units(&holder, &admin, &offer_id, &units);
        env.events()
            .publish((symbol_short!("burned"), holder), (offer_id, units));
    }

    pub fn get_plan(env: Env, offer_id: u64) -> DistroPlan {
        env.storage()
            .persistent()
            .get(&DataKey::Plan(offer_id))
            .expect("no plan")
    }
}

mod participation_client {
    use super::*;
    use soroban_sdk::contractclient;

    #[contractclient(name = "ParticipationClient")]
    pub trait ParticipationTrait {
        fn transfer_units(env: Env, from: Address, to: Address, offer_id: u64, units: i128);
    }
}
use participation_client::*;

mod test;
