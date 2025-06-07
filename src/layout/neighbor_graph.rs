use std::collections::HashMap;

use smithay::desktop::Window;

use crate::layout::Direction;

#[derive(Debug, Clone)]
pub struct NeighborGraph {
    edges: HashMap<Window, HashMap<Direction, Vec<Window>>>,
}

impl NeighborGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new()
        }
    }

    pub fn get(&self, window: &Window, direction: &Direction) -> Option<&Vec<Window>> {
        self.edges.get(window)?.get(direction)
    }

    pub fn get_mut(&mut self, window: &Window, direction: &Direction) -> Option<&mut Vec<Window>> {
        self.edges.get_mut(window)?.get_mut(direction)
    }
    
    pub fn add_window(&mut self, from: Window, direction: Direction, to: Window) {
        self.edges.entry(from).or_default().entry(direction).or_default().push(to);
    }

    pub fn remove_window(&mut self, from: Window, direction: Direction, to: &Window) {
        if let Some(dir_map) = self.edges.get_mut(&from) {
            if let Some(vec) = dir_map.get_mut(&direction) {
                vec.retain(|win| win != to);
                if vec.is_empty() {
                    dir_map.remove(&direction);
                }
            }
            if dir_map.is_empty() {
                self.edges.remove(&from);
            }
        }
    }

    pub fn add_bidirectional(&mut self, from: Window, dir: Direction, to: Window) {
        self.add_window(from.clone(), dir.clone(), to.clone());
        self.add_window(to, dir.opposite(), from);
    }
    
}