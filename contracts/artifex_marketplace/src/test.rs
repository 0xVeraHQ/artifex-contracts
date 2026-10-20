#![cfg(test)]

use super::*;
use artifex_participation::{Contract as Part, ContractClient as PartClient, TYPE_ECONOMIC};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env,
};

#[test]
fn list_and_partial_buy() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(&env, &usdc).mint(&buyer, &500_000_000);

    let part_id = env.register(Part, ());
    let part = PartClient::new(&env, &part_id);
    part.init(&admin);
    let piece = BytesN::from_array(&env, &[8u8; 32]);
    let terms = BytesN::from_array(&env, &[9u8; 32]);
    let oid = part.create_offer(
        &seller,
        &piece,
        &TYPE_ECONOMIC,
        &usdc,
        &1_000_000,
        &50,
        &terms,
        &false,
    );
    let mkt_id = env.register(Contract, ());
    let mkt = ContractClient::new(&env, &mkt_id);
    mkt.init(&admin, &part_id);
    part.transfer_units(&seller, &mkt_id, &oid, &20);
    let lid = mkt.list(&seller, &oid, &usdc, &2_000_000, &20);
    mkt.buy_partial(&buyer, &lid, &5);
    assert_eq!(part.balance_of(&oid, &buyer), 5);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&seller), 10_000_000);
    let listing = mkt.get_listing(&lid);
    assert_eq!(listing.remaining, 15);
    assert!(listing.active);
}
