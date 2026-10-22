#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env,
};

/// Participation types (4) — product keys: future_sale / guaranteed_buyback /
/// return_financing / exploitation_rights. On-chain values are stable u32 codes.
pub const TYPE_FUTURE_SALE: u32 = 1;
pub const TYPE_GUARANTEED_BUYBACK: u32 = 2;
pub const TYPE_RETURN_FINANCING: u32 = 3;
pub const TYPE_EXPLOITATION_RIGHTS: u32 = 4;
/// Legacy aliases (same u32) — prefer TYPE_FUTURE_SALE etc.
pub const TYPE_ECONOMIC: u32 = TYPE_FUTURE_SALE;
pub const TYPE_PATRONAGE: u32 = TYPE_GUARANTEED_BUYBACK;
pub const TYPE_EXHIBITION: u32 = TYPE_RETURN_FINANCING;
pub const TYPE_FRACTIONAL_CUSTODY: u32 = TYPE_EXPLOITATION_RIGHTS;

#[contracttype]
#[derive(Clone)]
pub struct Offer {
    pub offer_id: u64,
    pub piece_id: BytesN<32>,
    pub seller: Address,
    pub participation_type: u32,
    pub payment_token: Address,
    pub unit_price: i128,
    pub total_units: i128,
    pub remaining: i128,
    pub terms_hash: BytesN<32>,
    pub certified: bool,
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    NextOffer,
    Offer(u64),
    Balance(u64, Address),
    Inited,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Inited) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextOffer, &1u64);
        env.storage().instance().set(&DataKey::Inited, &true);
    }

    pub fn create_offer(
        env: Env,
        seller: Address,
        piece_id: BytesN<32>,
        participation_type: u32,
        payment_token: Address,
        unit_price: i128,
        total_units: i128,
        terms_hash: BytesN<32>,
        certified: bool,
    ) -> u64 {
        seller.require_auth();
        if participation_type < TYPE_FUTURE_SALE
            || participation_type > TYPE_EXPLOITATION_RIGHTS
        {
            panic!("bad type");
        }
        if unit_price <= 0 || total_units <= 0 {
            panic!("bad amounts");
        }
        let oid: u64 = env.storage().instance().get(&DataKey::NextOffer).unwrap();
        env.storage().instance().set(&DataKey::NextOffer, &(oid + 1));
        let offer = Offer {
            offer_id: oid,
            piece_id,
            seller: seller.clone(),
            participation_type,
            payment_token,
            unit_price,
            total_units,
            remaining: total_units,
            terms_hash,
            certified,
            active: true,
        };
        env.storage().persistent().set(&DataKey::Offer(oid), &offer);
        // Seller holds unsold inventory as internal balance until purchase.
        env.storage()
            .persistent()
            .set(&DataKey::Balance(oid, seller.clone()), &total_units);
        env.events()
            .publish((symbol_short!("offer_new"), seller), oid);
        oid
    }

    /// Atomic primary sale: payment + unit delivery in one invocation.
    pub fn buy_primary(env: Env, buyer: Address, offer_id: u64, units: i128) {
        buyer.require_auth();
        if units <= 0 {
            panic!("units");
        }
        let mut offer: Offer = env
            .storage()
            .persistent()
            .get(&DataKey::Offer(offer_id))
            .expect("no offer");
        if !offer.active {
            panic!("inactive");
        }
        if units > offer.remaining {
            panic!("insufficient remaining");
        }
        let payment = units
            .checked_mul(offer.unit_price)
            .expect("overflow");
        let pay = token::Client::new(&env, &offer.payment_token);
        // Payment to seller
        pay.transfer(&buyer, &offer.seller, &payment);
        // Deliver units: reduce seller inventory, credit buyer
        let seller_bal: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(offer_id, offer.seller.clone()))
            .unwrap_or(0);
        if seller_bal < units {
            panic!("seller inventory");
        }
        env.storage().persistent().set(
            &DataKey::Balance(offer_id, offer.seller.clone()),
            &(seller_bal - units),
        );
        let buyer_bal: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(offer_id, buyer.clone()))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::Balance(offer_id, buyer.clone()),
            &(buyer_bal + units),
        );
        offer.remaining -= units;
        if offer.remaining == 0 {
            offer.active = false;
        }
        env.storage()
            .persistent()
            .set(&DataKey::Offer(offer_id), &offer);
        env.events()
            .publish((symbol_short!("prim_sold"), buyer), (offer_id, units));
    }

    pub fn cancel_offer(env: Env, seller: Address, offer_id: u64) {
        seller.require_auth();
        let mut offer: Offer = env
            .storage()
            .persistent()
            .get(&DataKey::Offer(offer_id))
            .expect("no offer");
        if offer.seller != seller {
            panic!("not seller");
        }
        offer.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Offer(offer_id), &offer);
        env.events()
            .publish((symbol_short!("offer_can"), seller), offer_id);
    }

    pub fn get_offer(env: Env, offer_id: u64) -> Offer {
        env.storage()
            .persistent()
            .get(&DataKey::Offer(offer_id))
            .expect("no offer")
    }

    pub fn balance_of(env: Env, offer_id: u64, holder: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(offer_id, holder))
            .unwrap_or(0)
    }

    /// Transfer participation units (for secondary marketplace escrow deposit).
    pub fn transfer_units(env: Env, from: Address, to: Address, offer_id: u64, units: i128) {
        from.require_auth();
        if units <= 0 {
            panic!("units");
        }
        let from_bal: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(offer_id, from.clone()))
            .unwrap_or(0);
        if from_bal < units {
            panic!("insufficient");
        }
        env.storage().persistent().set(
            &DataKey::Balance(offer_id, from),
            &(from_bal - units),
        );
        let to_bal: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(offer_id, to.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(offer_id, to), &(to_bal + units));
    }
}

mod test;
