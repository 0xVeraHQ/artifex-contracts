#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

#[test]
fn authorize_and_check() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let cnn = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform);
    let piece = BytesN::from_array(&env, &[7u8; 32]);
    assert!(!client.is_authorized(&cnn, &piece));
    client.authorize(&cnn, &piece);
    assert!(client.is_authorized(&cnn, &piece));
    client.revoke_auth(&cnn, &cnn, &piece);
    assert!(!client.is_authorized(&cnn, &piece));
}

#[test]
fn platform_can_revoke_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let cnn = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform);
    let piece = BytesN::from_array(&env, &[8u8; 32]);
    client.authorize(&cnn, &piece);
    client.revoke_auth(&platform, &cnn, &piece);
    assert!(!client.is_authorized(&cnn, &piece));
}

#[test]
#[should_panic(expected = "unauthorized")]
fn stranger_cannot_revoke_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let cnn = Address::generate(&env);
    let stranger = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform);
    let piece = BytesN::from_array(&env, &[9u8; 32]);
    client.authorize(&cnn, &piece);
    client.revoke_auth(&stranger, &cnn, &piece);
}

#[test]
fn connoisseur_without_auth_is_false() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let cnn = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform);
    let piece = BytesN::from_array(&env, &[1u8; 32]);
    // Never authorized — query must stay false (gate for deceased cert path).
    assert!(!client.is_authorized(&cnn, &piece));
}
