#![cfg(test)]

use super::*;
use artifex_participation::{Contract as Part, ContractClient as PartClient, TYPE_ECONOMIC};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, Vec,
};

fn tokens_prices(env: &Env, token: &Address, price: i128) -> (Vec<Address>, Vec<i128>) {
    let mut tokens = Vec::new(env);
    tokens.push_back(token.clone());
    let mut prices = Vec::new(env);
    prices.push_back(price);
    (tokens, prices)
}

fn mkt_setup(
    env: &Env,
) -> (
    Address,
    Address,
    Address,
    Address,
    Address,
    PartClient<'_>,
    ContractClient<'_>,
) {
    let admin = Address::generate(env);
    let seller = Address::generate(env);
    let buyer = Address::generate(env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(env, &usdc).mint(&buyer, &500_000_000);

    let part_id = env.register(Part, ());
    let part = PartClient::new(env, &part_id);
    part.init(&admin);
    let mkt_id = env.register(Contract, ());
    let mkt = ContractClient::new(env, &mkt_id);
    mkt.init(&admin, &part_id);
    (admin, seller, buyer, usdc, mkt_id, part, mkt)
}

#[test]
fn list_and_partial_buy() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, usdc, mkt_id, part, mkt) = mkt_setup(&env);
    let piece = BytesN::from_array(&env, &[8u8; 32]);
    let terms = BytesN::from_array(&env, &[9u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1_000_000);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &50,
        &terms,
        &false,
    );
    part.transfer_units(&seller, &mkt_id, &oid, &20);
    let lid = mkt.list(&seller, &oid, &usdc, &2_000_000, &20);
    mkt.buy_partial(&buyer, &lid, &5);
    assert_eq!(part.balance_of(&oid, &buyer), 5);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&seller), 10_000_000);
    let listing = mkt.get_listing(&lid);
    assert_eq!(listing.remaining, 15);
    assert!(listing.active);
}

#[test]
#[should_panic(expected = "insufficient")]
fn buy_more_than_listing_available_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, usdc, mkt_id, part, mkt) = mkt_setup(&env);
    let piece = BytesN::from_array(&env, &[1u8; 32]);
    let terms = BytesN::from_array(&env, &[2u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1_000_000);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &50,
        &terms,
        &false,
    );
    part.transfer_units(&seller, &mkt_id, &oid, &5);
    let lid = mkt.list(&seller, &oid, &usdc, &2_000_000, &5);
    mkt.buy_partial(&buyer, &lid, &6);
}

#[test]
#[should_panic(expected = "inactive")]
fn buy_cancelled_listing_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, usdc, mkt_id, part, mkt) = mkt_setup(&env);
    let piece = BytesN::from_array(&env, &[3u8; 32]);
    let terms = BytesN::from_array(&env, &[4u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1_000_000);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &50,
        &terms,
        &false,
    );
    part.transfer_units(&seller, &mkt_id, &oid, &10);
    let lid = mkt.list(&seller, &oid, &usdc, &2_000_000, &10);
    mkt.cancel_listing(&seller, &lid);
    mkt.buy_partial(&buyer, &lid, &1);
}

#[test]
#[should_panic]
fn buy_wrong_asset_insufficient_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    // buyer has no USDC — wrong/insufficient asset
    let part_id = env.register(Part, ());
    let part = PartClient::new(&env, &part_id);
    part.init(&admin);
    let mkt_id = env.register(Contract, ());
    let mkt = ContractClient::new(&env, &mkt_id);
    mkt.init(&admin, &part_id);
    let piece = BytesN::from_array(&env, &[5u8; 32]);
    let terms = BytesN::from_array(&env, &[6u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1_000_000);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &50,
        &terms,
        &false,
    );
    part.transfer_units(&seller, &mkt_id, &oid, &10);
    let lid = mkt.list(&seller, &oid, &usdc, &2_000_000, &10);
    mkt.buy_partial(&buyer, &lid, &1);
}

#[test]
fn cancel_listing_returns_units() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, _buyer, usdc, mkt_id, part, mkt) = mkt_setup(&env);
    let piece = BytesN::from_array(&env, &[7u8; 32]);
    let terms = BytesN::from_array(&env, &[8u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1_000_000);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &50,
        &terms,
        &false,
    );
    part.transfer_units(&seller, &mkt_id, &oid, &10);
    let lid = mkt.list(&seller, &oid, &usdc, &2_000_000, &10);
    mkt.cancel_listing(&seller, &lid);
    assert!(!mkt.get_listing(&lid).active);
    assert_eq!(part.balance_of(&oid, &seller), 50);
}
