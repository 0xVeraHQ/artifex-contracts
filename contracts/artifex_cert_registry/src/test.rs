#![cfg(test)]

use super::*;
use artifex_connoisseur::{Contract as Cnn, ContractClient as CnnClient};
use artifex_fee_collector::{Contract as Fee, ContractClient as FeeClient};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, Vec,
};

fn setup_env() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let validator = Address::generate(&env);
    // Real fee collector so finalize's has_deposit check does not hit MissingValue.
    let usdc = env
        .register_stellar_asset_contract_v2(platform.clone())
        .address();
    let fee_c = env.register(Fee, ());
    FeeClient::new(&env, &fee_c).init(&platform, &usdc, &50_000_000, &10_000_000);
    let cnn_id = env.register(Cnn, ());
    let cnn = CnnClient::new(&env, &cnn_id);
    cnn.init(&platform);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&platform, &validator, &fee_c, &cnn_id);
    FeeClient::new(&env, &fee_c).set_consumer(&platform, &id);
    (env, id, cnn_id, platform, validator, fee_c)
}

/// Full stack: real fee collector + token SAC for deposit/finalize/refund paths.
fn setup_with_fee() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    let platform = Address::generate(&env);
    let validator = Address::generate(&env);
    let usdc = env
        .register_stellar_asset_contract_v2(platform.clone())
        .address();
    let xlm = env
        .register_stellar_asset_contract_v2(platform.clone())
        .address();
    let fee_id = env.register(Fee, ());
    let fee = FeeClient::new(&env, &fee_id);
    fee.init(&platform, &usdc, &50_000_000, &10_000_000);
    let cnn_id = env.register(Cnn, ());
    CnnClient::new(&env, &cnn_id).init(&platform);
    let cert_id = env.register(Contract, ());
    let cert = ContractClient::new(&env, &cert_id);
    cert.init(&platform, &validator, &fee_id, &cnn_id);
    fee.set_consumer(&platform, &cert_id);
    (env, cert_id, cnn_id, platform, validator, fee_id, xlm, usdc)
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
    let (env, id, cnn_id, _platform, validator, _fee) = setup_env();
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
    // validator never signs — finalize rejects before any fee consume
    client.finalize_cert(&connoisseur, &piece);
}

#[test]
#[should_panic(expected = "missing signature")]
fn mark_fee_rejected_until_signatures_ready() {
    let (env, id, cnn_id, platform, validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let cnn = CnnClient::new(&env, &cnn_id);
    let connoisseur = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[21u8; 32]);
    let claim = BytesN::from_array(&env, &[22u8; 32]);
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
    // only one of two sigs — mark_fee must fail (fee only when finalize-ready)
    client.mark_fee_paid(&platform, &piece);
}

