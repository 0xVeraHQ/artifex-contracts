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

    /// Seller must have already transferred `units` into this contract via participation.transfer_units.
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
