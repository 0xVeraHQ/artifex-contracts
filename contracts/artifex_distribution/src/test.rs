#![cfg(test)]

use super::*;
use artifex_participation::{
    Contract as Part, ContractClient as PartClient, TYPE_ECONOMIC, TYPE_PATRONAGE,
};
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

#[test]
fn distribute_and_burn() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let holder = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(&env, &usdc).mint(&seller, &1_000_000_000);

    let part_id = env.register(Part, ());
    let part = PartClient::new(&env, &part_id);
    part.init(&admin);
    let piece = BytesN::from_array(&env, &[1u8; 32]);
    let terms = BytesN::from_array(&env, &[2u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &100,
        &terms,
        &true,
    );
    part.transfer_units(&seller, &holder, &oid, &30);

    let dist_id = env.register(Contract, ());
    let dist = ContractClient::new(&env, &dist_id);
    dist.init(&admin, &part_id);
    dist.set_terms_hash(&admin, &oid, &TYPE_ECONOMIC, &terms, &usdc);

    let mut recipients = Vec::new(&env);
    recipients.push_back(a.clone());
    recipients.push_back(b.clone());
    let mut shares = Vec::new(&env);
    shares.push_back(6000u32);
    shares.push_back(4000u32);
    dist.distribute(&seller, &oid, &100_000_000, &recipients, &shares);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&a), 60_000_000);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&b), 40_000_000);

    dist.burn_units(&holder, &oid, &10);
    assert_eq!(part.balance_of(&oid, &holder), 20);
    assert_eq!(part.balance_of(&oid, &admin), 10);
}

#[test]
#[should_panic(expected = "bps must sum 10000")]
fn distribution_bps_mismatch_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let a = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(&env, &usdc).mint(&seller, &1_000_000_000);

    let part_id = env.register(Part, ());
    let part = PartClient::new(&env, &part_id);
    part.init(&admin);
    let piece = BytesN::from_array(&env, &[3u8; 32]);
    let terms = BytesN::from_array(&env, &[4u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &100,
        &terms,
        &true,
    );

    let dist_id = env.register(Contract, ());
    let dist = ContractClient::new(&env, &dist_id);
    dist.init(&admin, &part_id);
    dist.set_terms_hash(&admin, &oid, &TYPE_ECONOMIC, &terms, &usdc);

    let mut recipients = Vec::new(&env);
    recipients.push_back(a);
    let mut shares = Vec::new(&env);
    shares.push_back(5000u32); // not 10000
    dist.distribute(&seller, &oid, &100_000_000, &recipients, &shares);
}

#[test]
#[should_panic(expected = "len mismatch")]
fn distribution_len_mismatch_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(&env, &usdc).mint(&seller, &1_000_000_000);

    let part_id = env.register(Part, ());
    let part = PartClient::new(&env, &part_id);
    part.init(&admin);
    let piece = BytesN::from_array(&env, &[5u8; 32]);
    let terms = BytesN::from_array(&env, &[6u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &tokens,
        &prices,
        &100,
        &terms,
        &true,
    );

    let dist_id = env.register(Contract, ());
    let dist = ContractClient::new(&env, &dist_id);
    dist.init(&admin, &part_id);
    dist.set_terms_hash(&admin, &oid, &TYPE_ECONOMIC, &terms, &usdc);

    let mut recipients = Vec::new(&env);
    recipients.push_back(a);
    recipients.push_back(b);
    let mut shares = Vec::new(&env);
    shares.push_back(10000u32); // len 1 vs recipients 2
    dist.distribute(&seller, &oid, &100_000_000, &recipients, &shares);
}

#[test]
#[should_panic(expected = "admin only")]
fn non_platform_cannot_set_terms() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let stranger = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let part_id = env.register(Part, ());
    PartClient::new(&env, &part_id).init(&admin);
    let dist_id = env.register(Contract, ());
    let dist = ContractClient::new(&env, &dist_id);
    dist.init(&admin, &part_id);
    let terms = BytesN::from_array(&env, &[7u8; 32]);
    dist.set_terms_hash(&stranger, &1u64, &TYPE_ECONOMIC, &terms, &usdc);
}

#[test]
#[should_panic(expected = "burn not allowed for type")]
fn burn_not_allowed_for_patronage() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let holder = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();

    let part_id = env.register(Part, ());
    let part = PartClient::new(&env, &part_id);
    part.init(&admin);
    let piece = BytesN::from_array(&env, &[8u8; 32]);
    let terms = BytesN::from_array(&env, &[9u8; 32]);
    let (tokens, prices) = tokens_prices(&env, &usdc, 1);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_PATRONAGE,
        &tokens,
        &prices,
        &100,
        &terms,
        &true,
    );
    part.transfer_units(&seller, &holder, &oid, &30);

    let dist_id = env.register(Contract, ());
    let dist = ContractClient::new(&env, &dist_id);
    dist.init(&admin, &part_id);
    dist.set_terms_hash(&admin, &oid, &TYPE_PATRONAGE, &terms, &usdc);
    dist.burn_units(&holder, &oid, &10);
}
