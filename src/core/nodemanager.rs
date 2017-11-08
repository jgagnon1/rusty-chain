use reqwest::{Client, Error};
use std::collections::HashSet;

use core::blockchain::Chain;

pub struct NodeManager {
    client: Client,
    nodes: HashSet<Node>,
}

impl NodeManager {
    pub fn new() -> NodeManager {
        NodeManager {
            client: Client::new(),
            nodes: HashSet::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) -> u32 {
        self.nodes.insert(node);
        return self.nodes.len() as u32;
    }

    pub fn get_chains(&self) -> Vec<Chain> {
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

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Node {
    address: String,
}