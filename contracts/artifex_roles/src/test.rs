#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Contract, ());
    let platform = Address::generate(&env);
    let client = ContractClient::new(&env, &contract_id);
    client.init(&platform);
    (env, contract_id, platform)
}

#[test]
fn init_assigns_platform_role() {
    let (env, id, platform) = setup();
    let client = ContractClient::new(&env, &id);
    assert_eq!(client.get_role(&platform), ROLE_PLATFORM);
    assert_eq!(client.platform(), platform);
}

#[test]
fn assign_and_revoke_role() {
    let (env, id, platform) = setup();
    let client = ContractClient::new(&env, &id);
    let artist = Address::generate(&env);
    client.assign_role(&platform, &artist, &ROLE_ARTIST);
    assert_eq!(client.get_role(&artist), ROLE_ARTIST);
    client.revoke_role(&platform, &artist);
    assert_eq!(client.get_role(&artist), ROLE_NONE);
}

#[test]
#[should_panic(expected = "platform only")]
fn non_platform_cannot_assign() {
    let (env, id, _platform) = setup();
    let client = ContractClient::new(&env, &id);
    let other = Address::generate(&env);
    let artist = Address::generate(&env);
    client.assign_role(&other, &artist, &ROLE_ARTIST);
}
