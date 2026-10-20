#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

#[test]
fn mint_transfer_delivery() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let artist = Address::generate(&env);
    let buyer = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&admin);
    let piece = BytesN::from_array(&env, &[9u8; 32]);
    let tid = client.mint(&artist, &piece, &artist, &true, &true);
    assert_eq!(client.owner_of(&tid), artist);
    client.transfer(&artist, &buyer, &tid);
    assert_eq!(client.owner_of(&tid), buyer);
    let hash = BytesN::from_array(&env, &[1u8; 32]);
    client.set_delivery_clause(&buyer, &tid, &true, &Some(hash));
    let nft = client.get_nft(&tid);
    assert!(nft.delivery_clause);
    assert_eq!(client.token_of_piece(&piece), tid);
}

#[test]
#[should_panic(expected = "not owner")]
fn transfer_nft_non_owner_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let artist = Address::generate(&env);
    let thief = Address::generate(&env);
    let buyer = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&admin);
    let piece = BytesN::from_array(&env, &[2u8; 32]);
    let tid = client.mint(&artist, &piece, &artist, &true, &false);
    client.transfer(&thief, &buyer, &tid);
}

#[test]
#[should_panic(expected = "piece already minted")]
fn double_mint_same_piece_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let artist = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&admin);
    let piece = BytesN::from_array(&env, &[3u8; 32]);
    client.mint(&artist, &piece, &artist, &true, &false);
    client.mint(&artist, &piece, &artist, &true, &false);
}

#[test]
#[should_panic(expected = "not owner")]
fn non_owner_cannot_set_delivery() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let artist = Address::generate(&env);
    let other = Address::generate(&env);
    let id = env.register(Contract, ());
    let client = ContractClient::new(&env, &id);
    client.init(&admin);
    let piece = BytesN::from_array(&env, &[4u8; 32]);
    let tid = client.mint(&artist, &piece, &artist, &true, &false);
    client.set_delivery_clause(&other, &tid, &true, &None);
}
