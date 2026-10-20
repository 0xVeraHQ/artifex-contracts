#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};

#[test]
fn living_author_flow() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let validator = Address::generate(&env);
    let fee_c = Address::generate(&env);
    let artist = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &validator, &fee_c);

    let piece = BytesN::from_array(&env, &[1u8; 32]);
    let claim = BytesN::from_array(&env, &[2u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    client.propose_cert(
        &artist,
        &piece,
        &claim,
        &CERT_LIVING_AUTHOR,
        &required,
    );
    client.cosign(&artist, &piece);
    client.mark_fee_paid(&platform, &piece);
    client.finalize_cert(&artist, &piece);
    assert!(client.is_finalized(&piece));
    let c = client.get_cert(&piece);
    assert_eq!(c.cert_type, CERT_LIVING_AUTHOR);
    assert_eq!(c.signed.len(), 1);
}

#[test]
#[should_panic(expected = "fee required")]
fn finalize_without_fee_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let validator = Address::generate(&env);
    let fee_c = Address::generate(&env);
    let artist = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &validator, &fee_c);
    let piece = BytesN::from_array(&env, &[3u8; 32]);
    let claim = BytesN::from_array(&env, &[4u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    client.propose_cert(
        &artist,
        &piece,
        &claim,
        &CERT_LIVING_AUTHOR,
        &required,
    );
    client.cosign(&artist, &piece);
    client.finalize_cert(&artist, &piece);
}
