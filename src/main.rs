#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

extern crate chrono;
extern crate crypto;

use chrono::prelude::*;
use crypto::digest::Digest;
use crypto::sha2::Sha256;

fn main() {
    println!("Hello, world!");
}

struct Blockchain {
    chain: Vec<Block>,
    pending_transactions: Vec<Transaction>
}

impl Blockchain {

    fn new_block(&mut self, proof: u64) -> Block {
        let previous_hash = Blockchain::hash(self.last_block());
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
        return self.chain.last_mut().unwrap();
    }

    fn proof_of_work(&self, last_proof: u64) -> u64 {
        let mut proof = 0;
        while !(Blockchain::valid_proof(last_proof, proof)) {
            proof += 1;
        }
        return proof;
    }

    fn valid_proof(last_proof: u64, proof: u64) -> bool {
        let guess = format!("{}{}", last_proof, proof);
        let mut sha = Sha256::new();
        sha.input_str(&guess);
        return sha.result_str().ends_with("0000");
    }

    fn hash(block: &Block) -> String {
        let block_json = serde_json::to_string(block).unwrap();

        // Create Sha256 of JSON serialized block
        let mut sha = Sha256::new();
        sha.input_str(&block_json);
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
