use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use smithay::desktop::Window;

use super::workspace::WorkspaceID;

static NEXT_WINDOW_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowID(usize);

impl WindowID {
    #[inline]
    pub fn next() -> Self {
        Self(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub trait WindowExt {
    fn set_window_id(&mut self) -> WindowID;
    fn get_window_id(&self) -> Option<WindowID>;
}

impl WindowExt for Window {
    fn set_window_id(&mut self) -> WindowID {
        let id = WindowID::next();
        self.user_data().insert_if_missing(|| id);
        id
    }

    fn get_window_id(&self) -> Option<WindowID> {
        self
            .user_data()
            .get::<WindowID>()
            .and_then(|id| Some(id.clone()))
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
        let id = WindowID::next();
        window.user_data().insert_if_missing(|| id);
        id
    }

    pub fn get_window_id(&self, window: &Window) -> Option<WindowID> {
        window
            .user_data()
            .get::<WindowID>()
            .and_then(|id| Some(id.clone()))
    }

    pub fn get_window(&self, id: WindowID) -> Option<&Window> {
        self.windows
            .iter()
            .find(|w| w.get_window_id() == Some(id))
    }

    pub fn add_window(&mut self, window: Window, workspace_id: WorkspaceID) {
        let id = self.set_window_id(&window);
        self.windows.push(window);
        self.window_workspace.insert(id, workspace_id);
    }

    pub fn _remove_window(&mut self) {
        todo!()
    }
}

