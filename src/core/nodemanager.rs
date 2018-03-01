use reqwest::{Client, Error, Response};
use std::collections::HashSet;

use core::blockchain::Chain;

pub trait NodeManager {
    fn add_node(&mut self, remote: Node) -> Result<bool, Error>;

    fn get_nodes(&self) -> Vec<Node>;

    fn get_chains(&self) -> Vec<Chain>;
}

pub struct P2PNodeManager {
    local: Node,
    client: Client,
    nodes: HashSet<Node>,
}

impl P2PNodeManager {
    pub fn new(local: Node) -> P2PNodeManager {
        P2PNodeManager {
            local,
            client: Client::new(),
            nodes: HashSet::new(),
        }
    }

    fn register_remote(&self, remote: &Node) -> Result<Response, Error> {
        let register_uri = format!("http://{}/node/register", remote.address);
        let res = self.client
            .post(&register_uri)
            .json(&self.local)
            .send()?;
        Ok(res)
    }

    fn get_node_chain(&self, remote: &Node) -> Result<Chain, Error> {
        let chain_uri = format!("http://{}/chain", remote.address);
        let res = self.client.get(&chain_uri).send()?.json()?;
        Ok(res)
    }

    // FIXME : Remove those methods when switching to NodeManager

    pub fn add_node(&mut self, remote: Node) -> Result<bool, Error> {
        // FIXME : Avoid adding continuously cycle P2P
        if !self.nodes.contains(&remote) {
            self.register_remote(&remote).map(|_res| {
                self.nodes.insert(remote)
            })
        } else {
            info!("Node {:?} was already registered.", remote);
            Ok(false)
        }
    }

    pub fn get_nodes(&self) -> Vec<Node> {
        self.nodes
            .iter()
            .cloned()
            .filter(|n| n != &self.local)
            .collect::<Vec<Node>>()
    }

    pub fn get_chains(&self) -> Vec<Chain> {
        let chains = self.nodes
            .iter()
            .map(|node| self.get_node_chain(node))
            .filter_map(Result::ok)
            .collect();

        chains
    }
}

//impl NodeManager for P2PNodeManager {
//    fn add_node(&mut self, remote: Node) -> Result<bool, Error> {
//        // FIXME : Avoid adding continuously cycle P2P
//        if !self.nodes.contains(&remote) {
//            self.register_remote(&remote).map(|_res| {
//                self.nodes.insert(remote)
//            })
//        } else {
//            info!("Node {:?} was already registered.", remote);
//            Ok(false)
//        }
//    }
//
//    fn get_nodes(&self) -> Vec<Node> {
//        self.nodes
//            .iter()
//            .cloned()
//            .filter(|n| n != &self.local)
//            .collect::<Vec<Node>>()
//    }
//
//    fn get_chains(&self) -> Vec<Chain> {
//        let chains = self.nodes
//            .iter()
//            .map(|node| self.get_node_chain(node))
//            .filter_map(Result::ok)
//            .collect();
//
//        chains
//    }
//}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Node {
    address: String
}

impl Node {
    pub fn new(address: String) -> Node {
        Node { address }
    }
}