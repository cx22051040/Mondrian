// pub mod json_tiled_tree;
// pub mod neighbor_graph;
// pub mod tiled_tree;

use smithay::utils::{Logical, Rectangle};

pub mod container_tree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    pub fn invert(&self) -> Direction {
        match self {
            Direction::Horizontal => Direction::Vertical,
            Direction::Vertical => Direction::Horizontal
        }
    }
}

#[derive(Debug, Clone)]
pub enum TiledScheme {
    Default,
    #[allow(dead_code)]
    Spiral,
}

#[derive(Debug, Clone)]
pub enum ResizeEdge {
    None,

    Top,
    Bottom,
    Left,
    Right,

    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    pub fn to_direction_and_favour(&self, rect: Rectangle<i32, Logical>) -> (Direction, bool) {
        use ResizeEdge::*;

        match self {
            // use Rectangle::default()
            Left => (Direction::Horizontal, true),
            Top => (Direction::Vertical, true),
            Right => (Direction::Horizontal, false),
            Bottom => (Direction::Vertical, false),

            TopLeft | TopRight | BottomLeft => {
                if rect.size.w > rect.size.h {
                    let is_favour = matches!(self, TopLeft | BottomLeft);
                    (Direction::Horizontal, is_favour)
                } else {
                    let is_favour = matches!(self, TopLeft | TopRight);
                    (Direction::Vertical, is_favour)
                }
            }

            BottomRight => {
                if rect.size.w > rect.size.h {
                    (Direction::Horizontal, false)
                } else {
                    (Direction::Vertical, false)
                }
            }

            // fallback
            _ => (Direction::Horizontal, false),
        }
    }

    pub fn _is_favour(&self) -> bool {
        use ResizeEdge::*;

        match self {
            Top | Left | TopLeft | TopRight | BottomLeft => {
                true
            }

            _ => false,
        }
    }

    pub fn split(&self) -> impl Iterator<Item = ResizeEdge> {
        use ResizeEdge::*;

        match self {
            TopLeft => vec![Top, Left].into_iter(),
            TopRight => vec![Top, Right].into_iter(),
            BottomLeft => vec![Bottom, Left].into_iter(),
            BottomRight => vec![Bottom, Right].into_iter(),

            _ => vec![self.clone()].into_iter(),
        }
    }
}

