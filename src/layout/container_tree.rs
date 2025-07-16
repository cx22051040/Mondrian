use std::time::Duration;

use indexmap::IndexMap;
use slotmap::{new_key_type, SlotMap};
use smithay::{desktop::Window, reexports::calloop::LoopHandle, utils::{Logical, Rectangle}};

use crate::{layout::Direction, manager::window::WindowExt, state::GlobalData};

new_key_type! {
    pub struct NodeId;
}

#[derive(Debug, Clone)]
pub enum NodeData {
    Node {
        window: Window,

        sibling: NodeId,
        parent: NodeId,
    },
    Container {
        elements: Vec<NodeId>,
        rect: Rectangle<i32, Logical>,
        offset: i32,

        sibling: NodeId,
        parent: NodeId,

        direction: Direction,
    },
}

#[derive(Debug)]
pub struct ContainerTree {
    nodes: SlotMap<NodeId, NodeData>,
    root: NodeId,

    windows: IndexMap<Window, NodeId>,
    gap: i32,
}

impl ContainerTree {
    pub fn new_with_first_node(target: Window, root_rect: Rectangle<i32, Logical>, gap: i32) -> ContainerTree {
        
        target.set_rect_cache(root_rect);

        let mut nodes = SlotMap::with_key();
        let mut windows = IndexMap::new();

        let first_node = NodeData::Node { 
            window: target.clone(), 
            sibling: NodeId::default(), 
            parent: NodeId::default()
        };

        let first_id = nodes.insert(first_node);

        // set sibling and parent to itself
        if let Some(NodeData::Node { sibling, parent, .. }) = nodes.get_mut(first_id) {
            *sibling = first_id;
            *parent = first_id;
        }
        
        windows.insert(target, first_id);

        Self { 
            nodes, 
            root: first_id, 
            windows,
            
            gap,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn insert(
        &mut self, 
        target: &Window, 
        direction: Direction, 
        window: Window, 
        is_favour: bool, 
        loop_handle: &LoopHandle<'_, GlobalData>
    ) {
        /*
            split new_rect from target nodes,
            convert target (nodes) into parent (container),
            insert new_target and old_target
        */

        if let Some(target_id) = self.find_node_id(target).cloned() {
            if let Some(NodeData::Node { window: old_window, sibling: old_sibling, parent: old_parent }) = self.nodes.get(target_id) {
                let old_window = old_window.clone();
                let old_sibling = old_sibling.clone();
                let old_parent = old_parent.clone();

                let old_rect = old_window.get_rect();

                // get new rect
                let (target_rect, new_rect) = split_rect(old_rect, direction, 0, self.gap, is_favour);
                old_window.set_rect_cache(target_rect);
                window.set_rect_cache(new_rect);

                // insert target_copy and new nodes
                let target_copy_id = self.nodes.insert(
                    NodeData::Node { 
                        window: old_window.clone(), 
                        sibling: NodeId::default(), 
                        parent: target_id
                    }  
                );

                let new_id = self.nodes.insert(
                    NodeData::Node { 
                        window: window.clone(), 
                        sibling: target_copy_id, 
                        parent: target_id 
                    }
                );

                self.windows.insert(old_window.clone(), target_copy_id);
                self.windows.insert(window.clone(), new_id);

                if let Some(NodeData::Node { sibling, .. }) = self.nodes.get_mut(target_copy_id) {
                    *sibling = new_id;
                }

                // convert target from node to container inplace
                let mut elements = vec![];

                if is_favour {
                    elements.push(new_id);
                    elements.push(target_copy_id);
                }else {
                    elements.push(target_copy_id);
                    elements.push(new_id);
                }

                if let Some(target_data) = self.nodes.get_mut(target_id) {
                    *target_data = NodeData::Container { 
                        elements, 
                        rect: old_rect,
                        offset: 0,
                        sibling: old_sibling, 
                        parent: old_parent, 
                        direction 
                    };
                }

                // add animation
                {
                    // target node
                    loop_handle.insert_idle(move |data| {
                        data.render_manager.add_animation(
                            old_window,
                            old_rect,
                            target_rect,
                            Duration::from_millis(15),
                            crate::animation::AnimationType::EaseInOutQuad,
                        );
                    });

                    // new node
                    loop_handle.insert_idle(move |data| {
                        let mut from = new_rect;
                        if matches!(direction, Direction::Horizontal) {
                            if is_favour {
                                from.loc.x -= from.size.w;
                            } else {
                                from.loc.x += from.size.w;
                            }
                        } else if matches!(direction, Direction::Vertical){
                            if is_favour {
                                from.loc.y -= from.size.h;
                            } else {
                                from.loc.y += from.size.h;
                            }
                        }

                        data.render_manager.add_animation(
                            window,
                            from,
                            new_rect,
                            Duration::from_millis(45),
                            crate::animation::AnimationType::OvershootBounce,
                        );
                    });
                }
            }
        } else {
            error!("not find target_id from window: {:?}", target);
        }
    }

    pub fn remove(&mut self, target: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        /*
            convert parent (Container) into sibling (Node),
            inherit parent's parent and sibling,
            use parent's rect,
            delete target and old_sibling node
        */

        if let Some(target_id) = self.find_node_id(target).cloned() {
            if let Some(NodeData::Node { sibling: target_sibling, parent: target_parent, .. }) = self.nodes.get(target_id) {
                // only root node
                if target_id == self.root {
                    self.nodes.remove(target_id);
                    self.windows.shift_remove(target);

                    return;
                }

                let target_parent = target_parent.clone();
                let target_sibling = target_sibling.clone();

                // convert parent into sibling node
                // use container's sibling & parent
                if let Some(sibling_data) = self.nodes.get(target_sibling) {
                    match sibling_data {
                        NodeData::Node { window: sibling_window, .. } => {
                            let sibling_window = sibling_window.clone();

                            if let Some(NodeData::Container { rect: parent_rect, sibling: parent_sibling, parent: parent_parent, .. }) = self.nodes.get(target_parent) {
                                let sibling_window = sibling_window.clone();
                                let parent_rect = parent_rect.clone();

                                // merge target rect and sibling rect
                                let sibling_rect = sibling_window.get_rect();
                                sibling_window.set_rect_cache(parent_rect.clone());

                                self.windows.insert(sibling_window.clone(), target_parent);
                                self.nodes[target_parent] = NodeData::Node {
                                    window: sibling_window.clone(),
                                    sibling: parent_sibling.clone(),
                                    parent: parent_parent.clone(),
                                };

                                // add animation
                                loop_handle.insert_idle(move |data| {
                                    data.render_manager.add_animation(
                                        sibling_window.clone(),
                                        sibling_rect,
                                        parent_rect,
                                        Duration::from_millis(30),
                                        crate::animation::AnimationType::EaseInOutQuad,
                                    );
                                });
                            }

                            // remove old_sibling nodes
                            self.windows.insert(sibling_window.clone(), target_parent);
                        }
                        NodeData::Container { offset: sibling_offset, elements: sibling_elements, direction: sibling_direction, .. } => {
                            if let Some(NodeData::Container { rect: parent_rect, sibling: parent_sibling, parent: parent_parent, .. }) = self.nodes.get(target_parent) {
                                let sibling_elements = sibling_elements.clone();
                                let parent_rect = parent_rect.clone();
                                
                                self.nodes[target_parent] = NodeData::Container { 
                                    elements: sibling_elements.clone(), 
                                    rect: parent_rect.clone(), 
                                    offset: sibling_offset.clone(),

                                    sibling: parent_sibling.clone(), 
                                    parent: parent_parent.clone(), 
                                    direction: sibling_direction.clone()
                                };

                                self.update_rect_recursive(target_parent, parent_rect, loop_handle);

                                sibling_elements.iter().for_each(|node_id| {
                                    if let Some(node_data) = self.nodes.get_mut(*node_id) {
                                        match node_data {
                                            NodeData::Node { parent, .. } => *parent = target_parent,
                                            NodeData::Container { parent, .. } => *parent = target_parent,
                                        }
                                    }
                                });
                            }
                        }
                    }
                }

                // remove from nodes and windows
                self.nodes.remove(target_id);
                self.nodes.remove(target_sibling);
                self.windows.shift_remove(target);
            }
        } else {
            error!("not find target_id from window: {:?}", target);
        }
    }

    pub fn invert(&mut self, target: &Window, loop_handle: &LoopHandle<'_, GlobalData>) {
        /*
            invert parent (Container) direction
            update recursive 
        */

        if let Some(target_id) = self.find_node_id(target).cloned() {
            if let Some(NodeData::Node { parent: target_parent, .. }) = self.nodes.get(target_id) {
                let target_parent = target_parent.clone();

                if let Some(NodeData::Container { rect, direction, .. }) = self.nodes.get_mut(target_parent) {
                    *direction = direction.invert();
                    let rect = rect.clone();

                    self.update_rect_recursive(target_parent, rect, loop_handle);
                }
            }
        }
    }

    pub fn exchange(&mut self, _target: &Window, _is_favour: bool, _loop_handle: &LoopHandle<'_, GlobalData>) {
        /*
            find exchange node with vec add or sub,
            if none, get parent and continue until find root,
            exchange node
        */
    }

    pub fn expansion(&self, root_rect: Rectangle<i32, Logical>, loop_handle: &LoopHandle<'_, GlobalData>) {
        let total = self.windows.len();
        let max_per_row = 4;
        let gap = self.gap;
        let screen = root_rect;

        let row_counts = split_rows(total, max_per_row);
        
        #[cfg(feature = "trace_layout")]
        info!("expansion row counts: {:?}", row_counts);

        let row_count = row_counts.len();
        let win_height = (screen.size.h - gap * (row_count - 1) as i32) / row_count as i32;
        
        let total_gap = gap * (max_per_row + 1 - 1) as i32;
        let win_width = (screen.size.w - total_gap) / (max_per_row + 1) as i32;

        let mut window_iter = self.windows.keys();

        let mut y = screen.loc.y;
        for &cols in &row_counts {
            let total_width = win_width * cols as i32 + total_gap;
            let start_x = screen.loc.x + (screen.size.w - total_width) / 2;

            for i in 0..cols {
                let x = start_x + i as i32 * (win_width + gap);
                let rect = Rectangle {loc: (x, y).into(), size: (win_width, win_height).into()};
                
                #[cfg(feature = "trace_layout")]
                info!("expansion rect: {:?}", rect);

                if let Some(window) = window_iter.next().cloned() {
                    // add animation
                    let window_rect = window.get_rect();
                    window.set_rect_cache(rect);

                    loop_handle.insert_idle(move |data| {
                        data.render_manager.add_animation(
                            window,
                            window_rect,
                            rect,
                            Duration::from_millis(30),
                            crate::animation::AnimationType::EaseInOutQuad,
                        );
                    });

                }
            }

            y += win_height + gap;
        }
    }

    pub fn resize(&mut self, target: &Window, direction: Direction, offset: i32, is_favour: bool, loop_handle: &LoopHandle<'_, GlobalData>) {
        /*
            find the target nodes and resize target nodes,
            get the max container,
            resize the max container's elements
        */

        if let Some(target_id) = self.find_node_id(target).cloned() {
            if let Some(max_parent_id) = self.find_node_with_direction_and_favour(target_id, direction, is_favour) {
                if let Some(NodeData::Container { rect, offset: parent_offset, .. }) = self.nodes.get_mut(max_parent_id) {
                    let rect = rect.clone();
                    // TODO: use client's given
                    let min = 175;

                    let half = match direction {
                        Direction::Horizontal => {
                            (rect.size.w - self.gap) / 2 - min
                        }
                        Direction::Vertical => {
                            (rect.size.h - self.gap) / 2 - min
                        }
                    };
                    *parent_offset = (*parent_offset + offset).clamp(-half, half);

                    self.update_rect_recursive(max_parent_id, rect, loop_handle);
                }
            }
        }
    }

    fn find_node_with_direction_and_favour(&self, node_id: NodeId, direction: Direction, is_favour: bool) -> Option<NodeId> {
        /*
            find node with direction and favour,
            if not, jump to parent and continue,
            if parent's direction is not eqult to given diretion,
            jump to parent's parent and continue,
            return current node id and resize target node id
        */

        if self.root == node_id {
            return None;
        }

        if let Some(node_data) = self.nodes.get(node_id) {
            let parent = match node_data {
                NodeData::Node { parent, .. } => {
                    parent.clone()
                },
                NodeData::Container { parent, .. } => {
                    parent.clone()
                }  
            };

            if let Some(NodeData::Container { elements, direction: parent_direction, .. }) = self.nodes.get(parent) {
                if direction == *parent_direction {
                    if let Some(idx) = elements.iter().position(|id| *id == node_id) {
                        let neighbor = if is_favour {
                            idx.checked_sub(1).and_then(|i| elements.get(i))
                        } else {
                            elements.get(idx + 1)
                        };

                        if neighbor.is_some() {
                            return Some(parent);
                        }
                    }
                }

                return self.find_node_with_direction_and_favour(parent, direction, is_favour);
            }
        }

        return None;
    }

    pub fn update_root_rect_recursive(&mut self, root_rect: Rectangle<i32, Logical>, loop_handle: &LoopHandle<'_, GlobalData>) {
        self.update_rect_recursive(self.root, root_rect, loop_handle);
    }

    fn update_rect_recursive(&mut self, node_id: NodeId, new_rect: Rectangle<i32, Logical>, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(node_data) = self.nodes.get_mut(node_id) {
            match node_data {
                NodeData::Node { window, .. } => {
                    let old_rect = window.get_rect();
                    window.set_rect_cache(new_rect);

                    // add animation
                    let window = window.clone();
                    loop_handle.insert_idle(move |data| {
                        data.render_manager.add_animation(
                            window,
                            old_rect,
                            new_rect,
                            Duration::from_millis(30),
                            crate::animation::AnimationType::EaseInOutQuad,
                        );
                    });
                }

                NodeData::Container { elements, rect, offset, direction, .. } => {
                    *rect = new_rect;

                    let (rect_1, rect_2) = split_rect(new_rect, direction.clone(), offset.clone(), self.gap, false);
                    
                    let children = elements.clone();
                    for (child_id, sub_rect) in children.into_iter().zip([rect_1, rect_2]) {
                        self.update_rect_recursive(child_id, sub_rect, loop_handle);
                    }
                }
            }
        }
    }

    fn find_node_id(&self, target: &Window) -> Option<&NodeId> {
        self.windows.get(target)
    }

    #[cfg(feature = "trace_layout")]
    pub fn print_tree(&self) {
        fn print(nodes: &SlotMap<NodeId, NodeData>, windows: &IndexMap<Window, NodeId>, id: NodeId, depth: usize) {
            let indent = "  ".repeat(depth);

            match &nodes[id] {
                NodeData::Node { window, sibling, parent, .. } => {
                    let window_rect = window.get_rect();
                    info!("{indent}- Leaf: {:?} - Rect: {:?} - Sib: {:?} - Parent: {:?}", id, window_rect, sibling, parent);
                }

                NodeData::Container { elements, direction, .. } => {
                    info!("{indent}- Split: {:?} - Direction: {:?}", id, direction);

                    for child_id in elements {
                        print(nodes, windows, *child_id, depth + 1);
                    }
                }
            }
        }

        print(&self.nodes, &self.windows, self.root, 0);
    }
}

fn split_rect(
    target_rect: Rectangle<i32, Logical>, 
    direction: Direction, 
    offset: i32, 
    gap: i32, 
    is_favour: bool
) -> (Rectangle<i32, Logical>, Rectangle<i32, Logical>) {
    let mut target_rect = target_rect.clone();
    let mut new_rect = target_rect.clone();

    match direction {
        Direction::Horizontal => {
            let half = (target_rect.size.w - gap) / 2;

            new_rect.size.w = half;
            target_rect.size.w = half;

            if is_favour {
                target_rect.loc.x += half + gap + offset;

                target_rect.size.w -= offset;
                new_rect.size.w += offset;
            } else {
                new_rect.loc.x += half + gap + offset;

                new_rect.size.w -= offset;
                target_rect.size.w += offset;
            }
        }
        Direction::Vertical => {
            let half = (target_rect.size.h - gap) / 2;

            new_rect.size.h = half;
            target_rect.size.h = half;

            if is_favour {
                target_rect.loc.y += half + gap + offset;

                target_rect.size.h -= offset;
                new_rect.size.h += offset;
            } else {
                new_rect.loc.y += half + gap + offset;

                new_rect.size.h -= offset;
                target_rect.size.h += offset;
            }
        }
    }

    (target_rect, new_rect)
}

fn split_rows(total: usize, max_per_row: usize) -> Vec<usize> {
    let rows = (total + max_per_row - 1) / max_per_row;
    let base = total / rows;
    let mut remainder = total % rows;
    let mut result = Vec::new();

    for _ in 0..rows {
        if remainder > 0 {
            result.push(base + 1);
            remainder -= 1;
        } else {
            result.push(base);
        }
    }

    result
}