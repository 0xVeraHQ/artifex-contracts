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
