use std::{cell::RefCell, time::Duration};

use smithay::{desktop::Window, utils::{Logical, Point, Rectangle}};

use crate::{
    layout::{
        tiled_tree::TiledTree, 
        Direction, ResizeEdge, TiledScheme,
        WindowLayout
    }, 
    manager::{animation::{AnimationManager, AnimationType}, window::WindowExt},
};

#[derive(Default)]
pub struct ExpansionCache(pub RefCell<Option<Rectangle<i32, Logical>>>);

impl ExpansionCache {
    pub fn get(&self) -> Option<Rectangle<i32, Logical>> {
        self.0.borrow().clone()
    }
}

#[derive(Debug)]
pub struct ContainerTree {
    tiled_tree: Option<TiledTree>,
    floating: Vec<Window>,

    root_rect: Rectangle<i32, Logical>,
    gap: i32,
}

impl ContainerTree {
    pub fn new(root_rect: Rectangle<i32, Logical>, gap: i32) -> ContainerTree {
        Self { 
            tiled_tree: None,
            floating: Vec::new(),
            root_rect,
            gap,
        }
    }

    pub fn update_root_rect(&mut self, root_rect: Rectangle<i32, Logical>, animation_manager: &mut AnimationManager) {
        self.root_rect = root_rect;
        if let Some(tiled_tree) = &mut self.tiled_tree {
            tiled_tree.update_root_rect_recursive(root_rect, animation_manager);
        }
    }

    pub fn insert(
        &mut self,
        target: Option<&Window>,
        window: Window,
        edge: ResizeEdge,
        scheme: &TiledScheme,
        animation_manager: &mut AnimationManager,
    ) -> bool {
        let result = match window.get_layout() {
            WindowLayout::Tiled => {
                if let Some(tiled_tree) = &mut self.tiled_tree {
                    match scheme {
                        TiledScheme::Default => {
                            // default: must have target if layout tree is some         
                            let target = target.unwrap(); 
                            let target_rec = target.get_rect().unwrap();
    
                            let (direction, is_favour) = edge.to_direction_and_favour(target_rec);
    
                            tiled_tree.insert(
                                target,
                                direction,
                                window.clone(),
                                is_favour,
                                animation_manager,
                            )
                        }
                        TiledScheme::Scroll => {
                            // TODO
                            false
                        }
                    }
                } else {
                    self.tiled_tree = Some(TiledTree::new_with_first_node(window.clone(), self.root_rect, self.gap, animation_manager));
                    true
                }
            },
            WindowLayout::Floating => {
                self.floating.push(window.clone());
                true
            }
        };

        if result {
            #[cfg(feature = "trace_layout")]
            self.print_tree()
        }

        result
    }

    pub fn remove(&mut self, target: &Window, animation_manager: &mut AnimationManager) {
        match target.get_layout() {
            WindowLayout::Tiled => {
                if let Some(tiled_tree) = &mut self.tiled_tree {
                    tiled_tree.remove(target, animation_manager);
                } else {
                    error!("the tiled_tree is none");
                }

                // remove tiled_tree if empty
                if self.tiled_tree_is_empty() {
                    self.tiled_tree = None;
                }
            },
            WindowLayout::Floating => {
                self.floating.retain(|window| window != target);
            }
        }

        #[cfg(feature = "trace_layout")]
        self.print_tree();
    }

    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        let tiled_iter = self.tiled_tree
            .as_ref()
            .map(|tree| tree.windows())
            .into_iter()
            .flatten();

