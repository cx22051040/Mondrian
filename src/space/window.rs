use std::{
    cell::RefCell, collections::HashMap, sync::atomic::{AtomicUsize, Ordering}
};

use smithay::{desktop::Window, reexports::wayland_server::protocol::wl_surface::WlSurface, utils::{Logical, Rectangle}};

use super::workspace::WorkspaceID;

static NEXT_WINDOW_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowID(usize);

pub struct WindowExtElements {
    id: WindowID,
    rec: Rectangle<i32, Logical>,
}

impl WindowID {
    #[inline]
    pub fn next() -> Self {
        Self(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub trait WindowExt {
    fn set_id(&self) -> WindowID;
    fn get_id(&self) -> Option<WindowID>;
    fn set_rec(&self, new_rec: Rectangle<i32, Logical>);
    fn get_rec(&self) -> Option<Rectangle<i32, Logical>>;
}

impl WindowExt for Window {
    fn set_id(&self) -> WindowID {
        let id = WindowID::next();
        self.user_data().insert_if_missing(|| RefCell::new(WindowExtElements {id, rec: Rectangle::default()}));
        id
    }

    fn get_id(&self) -> Option<WindowID> {
        self
            .user_data()
            .get::<RefCell<WindowExtElements>>()
            .and_then(|e| Some(e.borrow().id.clone()))
    }

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

pub struct WindowManager {
    pub windows: Vec<Window>,
    pub window_workspace: HashMap<WindowID, WorkspaceID>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            window_workspace: HashMap::new(),
        }
    }

    pub fn set_window_id(&mut self, window: &Window) -> WindowID {
        window.set_id()
    }

    pub fn get_window_id(&self, window: &Window) -> Option<WindowID> {
        window.get_id()
    }

    pub fn get_window(&self, surface: &WlSurface) -> Option<&Window> {
        self.windows
            .iter()
            .find(|w| w.toplevel().unwrap().wl_surface() == surface)
    }

    pub fn add_window(&mut self, window: Window, workspace_id: WorkspaceID) {
        let id = self.set_window_id(&window);
        self.windows.push(window);
        self.window_workspace.insert(id, workspace_id);
    }

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<Window> {
        let window = self.get_window(surface).unwrap().clone();
    
        if let Some(window_id) = self.get_window_id(&window) {
            self.window_workspace.remove(&window_id);
        }

        if let Some(pos) = self.windows.iter().position(|w| w == &window) {
            return Some(self.windows.remove(pos));
        }

        None
    }
    
}

