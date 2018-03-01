#![feature(plugin)]
#![plugin(rocket_codegen)]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

extern crate bincode;
extern crate crypto;
extern crate chrono;
extern crate pnet;
extern crate uuid;

use rocket::State;
use rocket::response::status;
use rocket_contrib::{Json, Value};
use uuid::Uuid;
use pnet::datalink::{self, NetworkInterface};

use std::env;
use std::sync::RwLock;

mod core;

use core::blockchain::{Block, Blockchain, Chain, Transaction};
use core::nodemanager::{Node, P2PNodeManager};

struct Application {
    node_identifier: String,
    blockchain: RwLock<Blockchain>,
}

fn main() {
    let node_id = Uuid::new_v4().hyphenated().to_string();

    let interface_name = env::args().nth(1).unwrap();
    let local_ip = iface_ip(&interface_name);

    let local = Node::new(format!("{}:{}", local_ip.unwrap(), 8000));

    let node_manager = P2PNodeManager::new(local);

    let app = Application {
        node_identifier: node_id,
        blockchain: RwLock::new(Blockchain::new(node_manager)),
    };

    rocket::ignite()
        .mount(
            "/",
            routes![chain, nodes, node_info, node_consensus, node_register, mine, new_transaction],
        )
        .manage(app)
        .launch();
}

#[get("/nodes", format = "application/json")]
fn nodes(state: State<Application>) -> Json<Vec<Node>> {
    let nodes = state.blockchain.read().unwrap().node_manager.get_nodes();
    Json(nodes)
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
    let idx = state.blockchain.write().unwrap().node_manager.add_node(n_node).unwrap();
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

fn iface_ip(interface_name: &str) -> Option<String> {
    let interface_names_match =
        |iface: &NetworkInterface| iface.name == interface_name;

    // Find the network interface with the provided name
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter()
        .filter(interface_names_match)
        .next()
        .unwrap();

    let local_ip = interface.ips.iter()
        .find(|ip| ip.is_ipv4());
    local_ip.map(|ip| format!("{}", ip.ip()))
}
