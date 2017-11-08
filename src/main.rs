#![feature(plugin)]
#![plugin(rocket_codegen)]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate bincode;
extern crate chrono;
extern crate crypto;
extern crate uuid;

use bincode::{serialize, Infinite};
use chrono::prelude::*;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use rocket::State;
use rocket::response::status;
use rocket_contrib::{Json, Value};
use reqwest::{Client, Error};
use uuid::Uuid;

use std::collections::HashSet;
use std::sync::RwLock;

struct Application {
    node_identifier: String,
    blockchain: RwLock<Blockchain>,
}

type Chain = Vec<Block>;

fn main() {
    let node_id = Uuid::new_v4().hyphenated().to_string();

    let app = Application {
        node_identifier: node_id,
        blockchain: RwLock::new(Blockchain::new()),
    };

    rocket::ignite()
        .mount(
            "/",
            routes![chain, node_info, node_consensus, node_register, mine, new_transaction],
        )
        .manage(app)
        .launch();
}

#[get("/node/info", format = "application/json")]
fn node_info(state: State<Application>) -> Json<Value> {
    Json(json!({
        "id": state.node_identifier
    }))
}

#[post("/node/register", format = "application/json", data = "<node>")]
fn node_register(state: State<Application>, node: Json<Node>) -> status::Created<Json<Value>> {
    let n_node = node.into_inner();
    let idx = state.blockchain.write().unwrap().node_manager.add_node(n_node);
    status::Created(
        format!("/node/{}", idx),
        Some(Json(json!({
            "message": format!("Added new node #{}.", idx)
        }))),
    )
}

#[post("/node/resolve", format = "application/json")]
fn node_consensus(state: State<Application>) -> Json<Value> {
    let consensus = state.blockchain.write().unwrap().resolve_conflicts();
    let local_chain = &state.blockchain.read().unwrap().chain;

    if consensus {
      Json(json!({
        "message": "Local chain has been replaced.",
        "chain": local_chain
      }))
    } else {
      Json(json!({
        "message": "Local chain is authoritative.",
        "chain" : local_chain
      }))
    }
}

#[post("/mine", format = "application/json")]
fn mine(state: State<Application>) -> Json<Block> {
    let n_block = state
        .blockchain
        .write()
        .unwrap()
        .mine(state.node_identifier.as_ref());
    Json(n_block)
}

#[post("/transaction", format = "application/json", data = "<transaction>")]
fn new_transaction(
    state: State<Application>,
    transaction: Json<Transaction>,
) -> status::Created<Json<Value>> {
    let new_t: Transaction = transaction.into_inner();
    let idx = state.blockchain.write().unwrap().new_transaction(
        new_t.sender,
        new_t.recipient,
        new_t.amount,
    );

    status::Created(
        "/chain".to_owned(),
        Some(Json(json!({
            "message": format!("Added new transaction to block #{}.", idx)
        }))),
    )
}

#[get("/chain", format = "application/json")]
fn chain(state: State<Application>) -> Json<Chain> {
    // FIXME : Clone should not be needed here.
    let chain = state.blockchain.read().unwrap().chain.clone();
    Json(chain)
}

struct NodeManager {
    client: Client,
    nodes: HashSet<Node>,
}

impl NodeManager {
    fn new() -> NodeManager {
        NodeManager {
            client: Client::new(),
            nodes: HashSet::new(),
        }
    }

    fn add_node(&mut self, node: Node) -> u32 {
        self.nodes.insert(node);
        return self.nodes.len() as u32;
    }

    fn get_chains(&self) -> Vec<Chain> {
        let chains = self.nodes
            .iter()
            .map(|node| self.get_node_chain(node))
            .filter_map(Result::ok)
            .collect();

        chains
    }

    fn get_node_chain(&self, node: &Node) -> Result<Chain, Error> {
        let chain_uri = format!("http://{}/chain", node.address);
        let res = self.client.get(&chain_uri).send()?.json()?;
        Ok(res)
    }
}

