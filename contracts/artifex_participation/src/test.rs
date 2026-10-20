#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env,
};

#[test]
fn atomic_primary_sale() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(&env, &usdc).mint(&buyer, &1_000_000_000);

    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&admin);
    let piece = BytesN::from_array(&env, &[5u8; 32]);
    let terms = BytesN::from_array(&env, &[6u8; 32]);
    let oid = client.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &usdc,
        &1_000_000,
        &100,
        &terms,
        &true,
    );
    client.buy_primary(&buyer, &oid, &40);
    assert_eq!(client.balance_of(&oid, &buyer), 40);
    assert_eq!(client.balance_of(&oid, &seller), 60);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&seller), 40_000_000);
    let offer = client.get_offer(&oid);
    assert_eq!(offer.remaining, 60);
}
