#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, Vec,
};

fn single_token(env: &Env, token: &Address, price: i128) -> (Vec<Address>, Vec<i128>) {
    let mut tokens = Vec::new(env);
    tokens.push_back(token.clone());
    let mut prices = Vec::new(env);
    prices.push_back(price);
    (tokens, prices)
}

fn offer_setup(env: &Env) -> (Address, Address, Address, Address, ContractClient<'_>) {
    let admin = Address::generate(env);
    let seller = Address::generate(env);
    let buyer = Address::generate(env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(env, &usdc).mint(&buyer, &1_000_000_000);
    let id = env.register(Contract, ());
    let client = ContractClient::new(env, &id);
    client.init(&admin);
    (admin, seller, buyer, usdc, client)
}

#[test]
fn atomic_primary_sale() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, usdc, client) = offer_setup(&env);
    let piece = BytesN::from_array(&env, &[5u8; 32]);
    let terms = BytesN::from_array(&env, &[6u8; 32]);
    let (tokens, prices) = single_token(&env, &usdc, 1_000_000);
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &100,
        &terms,
        &true,
    );
    client.buy_primary(&buyer, &oid, &40, &usdc);
    assert_eq!(client.balance_of(&oid, &buyer), 40);
    assert_eq!(client.balance_of(&oid, &seller), 60);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&seller), 40_000_000);
    let offer = client.get_offer(&oid);
    assert_eq!(offer.remaining, 60);
}

#[test]
fn buyer_selects_xlm_among_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let xlm = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(&env, &xlm).mint(&buyer, &500_000_000);
    // Buyer has XLM only — no USDC. Offer accepts both.
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&admin);
    let piece = BytesN::from_array(&env, &[21u8; 32]);
    let terms = BytesN::from_array(&env, &[22u8; 32]);
    let mut tokens = Vec::new(&env);
    tokens.push_back(xlm.clone());
    tokens.push_back(usdc.clone());
    let mut prices = Vec::new(&env);
    prices.push_back(2_000_000i128); // XLM price
    prices.push_back(1_000_000i128); // USDC price
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &10,
        &terms,
        &true,
    );
    client.buy_primary(&buyer, &oid, &2, &xlm);
    assert_eq!(TokenClient::new(&env, &xlm).balance(&seller), 4_000_000);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&seller), 0);
    assert_eq!(client.balance_of(&oid, &buyer), 2);
}

#[test]
#[should_panic(expected = "token not accepted")]
fn buy_rejects_token_not_on_offer() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, usdc, client) = offer_setup(&env);
    let other = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    StellarAssetClient::new(&env, &other).mint(&buyer, &1_000_000_000);
    let piece = BytesN::from_array(&env, &[23u8; 32]);
    let terms = BytesN::from_array(&env, &[24u8; 32]);
    let (tokens, prices) = single_token(&env, &usdc, 1_000_000);
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &10,
        &terms,
        &true,
    );
    client.buy_primary(&buyer, &oid, &1, &other);
}

#[test]
#[should_panic(expected = "insufficient remaining")]
fn buy_more_than_available_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, usdc, client) = offer_setup(&env);
    let piece = BytesN::from_array(&env, &[1u8; 32]);
    let terms = BytesN::from_array(&env, &[2u8; 32]);
    let (tokens, prices) = single_token(&env, &usdc, 1_000_000);
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &10,
        &terms,
        &true,
    );
    client.buy_primary(&buyer, &oid, &11, &usdc);
}

#[test]
#[should_panic]
fn buy_insufficient_balance_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let poor = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    // poor has 0 USDC
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&admin);
    let piece = BytesN::from_array(&env, &[3u8; 32]);
    let terms = BytesN::from_array(&env, &[4u8; 32]);
    let (tokens, prices) = single_token(&env, &usdc, 1_000_000);
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &10,
        &terms,
        &true,
    );
    client.buy_primary(&poor, &oid, &1, &usdc);
}

#[test]
#[should_panic(expected = "inactive")]
fn buy_cancelled_offer_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, usdc, client) = offer_setup(&env);
    let piece = BytesN::from_array(&env, &[7u8; 32]);
    let terms = BytesN::from_array(&env, &[8u8; 32]);
    let (tokens, prices) = single_token(&env, &usdc, 1_000_000);
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &10,
        &terms,
        &true,
    );
    client.cancel_offer(&seller, &oid);
    client.buy_primary(&buyer, &oid, &1, &usdc);
}

#[test]
#[should_panic(expected = "not seller")]
fn non_seller_cannot_cancel() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, usdc, client) = offer_setup(&env);
    let piece = BytesN::from_array(&env, &[9u8; 32]);
    let terms = BytesN::from_array(&env, &[10u8; 32]);
    let (tokens, prices) = single_token(&env, &usdc, 1_000_000);
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &10,
        &terms,
        &true,
    );
    client.cancel_offer(&buyer, &oid);
}

#[test]
fn cancel_offer_positive() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, _buyer, usdc, client) = offer_setup(&env);
    let piece = BytesN::from_array(&env, &[11u8; 32]);
    let terms = BytesN::from_array(&env, &[12u8; 32]);
    let (tokens, prices) = single_token(&env, &usdc, 500_000);
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_PATRONAGE,
        &tokens,
        &prices,
        &20,
        &terms,
        &false,
    );
    client.cancel_offer(&seller, &oid);
    assert!(!client.get_offer(&oid).active);
}