// TODO : Move client and nodes into a NodeManager Struct/Impl.
struct Blockchain {
    chain: Chain,
    pending_transactions: Vec<Transaction>,
    node_manager: NodeManager,
}

impl Blockchain {
    const ORIGIN_SENDER: &'static str = "0";

    fn new() -> Blockchain {
        let mut blockchain = Blockchain {
            chain: Vec::new(),
            pending_transactions: Vec::new(),
            node_manager: NodeManager::new(),
        };

        // Create Genesis block
        blockchain.new_block(100, Some("1".to_owned()));
        return blockchain;
    }

    fn mine(&mut self, node_uuid: &str) -> Block {
        let last_proof = self.last_block().proof;
        let proof = Blockchain::proof_of_work(last_proof);

        // Pay the current node for mining
        self.new_transaction(
            String::from(Blockchain::ORIGIN_SENDER),
            String::from(node_uuid),
            1,
        );

        return self.new_block(proof, None);
    }

    fn new_block(&mut self, proof: u64, previous_hash: Option<String>) -> Block {
        let previous_hash = previous_hash.unwrap_or_else(|| Blockchain::hash(self.last_block()));
        let block = Block {
            index: (self.chain.len() as u32) + 1,
            timestamp: Utc::now().timestamp(),
            transactions: self.pending_transactions.clone(),
            proof,
            previous_hash,
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

    fn resolve_conflicts(&mut self) -> bool {
        // Get and verify the chain from all other nodes
        let new_chain = self.node_manager
            .get_chains()
            .into_iter()
            .find(|chain| {
                chain.len() > self.chain.len() &&
                    Blockchain::validate_chain(&chain)
            });

        if let Some(c) = new_chain {
            self.chain = c.to_owned();
            true
        } else {
            false
        }
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

    fn validate_chain(chain: &Vec<Block>) -> bool {
        chain.iter().zip(&chain[1..]).all(|(a, b)| -> bool {
            Blockchain::hash(a) == b.previous_hash &&
                Blockchain::valid_proof(b.proof, a.proof)
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Block {
    index: u32,
    timestamp: i64,
    transactions: Vec<Transaction>,
    proof: u64,
    previous_hash: String,
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
    amount: u64,
}

impl Transaction {
    pub fn new(sender: String, recipient: String, amount: u64) -> Transaction {
        Transaction {
            sender,
            recipient,
            amount,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
struct Node {
    address: String,
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
            previous_hash: "1".to_owned(),
        };

        let block2 = block.clone();

        assert_eq!(Blockchain::hash(&block), Blockchain::hash(&block2));
    }

    #[test]
    fn hash_blockchain_variability() {
        let mut block = Block {
            index: 1,
            timestamp: Utc::now().timestamp(),
            transactions: Vec::new(),
            proof: 100,
            previous_hash: "1".to_owned(),
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

    #[test]
    fn validate_chain() {
        let node_uuid = "1";

        // Valid chain check
        let mut blockchain = Blockchain::new();
        blockchain.new_transaction("alice".to_owned(), "bob".to_owned(), 10);
        blockchain.mine(node_uuid);
        blockchain.new_transaction("alice".to_owned(), "bob".to_owned(), 15);
        blockchain.mine(node_uuid);
        assert!(Blockchain::validate_chain(&blockchain.chain), "Chain should be valid.");

        // Invalid proof chain check
        let mut invalid_proof_chain = blockchain.chain.to_vec();
        invalid_proof_chain[1].proof = 0;
        assert!(!Blockchain::validate_chain(&invalid_proof_chain), "Should not validate incorrect proof.");

        // Invalid hash check
        let mut invalid_hash_chain = blockchain.chain.to_vec();
        invalid_hash_chain[1].previous_hash = "invalidhash".to_owned();
        assert!(!Blockchain::validate_chain(&invalid_hash_chain), "Should not invalidate incorrect hash chain.")
    }
}