#[test]
#[should_panic(expected = "connoisseur not authorized")]
fn connoisseur_without_auth_cannot_finalize() {
    let (env, id, _cnn_id, _platform, validator, _fee) = setup_env();
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
    client.cosign(&artist, &piece);
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

#[test]
fn deposit_then_finalize_consumes_fee() {
    let (env, cert_id, _cnn, _platform, _validator, fee_id, xlm, _usdc) = setup_with_fee();
    let cert = ContractClient::new(&env, &cert_id);
    let fee = FeeClient::new(&env, &fee_id);
    let artist = Address::generate(&env);
    StellarAssetClient::new(&env, &xlm).mint(&artist, &100_000_000);
    let piece = BytesN::from_array(&env, &[30u8; 32]);
    let claim = BytesN::from_array(&env, &[31u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    cert.propose_cert(&artist, &piece, &claim, &CERT_LIVING_AUTHOR, &required);
    cert.cosign(&artist, &piece);
    fee.deposit_xlm(&artist, &xlm, &piece);
    assert!(fee.has_deposit(&piece));
    cert.finalize_cert(&artist, &piece);
    assert!(cert.is_finalized(&piece));
    assert!(!fee.has_deposit(&piece));
    let c = cert.get_cert(&piece);
    assert!(c.fee_paid);
}

#[test]
fn deposit_refundable_when_finalize_blocked() {
    let (env, cert_id, cnn_id, platform, validator, fee_id, xlm, _usdc) = setup_with_fee();
    let cert = ContractClient::new(&env, &cert_id);
    let fee = FeeClient::new(&env, &fee_id);
    let cnn = CnnClient::new(&env, &cnn_id);
    let connoisseur = Address::generate(&env);
    StellarAssetClient::new(&env, &xlm).mint(&connoisseur, &100_000_000);
    let piece = BytesN::from_array(&env, &[32u8; 32]);
    let claim = BytesN::from_array(&env, &[33u8; 32]);
    cnn.authorize(&connoisseur, &piece);
    let mut required = Vec::new(&env);
    required.push_back(connoisseur.clone());
    required.push_back(validator.clone());
    cert.propose_cert(
        &connoisseur,
        &piece,
        &claim,
        &CERT_DECEASED_CONNOISSEUR,
        &required,
    );
    cert.cosign(&connoisseur, &piece);
    // deposit before second sig — finalize will fail; deposit must remain refundable
    fee.deposit_xlm(&connoisseur, &xlm, &piece);
    let before = TokenClient::new(&env, &xlm).balance(&connoisseur);
    // Attempt finalize — expect panic; catch via should_panic sibling. Here we refund instead.
    assert!(fee.has_deposit(&piece));
    fee.refund_deposit(&connoisseur, &piece, &xlm);
    assert!(!fee.has_deposit(&piece));
    let after = TokenClient::new(&env, &xlm).balance(&connoisseur);
    assert_eq!(after, before + 50_000_000);
    // still not finalized
    assert!(!cert.is_finalized(&piece));
    let _ = platform; // keep for clarity that platform could also refund
}

#[test]
#[should_panic(expected = "missing signature")]
fn deposit_not_consumed_when_finalize_missing_sig() {
    let (env, cert_id, cnn_id, _platform, validator, fee_id, xlm, _usdc) = setup_with_fee();
    let cert = ContractClient::new(&env, &cert_id);
    let fee = FeeClient::new(&env, &fee_id);
    let cnn = CnnClient::new(&env, &cnn_id);
    let connoisseur = Address::generate(&env);
    StellarAssetClient::new(&env, &xlm).mint(&connoisseur, &100_000_000);
    let piece = BytesN::from_array(&env, &[34u8; 32]);
    let claim = BytesN::from_array(&env, &[35u8; 32]);
    cnn.authorize(&connoisseur, &piece);
    let mut required = Vec::new(&env);
    required.push_back(connoisseur.clone());
    required.push_back(validator.clone());
    cert.propose_cert(
        &connoisseur,
        &piece,
        &claim,
        &CERT_DECEASED_CONNOISSEUR,
        &required,
    );
    cert.cosign(&connoisseur, &piece);
    fee.deposit_xlm(&connoisseur, &xlm, &piece);
    // missing validator sig — must panic before consume
    cert.finalize_cert(&connoisseur, &piece);
}

#[test]
fn prior_digitization_validator_may_substitute() {
    let (env, id, _cnn, platform, validator, _fee) = setup_env();
    let client = ContractClient::new(&env, &id);
    let artist = Address::generate(&env);
    let piece = BytesN::from_array(&env, &[36u8; 32]);
    let claim = BytesN::from_array(&env, &[37u8; 32]);
    let mut required = Vec::new(&env);
    required.push_back(artist.clone());
    required.push_back(validator.clone());
    client.propose_cert(
        &artist,
        &piece,
        &claim,
        &CERT_PRIOR_DIGITIZATION,
        &required,
    );
    // Only validator signs — substitutes for missing artist under prior digitization.
    client.cosign(&validator, &piece);
    assert!(client.is_ready(&piece));
    client.mark_fee_paid(&platform, &piece);
    client.finalize_cert(&artist, &piece);
    assert!(client.is_finalized(&piece));
}
