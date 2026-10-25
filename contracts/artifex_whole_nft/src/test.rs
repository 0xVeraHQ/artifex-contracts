#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    vec, Address, BytesN, Env,
};

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

fn sale_setup(
    env: &Env,
) -> (
    Address,
    Address,
    Address,
    Address,
    Address,
    ContractClient<'_>,
    u64,
) {
    let admin = Address::generate(env);
    let seller = Address::generate(env);
    let buyer = Address::generate(env);
    let xlm = env.register_stellar_asset_contract_v2(admin.clone()).address();
    let usdc = env.register_stellar_asset_contract_v2(admin.clone()).address();
    StellarAssetClient::new(env, &xlm).mint(&buyer, &10_000_000_000);
    StellarAssetClient::new(env, &usdc).mint(&buyer, &10_000_000_000);
    let id = env.register(Contract, ());
    let client = ContractClient::new(env, &id);
    client.init(&admin);
    let piece = BytesN::from_array(env, &[42u8; 32]);
    let tid = client.mint(&seller, &piece, &seller, &true, &true);
    (admin, seller, buyer, xlm, usdc, client, tid)
}

#[test]
fn list_buy_whole_xlm_then_relist_usdc_cancel() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, xlm, usdc, client, tid) = sale_setup(&env);
    let tokens = vec![&env, xlm.clone()];
    let prices = vec![&env, 5_000_000i128];
    let sid = client.list_for_sale(
        &seller,
        &tid,
        &tokens,
        &prices,
        &MODALITY_DIGITAL,
    );
    assert_eq!(client.sale_of_token(&tid), sid);
    assert_ne!(client.owner_of(&tid), seller);
    assert_ne!(client.owner_of(&tid), buyer);

    client.buy_sale(&buyer, &sid, &xlm);
    assert_eq!(client.owner_of(&tid), buyer);
    assert_eq!(TokenClient::new(&env, &xlm).balance(&seller), 5_000_000);
    assert!(!client.get_sale(&sid).active);

    let tokens2 = vec![&env, usdc.clone()];
    let prices2 = vec![&env, 2_000_000i128];
    let sid2 = client.list_for_sale(
        &buyer,
        &tid,
        &tokens2,
        &prices2,
        &MODALITY_PHYSICAL,
    );
    assert_eq!(client.sale_of_token(&tid), sid2);
    client.cancel_sale(&buyer, &sid2);
    assert_eq!(client.owner_of(&tid), buyer);
    assert!(!client.get_sale(&sid2).active);
}

#[test]
fn list_accepts_both_currencies_buyer_picks_usdc() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, xlm, usdc, client, tid) = sale_setup(&env);
    let tokens = vec![&env, xlm.clone(), usdc.clone()];
    let prices = vec![&env, 5_000_000i128, 1_000_000i128];
    let sid = client.list_for_sale(
        &seller,
        &tid,
        &tokens,
        &prices,
        &MODALITY_PHYSICAL,
    );
    client.buy_sale(&buyer, &sid, &usdc);
    assert_eq!(client.owner_of(&tid), buyer);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&seller), 1_000_000);
    assert_eq!(TokenClient::new(&env, &xlm).balance(&seller), 0);
}

#[test]
#[should_panic(expected = "token not accepted")]
fn buy_wrong_currency_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, xlm, usdc, client, tid) = sale_setup(&env);
    let tokens = vec![&env, xlm.clone()];
    let prices = vec![&env, 5_000_000i128];
    let sid = client.list_for_sale(
        &seller,
        &tid,
        &tokens,
        &prices,
        &MODALITY_DIGITAL,
    );
    client.buy_sale(&buyer, &sid, &usdc);
}

#[test]
#[should_panic(expected = "not seller")]
fn non_seller_cannot_cancel_sale() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, xlm, _usdc, client, tid) = sale_setup(&env);
    let tokens = vec![&env, xlm.clone()];
    let prices = vec![&env, 5_000_000i128];
    let sid = client.list_for_sale(
        &seller,
        &tid,
        &tokens,
        &prices,
        &MODALITY_DIGITAL,
    );
    client.cancel_sale(&buyer, &sid);
}

#[test]
#[should_panic(expected = "inactive")]
fn buy_cancelled_sale_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, xlm, _usdc, client, tid) = sale_setup(&env);
    let tokens = vec![&env, xlm.clone()];
    let prices = vec![&env, 5_000_000i128];
    let sid = client.list_for_sale(
        &seller,
        &tid,
        &tokens,
        &prices,
        &MODALITY_DIGITAL,
    );
    client.cancel_sale(&seller, &sid);
    client.buy_sale(&buyer, &sid, &xlm);
}

#[test]
#[should_panic(expected = "listed")]
fn transfer_while_listed_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, buyer, xlm, _usdc, client, tid) = sale_setup(&env);
    let tokens = vec![&env, xlm.clone()];
    let prices = vec![&env, 5_000_000i128];
    client.list_for_sale(
        &seller,
        &tid,
        &tokens,
        &prices,
        &MODALITY_DIGITAL,
    );
    client.transfer(&seller, &buyer, &tid);
}

#[test]
#[should_panic(expected = "already listed")]
fn double_list_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, seller, _buyer, xlm, usdc, client, tid) = sale_setup(&env);
    let tokens = vec![&env, xlm.clone()];
    let prices = vec![&env, 5_000_000i128];
    client.list_for_sale(
        &seller,
        &tid,
        &tokens,
        &prices,
        &MODALITY_DIGITAL,
    );
    let tokens2 = vec![&env, usdc.clone()];
    let prices2 = vec![&env, 1i128];
    client.list_for_sale(
        &seller,
        &tid,
        &tokens2,
        &prices2,
        &MODALITY_DIGITAL,
    );
}
