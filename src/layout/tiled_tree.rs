use std::time::Duration;

use slotmap::{new_key_type, SlotMap};
use smithay::{desktop::{Space, Window}, reexports::calloop::LoopHandle, utils::{Logical, Point, Rectangle}};

use crate::{layout::{neighbor_graph::NeighborGraph, Direction}, manager::window::WindowExt, state::GlobalData};

use super::json_tiled_tree::JsonTree;

const GAP: i32 = 12;

#[derive(Debug, Clone)]
pub enum TiledScheme {
    Default,
    Spiral,
}

new_key_type! {
    pub struct NodeId;
}

#[derive(Debug, Clone)]
pub enum NodeData {
    Leaf { window: Window },
    Split {
        direction: Direction,
        rec: Rectangle<i32, Logical>,
        offset: Point<i32, Logical>,
        left: NodeId,
        right: NodeId,
    }
}

#[derive(Debug)]
pub struct TiledTree {
    nodes: SlotMap<NodeId, NodeData>,
    spiral_node: Option<NodeId>,
    root: Option<NodeId>,
    neighbor_graph: NeighborGraph,
}

impl TiledTree {
    pub fn new(window: Window) -> Self {
        let mut nodes = SlotMap::with_key();
        let root = Some(nodes.insert(NodeData::Leaf { window }));
        let spiral_node = root.clone();

        Self { 
            nodes,
            spiral_node,
            root,
            neighbor_graph: NeighborGraph::new()
       }
    }

    pub fn expansion(&self, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(bound) = self.get_root_rec(space) {
            let width = (bound.size.w - 2*GAP) / 3;
            let height = bound.size.h;
            let mut loc = bound.loc;

            for node in self.nodes.values() {
                match node {
                    NodeData::Leaf { window } => {
                        let from = space.element_geometry(window).unwrap();

                        window.set_rec((width, height).into());
                        space.map_element(window.clone(), loc, false);

                        let window = window.clone();

                        loop_handle.insert_idle(move |data| {
                            data.render_manager.add_animation(
                                window,
                                from,
                                Rectangle { loc, size: (width, height).into() },
                                Duration::from_millis(30),
                                crate::animation::AnimationType::EaseInOutQuad,
                            );
                        });

                        loc.x = loc.x + width + GAP;
                    }
                    _ => { }
                }
            }
        }
    }

    pub fn recover(&mut self, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(root_id) = self.get_root() {
            match self.nodes[root_id] {
                NodeData::Split { rec , .. } => {
                    self.modify(root_id, rec, space, loop_handle);
                }
                _ => { }
            }
        }
    }
    
    pub fn get_root(&self) -> Option<NodeId> {
        self.root
    }
    
    pub fn get_root_rec(&self, space: &mut Space<Window>) -> Option<Rectangle<i32, Logical>>{
        match self.get_root() {
            Some(root_id) => {
                match &self.nodes[root_id] {
                    NodeData::Leaf { window } => { space.element_geometry(window) }
                    NodeData::Split { rec, .. } => Some(rec.clone())
                }
            }
            None => {
                None
            }
        }
    }

    pub fn get_count(&self) -> usize {
        self.nodes.values().filter(|node| matches!(node, NodeData::Leaf { .. })).count()
    }
    
