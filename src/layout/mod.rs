use serde::{Deserialize, Serialize};

pub mod tiled_tree;
pub mod json_tiled_tree;


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

    pub fn _opposite(&self) -> Direction {
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

    pub fn _orthogonal(&self) -> [Direction; 2] {
        match self {
            Direction::Left | Direction::Right => [Direction::Up, Direction::Down],
            Direction::Up | Direction::Down => [Direction::Left, Direction::Right],
        }
    }

    pub const ALL: [Direction; 4] = [
        Direction::Right,
        Direction::Down,
        Direction::Left,
        Direction::Up,
    ];
}
