use smithay::{desktop::{Window, WindowSurface}, utils::{Logical, Point, Rectangle, SERIAL_COUNTER}};

use crate::{input::focus::KeyboardFocusTarget, layout::ResizeEdge, manager::window::WindowExt, state::GlobalData};

pub mod compositor;
pub mod foreign_toplevel;
pub mod layer_shell;
pub mod xdg_shell;

#[cfg(feature = "xwayland")]
pub mod xwayland;

pub fn detect_pointer_quadrant(
    pointer_loc: Point<f64, Logical>,
    window_rect: Rectangle<f64, Logical>,
) -> ResizeEdge {
    let center_x = window_rect.loc.x + window_rect.size.w / 2.0;
    let center_y = window_rect.loc.y + window_rect.size.h / 2.0;

    let dx = pointer_loc.x - center_x;
    let dy = pointer_loc.y - center_y;

    match (dx >= 0., dy >= 0.) {
        (true, false) => ResizeEdge::TopRight,
        (false, false) => ResizeEdge::TopLeft,
        (false, true) => ResizeEdge::BottomLeft,
        (true, true) => ResizeEdge::BottomRight,
    }
}

impl GlobalData {
    pub fn map_window(&mut self, window: Window) {
        // add window in window_manager
        self.window_manager.add_window(
            window.clone(),
            self.workspace_manager.current_workspace().id(),
            &mut self.state
        );

        // use the size from the suggested size of the surface if available
        // if let Some(size) = surface.with_pending_state(|state| state.size) {
        //     window.set_rec(size);
        // }

        // map window for current workspace
        let target = self
            .input_manager
            .get_keyboard_focus()
            .and_then(|focus| {
                if let KeyboardFocusTarget::Window(w) = focus {
                    Some(w)
                } else {
                    None
                }
            });
        
        let pointer = self.input_manager.get_pointer();
        let pointer = match pointer {
            Some(k) => k,
            None => {
                error!("get pointer error");
                return;
            }
        };
        let pointer_loc = pointer.current_location();

        let edge = if let Some(KeyboardFocusTarget::Window(window)) = self.input_manager.get_keyboard_focus() {
            detect_pointer_quadrant(pointer_loc, window.get_rect().to_f64())
        } else {
            ResizeEdge::None
        };

        self.workspace_manager
            .map_window(
                target.as_ref(),
                window.clone(),
                edge,
                true,
                &self.loop_handle,
            );

        // set focus
        self.set_keyboard_focus(Some(window.clone().into()), SERIAL_COUNTER.next_serial());

        // xwayland config
        #[cfg(feature = "xwayland")]
        if let Some(xsurface) = window.x11_surface() {
            xsurface.configure(Some(window.get_rect())).unwrap();
        }
    }

    pub fn unmap_window(&mut self, surface: &WindowSurface) {
        // self.window_manager.get_foreign_handle(wl_surface)
        //     .map(|handle| {
        //         handle.send_closed();
        //     });
        match surface {
            WindowSurface::Wayland(toplevel) => {
                let window = match self.window_manager.get_window_wayland(toplevel.wl_surface()) {
                    Some(window) => window.clone(),
                    None => {
                        warn!("Failed to get window");
                        return;
                    }
                };

                match self.window_manager.remove_window(&window) {
                    Some(target) => {
                        self.workspace_manager.unmap_window(&target, &self.loop_handle);
                    }
                    None => {
                        warn!("Failed to find window for toplevel destroy");
                    }
                }
            },
            #[cfg(feature = "xwayland")]
            WindowSurface::X11(x11_surface) => {
                let window = match self.window_manager.get_window_xwayland(x11_surface) {
                    Some(window) => window.clone(),
                    None => {
                        warn!("Failed to get window");
                        return;
                    }
                };

                match self.window_manager.remove_window(&window) {
                    Some(target) => {
                        self.workspace_manager.unmap_window(&target, &self.loop_handle);
                    }
                    None => {
                        warn!("Failed to find window for toplevel destroy");
                    }
                }
            }
        }

        self.update_keyboard_focus();
    }

}