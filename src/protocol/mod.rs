use smithay::{
    desktop::Window, input::pointer::{
        Focus, GrabStartData as PointerGrabStartData, PointerHandle
    }, 
    reexports::wayland_server::protocol::wl_surface::WlSurface, 
    utils::{
        Logical, Point, Rectangle, Serial, SERIAL_COUNTER
    },
};

use crate::{
    input::{
        focus::{KeyboardFocusTarget, PointerFocusTarget}, 
        resize_grab::ResizeSurfaceGrab,
    }, 
    layout::ResizeEdge, 
    manager::window::WindowExt, 
    state::GlobalData
};

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
    pub fn map_window(&mut self, window: Window) -> bool {
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
                return false;
            }
        };
        let pointer_loc = pointer.current_location();

        let edge = if let Some(KeyboardFocusTarget::Window(window)) = self.input_manager.get_keyboard_focus() {
            detect_pointer_quadrant(pointer_loc, window.get_rect().unwrap().to_f64())
        } else {
            ResizeEdge::None
        };

        self.workspace_manager.map_window(
            target.as_ref(),
            window,
            edge,
            &mut self.animation_manager,
        )
    }

    pub fn set_mapped(&mut self, window: &Window) {
        self.window_manager.set_mapped(window);
        self.set_keyboard_focus(Some(window.clone().into()), SERIAL_COUNTER.next_serial());
    }

    pub fn unmap_window(&mut self, window: &Window) {
        self.window_manager.set_unmapped(window);
        self.workspace_manager.unmap_window(window, &mut self.animation_manager);
    }

    pub fn destroy_window(&mut self, window: &Window) {
        self.unmap_window(window);
        self.window_manager.remove_unmapped(window);

        self.update_keyboard_focus();
    }
    
    pub fn _grab_move_request(&mut self, _wl_surface: &WlSurface, _pointer: &PointerHandle<GlobalData>, _start_data: PointerGrabStartData<GlobalData>, _serial: Serial) {
        // TODO
    }

    pub fn resize_move_request(
        &mut self, 
        surface: &PointerFocusTarget, 
        pointer: &PointerHandle<GlobalData>, 
        start_data: PointerGrabStartData<GlobalData>, 
        serial: Serial
    ) {
        let window = match surface {
            PointerFocusTarget::WlSurface(wl_surface) => {
                // send resize state
                self.window_manager.get_mapped(&wl_surface.clone().into())
            },
            PointerFocusTarget::X11Surface(x11_surface) => {
                self.window_manager.get_mapped(&x11_surface.clone().into())
            }
        };

        if let Some(window) = window {
            let window_rect = window.get_rect().unwrap();
            
            let pointer_loc = start_data.location;
    
            let edge = detect_pointer_quadrant(pointer_loc, window_rect.to_f64());
    
            // set pointer state
            let grab = ResizeSurfaceGrab::start(
                start_data,
                window.clone(),
                edge,
                window_rect,
            );
            
            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }
}