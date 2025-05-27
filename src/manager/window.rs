use std::collections::HashMap;

use smithay::{
    desktop::Window,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Size}, wayland::{compositor, foreign_toplevel_list::ForeignToplevelHandle, shell::xdg::XdgToplevelSurfaceData},
};

use crate::state::{GlobalData, State};

use super::workspace::WorkspaceId;

pub trait WindowExt {
    fn set_rec(&self, size: Size<i32, Logical>);
    fn get_title_and_id(&self) -> Option<(Option<String>, Option<String>)>;
}

impl WindowExt for Window {
    fn set_rec(&self, size: Size<i32, Logical>) {
        self.toplevel()
            .unwrap()
            .with_pending_state(|state| state.size = Some(size));

        self.toplevel().unwrap().send_pending_configure();
    }

    fn get_title_and_id(&self) -> Option<(Option<String>, Option<String>)> {
        self.toplevel()
            .and_then(|toplevel| {
                compositor::with_states(toplevel.wl_surface(), |states| {
                    let roll= &mut states
                        .data_map
                        .get::<XdgToplevelSurfaceData>()
                        .unwrap()
                        .lock()
                        .unwrap();
                    Some((roll.title.clone(), roll.app_id.clone()))
                })
            })
    }
}

pub struct WindowManager {
    pub windows: Vec<Window>,
    pub window_workspace: HashMap<Window, WorkspaceId>,
    pub foreign_handle: HashMap<WlSurface, ForeignToplevelHandle>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            window_workspace: HashMap::new(),
            foreign_handle: HashMap::new(),
        }
    }

    pub fn get_window(&self, surface: &WlSurface) -> Option<&Window> {
        self.windows
            .iter()
            .find(|w| w.toplevel().unwrap().wl_surface() == surface)
    }

    pub fn add_window(&mut self, window: Window, workspace_id: WorkspaceId, state: &mut State) {
        let handle = state
            .foreign_toplevel_state
            .new_toplevel::<GlobalData>(
                "unkown", 
                "unkown"
            );
        self.foreign_handle.insert(window.toplevel().unwrap().wl_surface().clone(), handle);

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
        self.foreign_handle.remove(window.toplevel().unwrap().wl_surface());

        if let Some(pos) = self.windows.iter().position(|w| w == &window) {
            return Some(self.windows.remove(pos));
        }

        None
    }

    pub fn get_foreign_handle(&self, surface: &WlSurface) -> Option<&ForeignToplevelHandle> {
        self.foreign_handle.get(surface)
    }
}
