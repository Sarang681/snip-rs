use fnv::FnvHasher;
use fred::clients::Client;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub struct ConsistentHashRing {
    pub node_mappings: BTreeMap<u64, String>,
    pub clients: HashMap<String, Client>,
    pub virtual_nodes_per_real_node: u16,
}

impl ConsistentHashRing {
    pub fn new(virtual_nodes_per_real_node: u16) -> Self {
        Self {
            node_mappings: BTreeMap::new(),
            clients: HashMap::new(),
            virtual_nodes_per_real_node,
        }
    }

    pub fn add_node(&mut self, redis_client: Client, client_id: &str) {
        for i in 0..self.virtual_nodes_per_real_node {
            let virtual_client_id = format!("{}-#{}", client_id, i);
            let mut hasher = FnvHasher::default();
            virtual_client_id.hash(&mut hasher);
            let hash = hasher.finish();
            self.node_mappings.insert(hash, client_id.to_string());
        }
        self.clients.insert(client_id.to_string(), redis_client);
    }

    pub fn get_client(&self, key: &str) -> Option<Client> {
        let mut hasher = FnvHasher::default();
        key.hash(&mut hasher);
        let hashed_key = hasher.finish();

        let clients_map = &self.clients;

        let node_id = if let Some(mapping) = self.node_mappings.range(&hashed_key..).next() {
            mapping.1
        } else {
            clients_map.iter().next()?.0
        };

        clients_map.get(node_id).cloned()
    }
}
