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
    
    pub fn add_window(&mut self, from: Window, direction: Direction, to: Vec<Window>) {
        self.edges.entry(from).or_default().entry(direction).or_default().extend(to);
    }

    pub fn remove_window(&mut self, from: &Window, direction: Direction, to: &Window) {
        if let Some(dir_map) = self.edges.get_mut(from) {
            if let Some(vec) = dir_map.get_mut(&direction) {
                vec.retain(|win| win != to);
                if vec.is_empty() {
                    dir_map.remove(&direction);
                }
            }
            if dir_map.is_empty() {
                self.edges.remove(from);
            }
        }
    }

    pub fn remove_direction(&mut self, target: &Window, direction: &Direction) -> Option<Vec<Window>> {
        self.edges.get_mut(target)?.remove(direction)
    }

    pub fn tiled_add(&mut self, from: Window, direction: Direction, new: Window) {
        let opposite = direction.opposite();
        let orthogonal = direction.orthogonal();

        // new <--> orthogonal neighbors
        for d in orthogonal {
            if let Some(neighbors_orthogonal) = self.get(&from, &d).cloned() {
                
                for neighbor in &neighbors_orthogonal {
                    self.add_window(neighbor.clone(), d.opposite(), vec![new.clone()]);
                }
                
                self.add_window(new.clone(), d.clone(), neighbors_orthogonal);
            }
        }

        // new <--> neighbors
        if let Some(neighbors_direction) = self.remove_direction(&from, &direction) {
            
            for neighbor in &neighbors_direction {
                self.remove_window(neighbor, opposite.clone(), &from);
                self.add_window(neighbor.clone(), opposite.clone(), vec![new.clone()]);
            }
            
            self.add_window(new.clone(), direction.clone(), neighbors_direction);
        }

        // new <--> from
        self.add_window(from.clone(), direction, vec![new.clone()]);
        self.add_window(new, opposite, vec![from]);
    }

    pub fn exchange(&mut self, a: &Window, b: &Window) {
        let a_neighbors = self.edges.remove(a).unwrap_or_default();
        let b_neighbors = self.edges.remove(b).unwrap_or_default();

        self.edges.insert(a.clone(), b_neighbors);
        self.edges.insert(b.clone(), a_neighbors);

        // exchange a <-> b
        for (_, dir_map) in self.edges.iter_mut() {
            for (_, neighbors) in dir_map.iter_mut() {
                for win in neighbors.iter_mut() {
                    if win == a {
                        *win = b.clone();
                    } else if win == b {
                        *win = a.clone();
                    }
                }
            }
        }
    }

    #[cfg(feature="trace_layout")]
    pub fn print(&self) {
        for (from, hash_map) in &self.edges {
            info!("Window {:?} connections:", from.geometry().size);
            for (direction, to_list) in hash_map {
                for to in to_list {
                    info!("  ├── {:?} -> {:?}", direction, to.geometry().size);
                }
            }
        }
    }
    
}