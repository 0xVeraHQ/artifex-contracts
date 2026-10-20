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
