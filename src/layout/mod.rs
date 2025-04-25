use std::cell::RefCell;

use smithay::{desktop::Window, utils::{Logical, Rectangle}};

use crate::space::window::WindowExtElements;

pub mod tiled_tree;

pub trait LayoutHandle {
    fn set_rec(&self, new_rec: Rectangle<i32, Logical>);
    fn get_rec(&self) -> Option<Rectangle<i32, Logical>>;
}

impl LayoutHandle for Window {
    fn set_rec(&self, new_rec: Rectangle<i32, Logical>) {
        if let Some(e) = self
            .user_data()
            .get::<RefCell<WindowExtElements>>() 
        {
            e.borrow_mut().rec = new_rec;
        }
    }

    fn get_rec(&self) -> Option<Rectangle<i32, Logical>> {
        self.
            user_data()
            .get::<RefCell<WindowExtElements>>()
            .and_then(|e| Some(e.borrow().rec.clone()))
    }
}