#![cfg(test)]

use super::*;
use artifex_connoisseur::{Contract as Cnn, ContractClient as CnnClient};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};

fn setup_env() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let validator = Address::generate(&env);
    let fee_c = Address::generate(&env);
    let cnn_id = env.register(Cnn, ());
    let cnn = CnnClient::new(&env, &cnn_id);
    cnn.init(&platform);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &validator, &fee_c, &cnn_id);
    (env, id, cnn_id, platform, validator, fee_c)
}

#[test]
fn living_author_flow() {
    let (env, id, _cnn, platform, _validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let artist = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[1u8; 32]);
    let claim = BytesN::from_array(&env, &[2u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    client.propose_cert(&artist, &piece, &claim, &CERT_LIVING_AUTHOR, &required);
    client.cosign(&artist, &piece);
    client.mark_fee_paid(&platform, &piece);
    client.finalize_cert(&artist, &piece);
    assert!(client.is_finalized(&piece));
    let c = client.get_cert(&piece);
    assert_eq!(c.cert_type, CERT_LIVING_AUTHOR);
    assert_eq!(c.signed.len(), 1);
}

#[test]
fn deceased_connoisseur_flow_with_auth() {
    let (env, id, cnn_id, platform, validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let cnn = CnnClient::new(&env, &cnn_id);
    let connoisseur = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[11u8; 32]);
    let claim = BytesN::from_array(&env, &[12u8; 32]);
    cnn.authorize(&connoisseur, &piece);
    let mut required = Vec::new(&env);
    required.push_back(connoisseur.clone());
    required.push_back(validator.clone());
    client.propose_cert(
        &connoisseur,
        &piece,
        &claim,
        &CERT_DECEASED_CONNOISSEUR,
        &required,
    );
    client.cosign(&connoisseur, &piece);
    client.cosign(&validator, &piece);
    client.mark_fee_paid(&platform, &piece);
    client.finalize_cert(&connoisseur, &piece);
    assert!(client.is_finalized(&piece));
}

#[test]
#[should_panic(expected = "fee required")]
fn finalize_without_fee_panics() {
    let (env, id, _cnn, _platform, _validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let artist = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[3u8; 32]);
    let claim = BytesN::from_array(&env, &[4u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    client.propose_cert(&artist, &piece, &claim, &CERT_LIVING_AUTHOR, &required);
    client.cosign(&artist, &piece);
    client.finalize_cert(&artist, &piece);
}

#[test]
#[should_panic(expected = "required signer count")]
fn dead_author_single_sig_propose_fails() {
    let (env, id, _cnn, _platform, _validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let cnn = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[5u8; 32]);
    let claim = BytesN::from_array(&env, &[6u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(cnn);
    // Deceased types require 2 signers — single signer must fail.
    client.propose_cert(
        &Address::generate(&env),
        &piece,
        &claim,
        &CERT_DECEASED_CONNOISSEUR,
        &required,
    );
}

#[test]
#[should_panic(expected = "missing signature")]
fn dead_author_finalize_with_one_of_two_fails() {
    let (env, id, cnn_id, platform, validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let cnn = CnnClient::new(&env, &cnn_id);
    let connoisseur = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[13u8; 32]);
    let claim = BytesN::from_array(&env, &[14u8; 32]);
    cnn.authorize(&connoisseur, &piece);
    let mut required = Vec::new(&env);
    required.push_back(connoisseur.clone());
    required.push_back(validator.clone());
    client.propose_cert(
        &connoisseur,
        &piece,
        &claim,
        &CERT_DECEASED_CONNOISSEUR,
        &required,
    );
    client.cosign(&connoisseur, &piece);
    // validator never signs
    client.mark_fee_paid(&platform, &piece);
    client.finalize_cert(&connoisseur, &piece);
}

#[test]
#[should_panic(expected = "connoisseur not authorized")]
fn connoisseur_without_auth_cannot_finalize() {
    let (env, id, _cnn_id, platform, validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let connoisseur = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[15u8; 32]);
    let claim = BytesN::from_array(&env, &[16u8; 32]);
    // No cnn.authorize call
    let mut required = Vec::new(&env);
    required.push_back(connoisseur.clone());
    required.push_back(validator.clone());
    client.propose_cert(
        &connoisseur,
        &piece,
        &claim,
        &CERT_DECEASED_CONNOISSEUR,
        &required,
    );
    client.cosign(&connoisseur, &piece);
    client.cosign(&validator, &piece);
    client.mark_fee_paid(&platform, &piece);
    client.finalize_cert(&connoisseur, &piece);
}

#[test]
#[should_panic(expected = "cert exists")]
fn double_cert_rejected() {
    let (env, id, _cnn, _platform, _validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let artist = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[7u8; 32]);
    let claim = BytesN::from_array(&env, &[8u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    client.propose_cert(&artist, &piece, &claim, &CERT_LIVING_AUTHOR, &required);
    client.propose_cert(&artist, &piece, &claim, &CERT_LIVING_AUTHOR, &required);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn mark_fee_from_non_platform_fails() {
    let (env, id, _cnn, _platform, _validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let artist = Address::generate(&env);
    let stranger = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[9u8; 32]);
    let claim = BytesN::from_array(&env, &[10u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    client.propose_cert(&artist, &piece, &claim, &CERT_LIVING_AUTHOR, &required);
    client.mark_fee_paid(&stranger, &piece);
}

#[test]
#[should_panic(expected = "not a required signer")]
fn wrong_role_signer_rejected() {
    let (env, id, _cnn, _platform, _validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let artist = Address::generate(&env);
    let impostor = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[17u8; 32]);
    let claim = BytesN::from_array(&env, &[18u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist);
    client.propose_cert(
        &impostor,
        &piece,
        &claim,
        &CERT_LIVING_AUTHOR,
        &required,
    );
    client.cosign(&impostor, &piece);
}

#[test]
#[should_panic(expected = "already finalized")]
fn finalize_twice_rejected() {
    let (env, id, _cnn, platform, _validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let artist = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[19u8; 32]);
    let claim = BytesN::from_array(&env, &[20u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    client.propose_cert(&artist, &piece, &claim, &CERT_LIVING_AUTHOR, &required);
    client.cosign(&artist, &piece);
    client.mark_fee_paid(&platform, &piece);
    client.finalize_cert(&artist, &piece);
    client.finalize_cert(&artist, &piece);
}
