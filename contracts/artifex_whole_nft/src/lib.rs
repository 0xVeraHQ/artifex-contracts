#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, BytesN, Env, Vec,
};

/// Delivery / sale modality for whole-work marketplace listings.
pub const MODALITY_DIGITAL: u32 = 1;
pub const MODALITY_PHYSICAL: u32 = 2;

#[contracttype]
#[derive(Clone)]
pub struct Nft {
    pub piece_id: BytesN<32>,
    pub owner: Address,
    pub certified: bool,
    pub delivery_clause: bool,
    pub delivery_req_hash: Option<BytesN<32>>,
}

/// Escrowed whole-NFT sale: NFT held by this contract until buy or cancel.
#[contracttype]
#[derive(Clone)]
pub struct Sale {
    pub sale_id: u64,
    pub token_id: u64,
    pub seller: Address,
    /// Accepted settlement SACs (XLM and/or USDC). Buyer selects one at buy.
    pub payment_tokens: Vec<Address>,
    /// Price for the whole NFT per accepted token (parallel to `payment_tokens`).
    pub prices: Vec<i128>,
    pub modality: u32,
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Token(u64),
    NextId,
    PieceToken(BytesN<32>),
    NextSale,
    Sale(u64),
    /// Active sale id for a token (cleared when sale ends).
    TokenSale(u64),
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
        env.storage().instance().set(&DataKey::NextSale, &1u64);
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
        if env.storage().persistent().has(&DataKey::TokenSale(token_id)) {
            panic!("listed");
        }
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

    /// List whole NFT for sale: escrow into this contract; buyer pays + receives in one invoke.
    /// `payment_tokens` / `prices` must be same non-empty length (XLM and/or USDC SACs).
    pub fn list_for_sale(
        env: Env,
        owner: Address,
        token_id: u64,
        payment_tokens: Vec<Address>,
        prices: Vec<i128>,
        modality: u32,
    ) -> u64 {
        owner.require_auth();
        if modality != MODALITY_DIGITAL && modality != MODALITY_PHYSICAL {
            panic!("bad modality");
        }
        if payment_tokens.is_empty() || payment_tokens.len() != prices.len() {
            panic!("bad tokens");
        }
        for p in prices.iter() {
            if p <= 0 {
                panic!("bad amounts");
            }
        }
        let n = payment_tokens.len();
        for i in 0..n {
            for j in (i + 1)..n {
                if payment_tokens.get(i).unwrap() == payment_tokens.get(j).unwrap() {
                    panic!("dup token");
                }
            }
        }
        if env.storage().persistent().has(&DataKey::TokenSale(token_id)) {
            panic!("already listed");
        }
        let mut nft: Nft = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .expect("no token");
        if nft.owner != owner {
            panic!("not owner");
        }
        let escrow = env.current_contract_address();
        nft.owner = escrow;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &nft);

        let sid: u64 = env.storage().instance().get(&DataKey::NextSale).unwrap();
        env.storage().instance().set(&DataKey::NextSale, &(sid + 1));
        let sale = Sale {
            sale_id: sid,
            token_id,
            seller: owner.clone(),
            payment_tokens,
            prices,
            modality,
            active: true,
        };
        env.storage().persistent().set(&DataKey::Sale(sid), &sale);
        env.storage()
            .persistent()
            .set(&DataKey::TokenSale(token_id), &sid);
        env.events()
            .publish((symbol_short!("listed"), owner), (sid, token_id));
        sid
    }

    /// Atomic buy: pay seller in chosen accepted currency and receive the NFT.
    pub fn buy_sale(env: Env, buyer: Address, sale_id: u64, payment_token: Address) {
        buyer.require_auth();
        let mut sale: Sale = env
            .storage()
            .persistent()
            .get(&DataKey::Sale(sale_id))
            .expect("no sale");
        if !sale.active {
            panic!("inactive");
        }
        let mut price: Option<i128> = None;
        for i in 0..sale.payment_tokens.len() {
            if sale.payment_tokens.get(i).unwrap() == payment_token {
                price = Some(sale.prices.get(i).unwrap());
                break;
            }
        }
        let price = match price {
            Some(p) => p,
            None => panic!("token not accepted"),
        };
        let pay = token::Client::new(&env, &payment_token);
        pay.transfer(&buyer, &sale.seller, &price);

        let mut nft: Nft = env
            .storage()
            .persistent()
            .get(&DataKey::Token(sale.token_id))
            .expect("no token");
        nft.owner = buyer.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Token(sale.token_id), &nft);

        sale.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Sale(sale_id), &sale);
        env.storage()
            .persistent()
            .remove(&DataKey::TokenSale(sale.token_id));
        env.events().publish(
            (symbol_short!("sold"), buyer),
            (sale_id, sale.token_id),
        );
    }

    /// Seller cancels and recovers the escrowed NFT.
    pub fn cancel_sale(env: Env, seller: Address, sale_id: u64) {
        seller.require_auth();
        let mut sale: Sale = env
            .storage()
            .persistent()
            .get(&DataKey::Sale(sale_id))
            .expect("no sale");
        if sale.seller != seller {
            panic!("not seller");
        }
        if !sale.active {
            panic!("inactive");
        }
        let mut nft: Nft = env
            .storage()
            .persistent()
            .get(&DataKey::Token(sale.token_id))
            .expect("no token");
        nft.owner = seller.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Token(sale.token_id), &nft);
        sale.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Sale(sale_id), &sale);
        env.storage()
            .persistent()
            .remove(&DataKey::TokenSale(sale.token_id));
        env.events()
            .publish((symbol_short!("sale_can"), seller), sale_id);
    }

    pub fn get_sale(env: Env, sale_id: u64) -> Sale {
        env.storage()
            .persistent()
            .get(&DataKey::Sale(sale_id))
            .expect("no sale")
    }

    pub fn sale_of_token(env: Env, token_id: u64) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::TokenSale(token_id))
            .expect("no sale")
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
