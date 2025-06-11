use serde::{Deserialize, Serialize};

pub mod json_tiled_tree;
pub mod neighbor_graph;
pub mod tiled_tree;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Direction {
    Left,
    Up,
    Right,
    Down,
}

impl Direction {
    pub fn default() -> Direction {
        Direction::Right
    }

    pub fn opposite(&self) -> Direction {
        match self {
            Direction::Right => Direction::Left,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Up => Direction::Down,
        }
    }

    pub fn rotate_cw(&self) -> Direction {
        match self {
            Direction::Right => Direction::Down,
            Direction::Down => Direction::Left,
            Direction::Left => Direction::Up,
            Direction::Up => Direction::Right,
        }
    }

    pub fn horizontals() -> [Direction; 2] {
        [Direction::Left, Direction::Right]
    }

    pub fn verticals() -> [Direction; 2] {
        [Direction::Up, Direction::Down]
    }

    pub fn orthogonal(&self) -> [Direction; 2] {
        match self {
            Direction::Left | Direction::Right => Direction::verticals(),
            Direction::Up | Direction::Down => Direction::horizontals(),
        }
    }

    pub const ALL: [Direction; 4] = [
        Direction::Right,
        Direction::Down,
        Direction::Left,
        Direction::Up,
    ];
}
