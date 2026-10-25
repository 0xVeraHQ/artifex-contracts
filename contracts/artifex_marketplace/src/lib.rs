#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env,
};

/// Secondary marketplace: escrow units (held by this contract via participation.transfer_units)
/// or whole-NFT style unit amounts against an offer_id from participation.
#[contracttype]
#[derive(Clone)]
pub struct Listing {
    pub listing_id: u64,
    pub seller: Address,
    pub offer_id: u64,
    pub payment_token: Address,
    pub unit_price: i128,
    pub remaining: i128,
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Participation,
    NextListing,
    Listing(u64),
    /// Units deposited for listing but not yet consumed by `list` / still reclaimable.
    /// Keyed by (depositor, offer_id). Populated by `deposit_for_list`; consumed by `list`
    /// when credit is present; returned by `recover_orphaned_units`.
    Deposit(Address, u64),
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
        env.storage().instance().set(&DataKey::NextListing, &1u64);
        env.storage().instance().set(&DataKey::Inited, &true);
    }

    /// Explicit escrow deposit that credits the depositor. Prefer `list_with_deposit` for
    /// a single Freighter signature; use this when splitting deposit and list intentionally.
    pub fn deposit_for_list(env: Env, seller: Address, offer_id: u64, units: i128) {
        seller.require_auth();
        if units <= 0 {
            panic!("units");
        }
        let participation: Address =
            env.storage().instance().get(&DataKey::Participation).unwrap();
        let part_client = ParticipationClient::new(&env, &participation);
        part_client.transfer_units(
            &seller,
            &env.current_contract_address(),
            &offer_id,
            &units,
        );
        let key = DataKey::Deposit(seller.clone(), offer_id);
        let prev: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(prev + units));
        env.events()
            .publish((symbol_short!("deposit"), seller), (offer_id, units));
    }

    /// Seller must have already transferred `units` into this contract via participation.transfer_units
    /// (legacy two-tx path) OR via `deposit_for_list` (credited). When a deposit credit exists for
    /// (seller, offer_id), it is consumed; otherwise behavior matches the legacy uncredited escrow.
    pub fn list(
        env: Env,
        seller: Address,
        offer_id: u64,
        payment_token: Address,
        unit_price: i128,
        units: i128,
    ) -> u64 {
        seller.require_auth();
        if unit_price <= 0 || units <= 0 {
            panic!("bad amounts");
        }
        Self::consume_deposit_credit(&env, &seller, offer_id, units);
        Self::create_listing(
            &env,
            seller,
            offer_id,
            payment_token,
            unit_price,
            units,
        )
    }

    /// Atomic escrow + list in one invocation (single Freighter signature).
    /// Pulls `units` from seller into this contract, then opens the listing.
    pub fn list_with_deposit(
        env: Env,
        seller: Address,
        offer_id: u64,
        payment_token: Address,
        unit_price: i128,
        units: i128,
    ) -> u64 {
        seller.require_auth();
        if unit_price <= 0 || units <= 0 {
            panic!("bad amounts");
        }
        let participation: Address =
            env.storage().instance().get(&DataKey::Participation).unwrap();
        let part_client = ParticipationClient::new(&env, &participation);
        part_client.transfer_units(
            &seller,
            &env.current_contract_address(),
            &offer_id,
            &units,
        );
        Self::create_listing(
            &env,
            seller,
            offer_id,
            payment_token,
            unit_price,
            units,
        )
    }

    /// Return credited deposit units that were never listed (owner recovery).
    /// Only units tracked under Deposit(seller, offer_id) can be recovered this way.
    pub fn recover_orphaned_units(
        env: Env,
        seller: Address,
        offer_id: u64,
        units: i128,
    ) {
        seller.require_auth();
        if units <= 0 {
            panic!("units");
        }
        let key = DataKey::Deposit(seller.clone(), offer_id);
        let credit: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if credit < units {
            panic!("no credit");
        }
        env.storage().persistent().set(&key, &(credit - units));
        let participation: Address =
            env.storage().instance().get(&DataKey::Participation).unwrap();
        let part_client = ParticipationClient::new(&env, &participation);
        part_client.transfer_units(
            &env.current_contract_address(),
            &seller,
            &offer_id,
            &units,
        );
        env.events()
            .publish((symbol_short!("recover"), seller), (offer_id, units));
    }

    /// Platform return of uncredited legacy escrow (transfer_units without list / without deposit credit).
    /// Used for Testnet remediation of trapped units after interrupt between the two Freighter txs.
    pub fn admin_return_orphaned_units(
        env: Env,
        admin: Address,
        to: Address,
        offer_id: u64,
        units: i128,
    ) {
        admin.require_auth();
        let stored: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored {
            panic!("not admin");
        }
        if units <= 0 {
            panic!("units");
        }
        let participation: Address =
            env.storage().instance().get(&DataKey::Participation).unwrap();
        let part_client = ParticipationClient::new(&env, &participation);
        part_client.transfer_units(
            &env.current_contract_address(),
            &to,
            &offer_id,
            &units,
        );
        env.events()
            .publish((symbol_short!("adm_ret"), to), (offer_id, units));
    }

    pub fn deposit_credit(env: Env, seller: Address, offer_id: u64) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Deposit(seller, offer_id))
            .unwrap_or(0)
    }

    /// Partial fill: pay seller + deliver escrowed units from marketplace contract balance.
    pub fn buy_partial(env: Env, buyer: Address, listing_id: u64, units: i128) {
        buyer.require_auth();
        if units <= 0 {
            panic!("units");
        }
        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&DataKey::Listing(listing_id))
            .expect("no listing");
        if !listing.active {
            panic!("inactive");
        }
        if units > listing.remaining {
            panic!("insufficient");
        }
        let payment = units
            .checked_mul(listing.unit_price)
            .expect("overflow");
        let pay = token::Client::new(&env, &listing.payment_token);
        pay.transfer(&buyer, &listing.seller, &payment);

        let participation: Address = env.storage().instance().get(&DataKey::Participation).unwrap();
        // Deliver units held by this contract to buyer.
        // Marketplace must be authorized; in tests we mock_all_auths.
        // Cross-contract: invoke transfer_units(from=self, to=buyer, ...)
        let part_client = ParticipationClient::new(&env, &participation);
        part_client.transfer_units(
            &env.current_contract_address(),
            &buyer,
            &listing.offer_id,
            &units,
        );

        listing.remaining -= units;
        if listing.remaining == 0 {
            listing.active = false;
            env.events()
                .publish((symbol_short!("list_fill"), buyer), listing_id);
        } else {
            env.events().publish(
                (symbol_short!("partial"), buyer),
                (listing_id, units),
            );
        }
        env.storage()
            .persistent()
            .set(&DataKey::Listing(listing_id), &listing);
    }

    pub fn cancel_listing(env: Env, seller: Address, listing_id: u64) {
        seller.require_auth();
        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&DataKey::Listing(listing_id))
            .expect("no listing");
        if listing.seller != seller {
            panic!("not seller");
        }
        if listing.remaining > 0 {
            let participation: Address =
                env.storage().instance().get(&DataKey::Participation).unwrap();
            let part_client = ParticipationClient::new(&env, &participation);
            part_client.transfer_units(
                &env.current_contract_address(),
                &seller,
                &listing.offer_id,
                &listing.remaining,
            );
        }
        listing.active = false;
        listing.remaining = 0;
        env.storage()
            .persistent()
            .set(&DataKey::Listing(listing_id), &listing);
        env.events()
            .publish((symbol_short!("list_can"), seller), listing_id);
    }

    pub fn get_listing(env: Env, listing_id: u64) -> Listing {
        env.storage()
            .persistent()
            .get(&DataKey::Listing(listing_id))
            .expect("no listing")
    }

    fn consume_deposit_credit(env: &Env, seller: &Address, offer_id: u64, units: i128) {
        let key = DataKey::Deposit(seller.clone(), offer_id);
        let credit: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if credit == 0 {
            // Legacy path: units already sit in marketplace via raw transfer_units.
            return;
        }
        if credit < units {
            panic!("insufficient credit");
        }
        env.storage().persistent().set(&key, &(credit - units));
    }

    fn create_listing(
        env: &Env,
        seller: Address,
        offer_id: u64,
        payment_token: Address,
        unit_price: i128,
        units: i128,
    ) -> u64 {
        let lid: u64 = env.storage().instance().get(&DataKey::NextListing).unwrap();
        env.storage().instance().set(&DataKey::NextListing, &(lid + 1));
        let listing = Listing {
            listing_id: lid,
            seller: seller.clone(),
            offer_id,
            payment_token,
            unit_price,
            remaining: units,
            active: true,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Listing(lid), &listing);
        env.events()
            .publish((symbol_short!("listed"), seller), (lid, units));
        lid
    }
}

/// Minimal client for artifex_participation.transfer_units.
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
