#![feature(plugin)]
#![plugin(rocket_codegen)]
extern crate rocket;
#[macro_use] extern crate rocket_contrib;

#[macro_use] extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate bincode;

extern crate chrono;
extern crate crypto;

extern crate uuid;

use bincode::{serialize, Infinite};
use chrono::prelude::*;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use rocket::State;
use rocket_contrib::{Json, Value};
use uuid::Uuid;
use std::sync::RwLock;

struct Application {
    node_identifier: String,
    blockchain: RwLock<Blockchain>
}

fn main() {
    let node_id= Uuid::new_v4().hyphenated().to_string();

    let app = Application {
        node_identifier: node_id,
        blockchain: RwLock::new(Blockchain::new())
    };

    rocket::ignite()
        .mount("/", routes![chain, node_info, mine, new_transaction])
        .manage(app)
        .launch();
}

#[get("/node/info")]
fn node_info(state: State<Application>) -> Json<Value> {
    Json(json!({
        "id": state.node_identifier
    }))
}

#[post("/mine")]
fn mine(state: State<Application>) -> String {
    let n_block = state.blockchain.write().unwrap().mine(state.node_identifier.as_ref());
    serde_json::to_string(&n_block).unwrap()
}

#[post("/transaction", format = "application/json", data = "<transaction>")]
fn new_transaction(transaction: Json<Transaction>, state: State<Application>) -> &'static str {
    let new_t: Transaction = transaction.into_inner();
    state.blockchain.write().unwrap()
        .new_transaction(new_t.sender, new_t.recipient, new_t.amount);
    "Adding new transaction to current block."
}

#[get("/chain", format = "application/json")]
fn chain(state: State<Application>) -> String {
    serde_json::to_string(&state.blockchain.read().unwrap().chain).unwrap()
}

struct Blockchain {
    chain: Vec<Block>,
    pending_transactions: Vec<Transaction>
}

impl Blockchain {

    const ORIGIN_SENDER: &'static str = "0";

    fn new() -> Blockchain {
        let mut blockchain = Blockchain {
            chain: Vec::new(),
            pending_transactions: Vec::new()
        };

        // Create Genesis block
        blockchain.new_block(100, Some("1".to_owned()));
        return blockchain;
    }

    fn mine(&mut self, node_uuid: &str) -> Block {
        let last_proof = self.last_block().proof;
        let proof = Blockchain::proof_of_work(last_proof);

        // Pay the current node for mining
        self.new_transaction(String::from(Blockchain::ORIGIN_SENDER),String::from(node_uuid), 1);

        return self.new_block(proof, None);
    }

    fn new_block(&mut self, proof: u64, previous_hash: Option<String>) -> Block {
        let previous_hash = previous_hash.unwrap_or_else(|| Blockchain::hash(self.last_block()));
        let block = Block {
            index: (self.chain.len() as u32) + 1,
            timestamp: Utc::now().timestamp(),
            transactions: self.pending_transactions.clone(),
            proof,
            previous_hash
        };

        // Clear transactions included in new block and push to chain
        self.pending_transactions.clear();
        self.chain.push(block.clone());

        return block;
    }

    fn new_transaction(&mut self, sender: String, recipient: String, amount: u64) -> u32 {
        let l_block = self.last_block();
        l_block.append_transaction(Transaction::new(sender, recipient, amount));
        return l_block.index + 1;
    }

    fn last_block(&mut self) -> &mut Block {
        self.chain.last_mut().expect("Chain is empty of blocks.")
    }

    fn proof_of_work(last_proof: u64) -> u64 {
        let mut proof = 0;
        while !(Blockchain::valid_proof(last_proof, proof)) {
            proof += 1;
        }
        proof
    }

    fn valid_proof(last_proof: u64, proof: u64) -> bool {
        let guess = format!("{}", last_proof * proof);
        let mut sha = Sha256::new();
        sha.input_str(&guess);
        return sha.result_str().ends_with("0000");
    }

    // TODO : Consider changing signature to &static str
    fn hash(block: &Block) -> String {
        let ser_block = serialize(block, Infinite).unwrap();

        // Create Sha256 of JSON serialized block
        let mut sha = Sha256::new();
        sha.input(&ser_block);
        return sha.result_str();
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Block {
    index: u32,
    timestamp: i64,
    transactions: Vec<Transaction>,
    proof: u64,
    previous_hash: String
}

impl Block {
    fn append_transaction(&mut self, txn: Transaction) {
        self.transactions.push(txn);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Transaction {
    sender: String,
    recipient: String,
    amount: u64
}

impl Transaction {
    pub fn new(sender: String, recipient: String, amount: u64) -> Transaction {
        Transaction { sender, recipient, amount }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_blockchain() {
        let blockchain = Blockchain::new();

        assert_eq!(blockchain.chain.len() as u32, 1);
        assert_eq!(blockchain.pending_transactions.len() as u32, 0);
    }

    #[test]
    fn hash_blockchain_determinism() {
        let block = Block {
            index: 1,
            timestamp: Utc::now().timestamp(),
            transactions: Vec::new(),
            proof: 100,
            previous_hash: "1".to_owned()
        };

        assert_eq!(Blockchain::hash(&block), Blockchain::hash(&block));
    }

    #[test]
    fn hash_blockchain_variability() {
        let mut block = Block {
            index: 1,
            timestamp: Utc::now().timestamp(),
            transactions: Vec::new(),
            proof: 100,
            previous_hash: "1".to_owned()
        };

        let h1 = Blockchain::hash(&block);

        block.append_transaction(Transaction::new("alice".to_owned(), "bob".to_owned(), 10));

        let h2 = Blockchain::hash(&block);

        assert_ne!(h1, h2);
    }

    #[test]
    fn validate_proof() {
        let last_proof = 1;
        let valid = 31214; // from: Blockchain::proof_of_work(last_proof);

        assert!(Blockchain::valid_proof(last_proof, valid));
        assert!(!Blockchain::valid_proof(last_proof, valid - 1));
    }
}
