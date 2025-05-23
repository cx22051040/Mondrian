use std::{
    collections::HashMap,
};

use smithay::{
    desktop::Window,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Size},
};

use super::workspace::WorkspaceId;

pub trait WindowExt {
    fn set_rec(&self, size: Size<i32, Logical>);
}

impl WindowExt for Window {
    fn set_rec(&self, size: Size<i32, Logical>) {
        self.toplevel()
            .unwrap()
            .with_pending_state(|state| state.size = Some(size));

        self.toplevel().unwrap().send_pending_configure();
    }
}

pub struct WindowManager {
    pub windows: Vec<Window>,
    pub window_workspace: HashMap<Window, WorkspaceId>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            window_workspace: HashMap::new(),
        }
    }

    pub fn get_window(&self, surface: &WlSurface) -> Option<&Window> {
        self.windows
            .iter()
            .find(|w| w.toplevel().unwrap().wl_surface() == surface)
    }

    pub fn add_window(&mut self, window: Window, workspace_id: WorkspaceId) {
        self.window_workspace.insert(window.clone(), workspace_id);
        self.windows.push(window);
    }

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<Window> {
        let window = match self.get_window(surface) {
            Some(window) => window.clone(),
            None => {
                warn!("Failed to get window");
                return None
            }
        };

        self.window_workspace.remove(&window);

        if let Some(pos) = self.windows.iter().position(|w| w == &window) {
            return Some(self.windows.remove(pos));
        }

        None
    }
}