    fn find_parent_and_sibling(&self, target: NodeId) -> Option<(NodeId, NodeId)> {
        self.nodes.iter().find_map(|(id, data)| match data {
            NodeData::Split { left, right, .. } => {
                if *left == target {
                    Some((id, *right))
                } else if *right == target {
                    Some((id, *left))
                } else {
                    None
                }
            }
            _ => None,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn find_node(&self, window: &Window) -> Option<NodeId> {
        self.nodes.iter()
            .find_map(|(id, data)| match data {
                NodeData::Leaf { window: w } if w == window => Some(id),
                _ => None,
            })
    }

    pub fn get_first_window(&self) -> Option<&Window> {
        let root_id = match self.get_root() {
            Some(r) => {
                r
            }
            None => {
                warn!("Failed to get root_id");
                return None
            }
        };

        fn get_window(nodes: &SlotMap<NodeId, NodeData>, id: NodeId) -> Option<&Window> {
            match &nodes[id] {
                NodeData::Leaf { window } => Some(window),
                NodeData::Split { left, .. } => {
                    get_window(nodes, *left)
                }
            }
        }

        get_window(&self.nodes, root_id)
    }

    pub fn insert_window(
        &mut self, 
        target: Option<&Window>, 
        new_window: Window, 
        direction: Direction, 
        space: &mut Space<Window>,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) -> bool {

        let target = match target {
            Some(window) => window.clone(),
            None => {
                match self.get_first_window() {
                    Some(window) => window.clone(),
                    None => {
                        warn!("Failed to get first window");
                        return false
                    }
                }
            }
        };

        if let Some(target_id) = self.find_node(&target) {
            // resize
            // TODO: use server geometry
            let rec = match space.element_geometry(&target){
                Some(r) => r,
                None => {
                    warn!("Failed to get window rectangle");
                    return false
                }
            };
            
            let mut original_rec = rec.clone();
            let new_rec = get_new_rec(&direction, &mut original_rec);
            
            // TODO: merge
            target.set_rec(original_rec.size);
            new_window.set_rec(new_rec.size);
            space.map_element(target.clone(), original_rec.loc, false);
            space.map_element(new_window.clone(), new_rec.loc, true);

            // adjust tree
            let old_leaf = self.nodes.insert(NodeData::Leaf { window: target.clone() });
            let new_leaf = self.nodes.insert(NodeData::Leaf { window: new_window.clone() });

            self.spiral_node = Some(new_leaf);

            // use split node hold leafs
            match direction {
                Direction::Left | Direction::Up => {
                    self.nodes[target_id] = NodeData::Split {
                        direction: direction.clone(),
                        rec,
                        offset: (0, 0).into(),
                        left: new_leaf,
                        right: old_leaf,
                    };
                }
                _ => {
                    self.nodes[target_id] = NodeData::Split {
                        direction: direction.clone(),
                        rec,
                        offset: (0, 0).into(),
                        left: old_leaf,
                        right: new_leaf,
                    };
                }   
            }

            // modify neighbor_graph
            self.neighbor_graph.tiled_add(target.clone(), direction.clone(), new_window.clone());

            // TODO: use config
            // create animation
            loop_handle.insert_idle(move |data| {
                data.render_manager.add_animation(
                    target,
                    rec,
                    original_rec,
                    Duration::from_millis(30),
                    crate::animation::AnimationType::EaseInOutQuad,
                );

                let mut from = new_rec;
                match direction {
                    Direction::Right => {
                        from.loc.x += from.size.w;
                    }
                    Direction::Left => {
                        from.loc.x -= from.size.w;
                    }
                    Direction::Up => {
                        from.loc.y -= from.size.h;
                    }
                    Direction::Down => {
                        from.loc.y += from.size.h;
                    }
                }

                data.render_manager.add_animation(
                    new_window,
                    from,
                    new_rec,
                    Duration::from_millis(30),
                    crate::animation::AnimationType::EaseInOutQuad,
                );
            });

            true
        } else {
            false
        }
    }

    pub fn insert_window_spiral(&mut self, new_window: Window, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) {

        let spiarl_node  = match self.spiral_node {
            Some(node_id) => &self.nodes[node_id],
            None => {
                return;
            }
        };

        let target = match spiarl_node {
            NodeData::Leaf { window } => { window }
            _ => { return; }
        };

        let direction = Direction::ALL[(self.get_count() - 1) % 4].clone();

        self.insert_window(Some(&target.clone()), new_window, direction, space, loop_handle);
    }

    pub fn remove(&mut self, target: &Window, focus: &mut Option<Window>, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) -> bool {
        let target_id = match self.find_node(target) {
            Some(r) => {
                r
            }
            None => {
                warn!("Failed to get target_id");
                return false
            }
        };

        // remove last node
        if Some(target_id) == self.root {
            if matches!(self.nodes[target_id], NodeData::Leaf { .. }) {
                self.nodes.remove(target_id);
                self.root = None;
                *focus = None;
                return true;
            }
        }

        let (parent_id, sibling_id) = match self.find_parent_and_sibling(target_id) {
            Some(r) => r,
            None => {
                warn!("Failed to get node: {:?} parent and sibling", target_id);
                return false
            }
        };

        if self.spiral_node == Some(target_id) {
            self.spiral_node = Some(parent_id);
        }

        match self.nodes[parent_id] {
            NodeData::Split { rec, .. } => {
                let sibling_data = match self.nodes.remove(sibling_id){
                    Some(r) => r,
                    None => {
                        warn!("Failed to remove sibling: {:?}", sibling_id);
                        return false
                    }
                };

                match sibling_data {
                    NodeData::Leaf { window } => {
                        let from = space.element_geometry(&window).unwrap();

                        window.set_rec(rec.size);
                        space.map_element(window.clone(), rec.loc, false);

                        self.nodes[parent_id] = NodeData::Leaf { window: window.clone() };

                        if focus.as_ref() == Some(target) {
                            *focus = Some(window.clone());
                        }

                        loop_handle.insert_idle(move |data| {
                            data.render_manager.add_animation(
                                window,
                                from,
                                rec,
                                Duration::from_millis(30),
                                crate::animation::AnimationType::EaseInOutQuad,
                            );
                        });
                    },
                    NodeData::Split { direction, left, right, .. } => {
                        self.nodes[parent_id] = NodeData::Split { 
                            direction, 
                            rec, // from parent
                            offset: (0, 0).into(),
                            left, 
                            right,
                        };
                        self.modify(parent_id, rec, space, loop_handle);

                        if focus.as_ref() == Some(target) {
                            *focus = self.get_first_window().cloned();
                        }

                    }
                }

                self.nodes.remove(target_id);

                true
            },
            NodeData::Leaf { .. } => { 
                false 
            }
        }
    }

    pub fn modify(&mut self, node_id: NodeId, rec: Rectangle<i32, Logical>, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) {
        // modify the child tree with new rec with direction
        match &mut self.nodes[node_id] {
            NodeData::Leaf { window } => {
                let from = space.element_geometry(&window).unwrap();

                window.set_rec(rec.size);
                space.map_element(window.clone(), rec.loc, false);

                let window = window.clone();
                loop_handle.insert_idle(move |data| {
                    data.render_manager.add_animation(
                        window,
                        from,
                        rec,
                        Duration::from_millis(30),
                        crate::animation::AnimationType::EaseInOutQuad,
                    );
                });
            },
            NodeData::Split { left, right, direction, rec: current_rec, offset } => {
                let (l_rec, r_rec) = recover_new_rec(rec, direction, offset.clone());

                *current_rec = rec.clone();

                let left_id = *left;
                let right_id = *right;

                self.modify(left_id, l_rec, space, loop_handle);
                self.modify(right_id, r_rec, space, loop_handle);
            }
        }
    }

    pub fn invert_window(&mut self, target: &Window, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>){
        let target_id = match self.find_node(target) {
            Some(r) => {
                r
            }
            None => {
                warn!("Failed to get target_id");
                return
            }
        };

        // Only single window
        if self.get_root() == Some(target_id) {
            return;
        }

        let (parent_id, _) = match self.find_parent_and_sibling(target_id) {
            Some(r) => r,
            None => {
                warn!("Failed to get node: {:?} parent and sibling", target_id);
                return
            }
        };

        match &mut self.nodes[parent_id] {
            NodeData::Split { direction, rec , .. } => {
                *direction = direction.rotate_cw();
                let rec = *rec;
                self.modify(parent_id, rec, space, loop_handle);
            },
            NodeData::Leaf { .. } => { }
        }
    }

    pub fn _resize(&mut self, target: &Window, offset: Point<i32, Logical>, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) {
        let target_id = match self.find_node(target) {
            Some(r) => {
                r
            }
            None => {
                warn!("Failed to get target_id");
                return
            }
        };

        // Only single window
        if self.get_root() == Some(target_id) {
            return;
        }

        let (parent_id, _) = match self.find_parent_and_sibling(target_id) {
            Some(r) => r,
            None => {
                warn!("Failed to get node: {:?} parent and sibling", target_id);
                return
            }
        };

        match &mut self.nodes[parent_id] {
            NodeData::Split { offset: current_offset, rec, .. } => {
                *current_offset += offset;
                let rec = *rec;
                self.modify(parent_id, rec, space, loop_handle);
            },
            NodeData::Leaf { .. } => { }
        }
    }

    pub fn from_json(&mut self, path: &str) {
        if let Some(json_tree) = JsonTree::from_json(path) {
            json_tree.print_tree();
        }
    }

    #[cfg(feature="trace_layout")]
    pub fn print_tree(&self) {
        let root_id = match self.get_root() {
            Some(r) => {
                r
            }
            None => {
                warn!("Failed to get root_id");
                return
            }
        };

        fn print(nodes: &SlotMap<NodeId, NodeData>, id: NodeId, depth: usize) {
            let indent = "  ".repeat(depth);
            match &nodes[id] {
                NodeData::Leaf { window } => tracing::info!("{indent}- Leaf: {:?}", window),
                NodeData::Split { left, right, .. } => {
                    tracing::info!("{indent}- Split:");
                    print(nodes, *left, depth + 1);
                    print(nodes, *right, depth + 1);
                }
            }
        }

        print(&self.nodes, root_id, 0);
        self.neighbor_graph.print();
    }
}

fn recover_new_rec(rec: Rectangle<i32, Logical>, direction: &Direction, offset: Point<i32, Logical>) -> (Rectangle<i32, Logical>, Rectangle<i32, Logical>) {
    let mut l_rec = rec;
    let mut r_rec = rec;

    let gap = (GAP as f32 * 0.5) as i32;

    match direction {
        Direction::Left | Direction::Right => {
            let half = rec.size.w / 2 - gap;
            l_rec.size.w = half + offset.x;
            r_rec.size.w = half - offset.x;

            r_rec.loc.x += half + GAP + offset.x;
        }
        Direction::Up | Direction::Down => {
            let half = rec.size.h / 2 - gap;
            l_rec.size.h = half + offset.y;
            r_rec.size.h = half - offset.y;

            r_rec.loc.y += half + GAP + offset.y;
        }
    }

    (l_rec, r_rec)
}

fn get_new_rec(direction: &Direction, rec: &mut Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {

    let mut new_rec = *rec;

    let gap = (GAP as f32 * 0.5) as i32;

    match direction {
        Direction::Left | Direction::Right => {
            let half = rec.size.w / 2 - gap;
            new_rec.size.w = half;
            rec.size.w = half;

            if direction == &Direction::Left {
                rec.loc.x += half + GAP;
            } else {
                new_rec.loc.x += half + GAP;
            }

            new_rec
        }
        Direction::Up | Direction::Down => {
            let half = rec.size.h / 2 - gap;
            new_rec.size.h = half;
            rec.size.h = half;

            if direction == &Direction::Up {
                rec.loc.y += half + GAP;
            } else {
                new_rec.loc.y += half + GAP;
            }

            new_rec
        }
    }
}