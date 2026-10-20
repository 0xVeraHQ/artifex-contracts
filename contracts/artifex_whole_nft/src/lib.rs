#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
};

#[contracttype]
#[derive(Clone)]
pub struct Nft {
    pub piece_id: BytesN<32>,
    pub owner: Address,
    pub certified: bool,
    pub delivery_clause: bool,
    pub delivery_req_hash: Option<BytesN<32>>,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Token(u64),
    NextId,
    PieceToken(BytesN<32>),
    Inited,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Inited) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextId, &1u64);
        env.storage().instance().set(&DataKey::Inited, &true);
    }

    pub fn mint(
        env: Env,
        minter: Address,
        piece_id: BytesN<32>,
        to: Address,
        certified: bool,
        delivery_clause: bool,
    ) -> u64 {
        minter.require_auth();
        if env
            .storage()
            .persistent()
            .has(&DataKey::PieceToken(piece_id.clone()))
        {
            panic!("piece already minted");
        }
        let id: u64 = env.storage().instance().get(&DataKey::NextId).unwrap();
        env.storage().instance().set(&DataKey::NextId, &(id + 1));
        let nft = Nft {
            piece_id: piece_id.clone(),
            owner: to.clone(),
            certified,
            delivery_clause,
            delivery_req_hash: None,
        };
        env.storage().persistent().set(&DataKey::Token(id), &nft);
        env.storage()
            .persistent()
            .set(&DataKey::PieceToken(piece_id), &id);
        env.events()
            .publish((symbol_short!("minted"), to), id);
        id
    }

    pub fn transfer(env: Env, from: Address, to: Address, token_id: u64) {
        from.require_auth();
        let mut nft: Nft = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .expect("no token");
        if nft.owner != from {
            panic!("not owner");
        }
        nft.owner = to.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &nft);
        env.events()
            .publish((symbol_short!("xfer"), to), token_id);
    }

    pub fn set_delivery_clause(
        env: Env,
        owner: Address,
        token_id: u64,
        clause: bool,
        req_hash: Option<BytesN<32>>,
    ) {
        owner.require_auth();
        let mut nft: Nft = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .expect("no token");
        if nft.owner != owner {
            panic!("not owner");
        }
        nft.delivery_clause = clause;
        nft.delivery_req_hash = req_hash;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &nft);
        env.events()
            .publish((symbol_short!("deliv"), token_id), clause);
    }

    pub fn owner_of(env: Env, token_id: u64) -> Address {
        let nft: Nft = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .expect("no token");
        nft.owner
    }

    pub fn get_nft(env: Env, token_id: u64) -> Nft {
        env.storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .expect("no token")
    }

    pub fn token_of_piece(env: Env, piece_id: BytesN<32>) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::PieceToken(piece_id))
            .expect("no mint")
    }
}

mod test;
