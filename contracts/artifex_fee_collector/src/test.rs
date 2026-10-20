#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

fn setup_token(env: &Env, admin: &Address) -> Address {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    sac.address()
}

#[test]
fn collect_usdc_and_withdraw() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let payer = Address::generate(&env);
    let usdc = setup_token(&env, &platform);
    let usdc_admin = StellarAssetClient::new(&env, &usdc);
    usdc_admin.mint(&payer, &10_000_000);

    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &usdc, &50_000_000, &10_000_000);

    client.collect_usdc(&payer);
    let (x, u) = client.balances();
    assert_eq!(x, 0);
    assert_eq!(u, 10_000_000);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&payer), 0);

    client.withdraw(&platform, &usdc, &10_000_000);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&platform), 10_000_000);
}

#[test]
fn set_fee_and_read() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let usdc = setup_token(&env, &platform);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &usdc, &50_000_000, &10_000_000);
    client.set_fee(&platform, &60_000_000, &20_000_000);
    let (x, u) = client.fees();
    assert_eq!(x, 60_000_000);
    assert_eq!(u, 20_000_000);
}

#[test]
#[should_panic(expected = "platform only")]
fn non_platform_cannot_set_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let other = Address::generate(&env);
    let usdc = setup_token(&env, &platform);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &usdc, &50_000_000, &10_000_000);
    client.set_fee(&other, &1, &1);
}

#[test]
#[should_panic(expected = "platform only")]
fn non_platform_cannot_withdraw() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let payer = Address::generate(&env);
    let other = Address::generate(&env);
    let usdc = setup_token(&env, &platform);
    StellarAssetClient::new(&env, &usdc).mint(&payer, &10_000_000);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &usdc, &50_000_000, &10_000_000);
    client.collect_usdc(&payer);
    client.withdraw(&other, &usdc, &10_000_000);
}

#[test]
#[should_panic]
fn collect_usdc_insufficient_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let payer = Address::generate(&env);
    let usdc = setup_token(&env, &platform);
    // payer has 0
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &usdc, &50_000_000, &10_000_000);
    client.collect_usdc(&payer);
}
