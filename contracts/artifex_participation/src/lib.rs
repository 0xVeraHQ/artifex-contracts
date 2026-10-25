#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env, Vec,
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
    /// Accepted settlement SACs (e.g. XLM SAC and/or USDC SAC). Buyer selects one at buy.
    pub payment_tokens: Vec<Address>,
    /// Unit price per accepted token (parallel to `payment_tokens`).
    pub unit_prices: Vec<i128>,
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

    /// Create an offer that accepts one or more payment tokens (buyer picks among them).
    /// `payment_tokens` and `unit_prices` must be same non-empty length.
    pub fn create_offer(
        env: Env,
        seller: Address,
        piece_id: BytesN<32>,
        participation_type: u32,
        payment_tokens: Vec<Address>,
        unit_prices: Vec<i128>,
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
        if payment_tokens.is_empty() || payment_tokens.len() != unit_prices.len() {
            panic!("bad tokens");
        }
        if total_units <= 0 {
            panic!("bad amounts");
        }
        for p in unit_prices.iter() {
            if p <= 0 {
                panic!("bad amounts");
            }
        }
        let n = payment_tokens.len();
        for i in 0..n {
            for j in (i + 1)..n {
                if payment_tokens.get(i).unwrap() == payment_tokens.get(j).unwrap() {
                    panic!("dup token");
                }
            }
        }
        let oid: u64 = env.storage().instance().get(&DataKey::NextOffer).unwrap();
        env.storage().instance().set(&DataKey::NextOffer, &(oid + 1));
        let offer = Offer {
            offer_id: oid,
            piece_id,
            seller: seller.clone(),
            participation_type,
            payment_tokens,
            unit_prices,
            total_units,
            remaining: total_units,
            terms_hash,
            certified,
            active: true,
        };
        env.storage().persistent().set(&DataKey::Offer(oid), &offer);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(oid, seller.clone()), &total_units);
        env.events()
            .publish((symbol_short!("offer_new"), seller), oid);
        oid
    }

    /// Atomic primary sale: payment in the buyer-selected accepted token + unit delivery.
    pub fn buy_primary(
        env: Env,
        buyer: Address,
        offer_id: u64,
        units: i128,
        payment_token: Address,
    ) {
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
        let mut unit_price: Option<i128> = None;
        for i in 0..offer.payment_tokens.len() {
            if offer.payment_tokens.get(i).unwrap() == payment_token {
                unit_price = Some(offer.unit_prices.get(i).unwrap());
                break;
            }
        }
        let unit_price = match unit_price {
            Some(p) => p,
            None => panic!("token not accepted"),
        };
        let payment = units.checked_mul(unit_price).expect("overflow");
        let pay = token::Client::new(&env, &payment_token);
        pay.transfer(&buyer, &offer.seller, &payment);
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
