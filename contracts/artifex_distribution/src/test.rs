#![cfg(test)]

use super::*;
use artifex_participation::{Contract as Part, ContractClient as PartClient, TYPE_ECONOMIC};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, Vec,
};

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
    StellarAssetClient::new(&env, &usdc).mint(&holder, &0);

    let part_id = env.register(Part, ());
    let part = PartClient::new(&env, &part_id);
    part.init(&admin);
    let piece = BytesN::from_array(&env, &[1u8; 32]);
    let terms = BytesN::from_array(&env, &[2u8; 32]);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &usdc,
        &1,
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
