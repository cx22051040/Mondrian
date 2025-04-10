use smithay::{desktop::{Space, Window}, utils::{Point, Size, Logical, Rectangle}};

use crate::{config::WorkspaceConfigs, space::window::{WindowExt, WindowID}};

const RATE: f32 = 2.0;
const GAP: i32 = 12;

#[derive(Debug, Clone)]
pub enum LayoutScheme {
    Default,
    BinaryTree,
}

#[derive(Debug, Clone)]
pub enum TiledLayoutTree {
    Leaf {
        window: Window,
        rec: Rectangle<i32, Logical>
    },
    Split { 
        left: Box<TiledLayoutTree>, 
        right: Box<TiledLayoutTree>,
    },
}

impl TiledLayoutTree {
    pub fn new(window: Window, rec: Rectangle<i32, Logical>) -> (Self, Point<i32, Logical>) {
        let new_size = Size::from((rec.size.w - GAP*2, rec.size.h - GAP*2));
        let new_location = Point::from((rec.loc.x+GAP, rec.loc.y+GAP));

        window.toplevel().unwrap().with_pending_state(|state| {
            state.size = Some(new_size);
        });

        (        
            TiledLayoutTree::Leaf {
                window,
                rec: Rectangle { loc: new_location, size: new_size },
            },
            new_location,
        )
    }

    pub fn insert_window(&mut self, new_window: Window) -> Point<i32, Logical> {
        match self {
            TiledLayoutTree::Leaf { window, rec } => {

                let mut new_rec = *rec;

                if rec.size.h as f32 / rec.size.w as f32 > RATE {
                    rec.size.h /= 2;
                    rec.size.h -= (0.5*GAP as f32) as i32;
                    new_rec.size.h /= 2;
                    new_rec.size.h -= (0.5*GAP as f32) as i32;

                    new_rec.loc.y += rec.size.h + GAP;
                } else {
                    rec.size.w /= 2;
                    rec.size.w -= (0.5*GAP as f32) as i32;
                    new_rec.size.w /= 2;
                    new_rec.size.w -= (0.5*GAP as f32) as i32;

                    new_rec.loc.x += rec.size.w + GAP;
                }

                // set the size of window
                window.toplevel().unwrap().with_pending_state(|state| {
                    state.size = Some(rec.size);
                });
                new_window.toplevel().unwrap().with_pending_state(|state| {
                    state.size = Some(new_rec.size);
                });

                let output = new_rec.loc.clone();

                *self = TiledLayoutTree::Split {
                    left: Box::new(TiledLayoutTree::Leaf {
                        window: window.clone(),
                        rec: *rec,
                    }),
                    right: Box::new(TiledLayoutTree::Leaf {
                        window: new_window,
                        rec: new_rec,
                    }),
                };

                output
            },
            TiledLayoutTree::Split { left, right } => {
                if left.depth() < right.depth() {
                    left.insert_window(new_window)
                } else {
                    right.insert_window(new_window)
                }
            }
        }
    }
    
    pub fn depth(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { left, right } => 1 + left.depth().min(right.depth()),
        }
    }
}