        self.floating.iter().chain(tiled_iter)
    }

    pub fn grab_move(&mut self, target: &Window, offset: Point<i32, Logical>, animation_manager: &mut AnimationManager) {
        match target.get_layout() {
            WindowLayout::Tiled => { },
            WindowLayout::Floating => {
                // void conflict
                animation_manager.stop_animation(target);

                let mut rect = target.get_rect().unwrap();
                rect.loc += offset;

                target.set_rect_cache(rect);
                target.send_rect(rect);
            }
        }
    }

    pub fn resize(
        &mut self, 
        target: &Window, 
        direction: Direction, 
        offset: i32, 
        is_favour: bool,
    ) {
        match target.get_layout() {
            WindowLayout::Tiled => {
                if let Some(tiled_tree) = &mut self.tiled_tree {
                    tiled_tree.resize(target, direction, offset, is_favour);
                } else {
                    error!("the tiled_tree is none");
                }
            },
            WindowLayout::Floating => {
                let mut rect = target.get_rect().unwrap();
                match direction {
                    Direction::Horizontal => {
                        if is_favour {
                            rect.loc.x += offset;
                            rect.size.w -= offset;
                        } else {
                            rect.size.w += offset;
                        }
                    }
                    Direction::Vertical => {
                        if is_favour {
                            rect.loc.y += offset;
                            rect.size.h -= offset;
                        } else {
                            rect.size.h += offset;
                        }
                    }
                }
                target.set_rect_cache(rect);
                target.send_rect(rect);
            }
        }

    }

    pub fn invert(&mut self, target: &Window, animation_manager: &mut AnimationManager) {
        match target.get_layout() {
            WindowLayout::Tiled => {
                if let Some(tiled_tree) = &mut self.tiled_tree {
                    tiled_tree.invert(target, animation_manager);
                } else {
                    error!("the tiled_tree is none");
                }
            },
            WindowLayout::Floating => { }
        }

        #[cfg(feature = "trace_layout")]
        self.print_tree();
    }

    pub fn exchange(&mut self, target: &Window, direction: Direction, is_favour: bool, animation_manager: &mut AnimationManager) {
        match target.get_layout() {
            WindowLayout::Tiled => {
                if let Some(tiled_tree) = &mut self.tiled_tree {
                    tiled_tree.exchange(target, direction, is_favour, animation_manager);
                } else {
                    error!("the tiled_tree is none");
                }
            },
            WindowLayout::Floating => { }
        }
        
        #[cfg(feature = "trace_layout")]
        self.print_tree();
    }    

    pub fn expansion(&self, animation_manager: &mut AnimationManager) {
        let total = self.windows().count();
        let max_per_row = 4;
        let gap = self.gap;
        let screen = self.root_rect;

        let row_counts = split_rows(total, max_per_row);
        
        #[cfg(feature = "trace_layout")]
        info!("expansion row counts: {:?}", row_counts);

        let row_count = row_counts.len();
        let win_height = (screen.size.h - gap * (row_count - 1) as i32) / row_count as i32;
        
        let total_gap = gap * (max_per_row + 1 - 1) as i32;
        let win_width = (screen.size.w - total_gap) / (max_per_row + 1) as i32;

        let mut window_iter = self.windows();

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
                    if window
                        .user_data()
                        .get::<ExpansionCache>()
                        .map(|guard| guard.0.borrow().is_none())
                        .unwrap_or(true) 
                    {
                        let window_rect = window.get_rect().unwrap();

                        // ExpansionCache
                        let guard = window.user_data().get_or_insert::<ExpansionCache, _>(|| {
                            ExpansionCache(RefCell::new(Some(rect)))
                        });
                        *guard.0.borrow_mut() = Some(rect);

                        animation_manager.add_animation(
                            window,
                            window_rect,
                            rect,
                            Duration::from_millis(30),
                            AnimationType::EaseInOutQuad,
                        );
                    }
                }
            }

            y += win_height + gap;
        }
    }

    pub fn recover(&self, animation_manager: &mut AnimationManager) {
        for window in self.windows() {
            // set expansion cache
            if let Some(guard) = window.user_data().get::<ExpansionCache>() {
                if let Some(from) = guard.get() {
                    let to = window.get_rect().unwrap();
    
                    guard.0.borrow_mut().take();
    
                    animation_manager.add_animation(
                        window.clone(), 
                        from, 
                        to, 
                        Duration::from_millis(30),
                        AnimationType::EaseInOutQuad,
                    );
                }
            }

        }
    }

    pub fn is_empty(&self) -> bool {
        self.tiled_tree_is_empty() && self.floating.is_empty()
    }

    pub fn tiled_tree_is_empty(&self) -> bool {
        self.tiled_tree
            .as_ref()
            .map(|tree| tree.is_empty())
            .unwrap_or(true)
    }

    #[cfg(feature = "trace_layout")]
    pub fn print_tree(&self) {
        if let Some(tiled_tree) = &self.tiled_tree {
            tiled_tree.print_tree();
        }

        let _ = self.floating.iter().map(|window| info!("Float window: Rect: {:?}", window.get_rect()));
    }
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