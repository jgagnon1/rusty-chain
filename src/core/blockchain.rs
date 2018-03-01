use bincode::{serialize, Infinite};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use chrono::prelude::*;

use core::nodemanager::P2PNodeManager;

pub type Chain = Vec<Block>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    index: u32,
    timestamp: i64,
    transactions: Vec<Transaction>,
    proof: u64,
    previous_hash: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
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

pub struct Blockchain {
    pub chain: Chain,
    pub node_manager: P2PNodeManager,
    pending_transactions: Vec<Transaction>,
}

impl Blockchain {
    const ORIGIN_SENDER: &'static str = "0";
    const ORIGIN_HASH: &'static str = "1";

    pub fn new(node_manager: P2PNodeManager) -> Blockchain {
        let mut blockchain = Blockchain {
            chain: Vec::new(),
            pending_transactions: Vec::new(),
            node_manager
        };

        // Create Genesis block
        blockchain.new_block(100, Some(Blockchain::ORIGIN_HASH));
        return blockchain;
    }

    pub fn mine(&mut self, node_uuid: &str) -> Block {
        let last_proof = self.last_block().proof;
        let proof = Blockchain::proof_of_work(last_proof);

        // Pay the current node for mining
        self.new_transaction(
            String::from(Blockchain::ORIGIN_SENDER),
            String::from(node_uuid),
            1,
        );

        self.new_block(proof, None)
    }

    fn new_block(&mut self, proof: u64, previous_hash: Option<&str>) -> Block {
        let hash = previous_hash.map(|s| s.into()).unwrap_or_else(|| Blockchain::hash(self.last_block()));
        let block = Block {
            index: (self.chain.len() as u32) + 1,
            timestamp: Utc::now().timestamp(),
            transactions: self.pending_transactions.clone(),
            proof,
            previous_hash: hash
        };

        // Clear transactions included in new block and push to chain
        self.pending_transactions.clear();
        self.chain.push(block.clone());

        block
    }

    pub fn new_transaction(&mut self, sender: String, recipient: String, amount: u64) -> u32 {
        self.pending_transactions.push(Transaction::new(sender, recipient, amount));
        self.chain.len() as u32
    }

    pub fn resolve_conflicts(&mut self) -> bool {
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


#[cfg(test)]
mod tests {
    use super::*;
    use core::nodemanager::Node;

    #[test]
    fn genesis_blockchain() {
        let blockchain = Blockchain::new(NODE_MANAGER);

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

        block.transactions.push(Transaction::new("alice".to_owned(), "bob".to_owned(), 10));

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
    fn validate_new_transaction() {
        let mut blockchain = Blockchain::new(NODE_MANAGER);
        blockchain.new_transaction("alice".to_owned(), "bob".to_owned(), 10);

        assert_eq!(blockchain.pending_transactions.len(), 1, "New transaction should be added to pending.")
    }

    #[test]
    fn validate_new_block() {
        let mut blockchain = Blockchain::new(NODE_MANAGER);
        blockchain.new_transaction("alice".to_owned(), "bob".to_owned(), 10);
        // Generate a block
        let new_block = blockchain.new_block(100, Some(&"1".to_owned()));

        assert_eq!(new_block.transactions.len(), 1, "New returned block should contain transaction.");
        assert_eq!(blockchain.pending_transactions.len(), 0, "Blockchain should be empty after new block generation.")
    }

    #[test]
    fn validate_chain() {
        let node_uuid = "1";

        // Valid chain check
        let mut blockchain = Blockchain::new(NODE_MANAGER);
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