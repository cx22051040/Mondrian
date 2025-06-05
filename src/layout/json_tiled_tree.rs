use std::fs;

use serde::{Deserialize, Serialize};

use super::{Direction, tiled_tree::TiledTree};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum JsonNode {
    Leaf {
        app_id: String,
    },
    Split {
        direction: Direction,
        offset: (i32, i32),
        left: Box<JsonNode>,
        right: Box<JsonNode>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct JsonTree {
    tiled_tree: JsonNode,
}

impl From<TiledTree> for JsonTree {
    fn from(_value: TiledTree) -> Self {
        todo!()
    }
}

impl JsonTree {
    pub fn from_json(path: &str) -> Option<Self> {
        match fs::read_to_string(path) {
            Ok(data) => {
                match serde_json::from_str(&data) {
                    Ok(tree) => {
                        Some(tree)
                    }
                    Err(err) => {
                        warn!("Failed to deserialize JSON from {}: {:?}", path, err);
                        None
                    }
                }
            }
            Err(err) => {
                warn!("Failed to read file: {} with err: {:?}", path, err);
                None
            }
        }
    }

    pub fn _to_json(&self, path: &str) {
        match fs::write(path, serde_json::to_string(self).expect("Failed to serialize JSON")) {
            Ok(_) => info!("Successfully wrote JSON to {}", path),
            Err(err) => warn!("Failed to write file: {} with err: {:?}", path, err),
        }
    }

    pub fn print_tree(&self) {
        fn print(node: &JsonNode, depth: usize) {
            let indent = "  ".repeat(depth);
            match node {
                JsonNode::Leaf { app_id } => tracing::info!("{indent}- Leaf: {:?}", app_id),
                JsonNode::Split { left, right, .. } => {
                    tracing::info!("{indent}- Split:");
                    print(left, depth + 1);
                    print(right, depth + 1);
                }
            }
        }
        print(&self.tiled_tree, 1);
    }
}