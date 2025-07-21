use std::time::Duration;

use smithay::{
    desktop::Window, input::pointer::{
        Focus, GrabStartData as PointerGrabStartData, PointerHandle
    }, 
    utils::{
        Logical, Point, Rectangle, Serial, SERIAL_COUNTER
    },
};

use crate::{
    input::{
        focus::{KeyboardFocusTarget, PointerFocusTarget}, move_grab::MoveSurfaceGrab, resize_grab::ResizeSurfaceGrab
    }, 
    layout::{ResizeEdge, WindowLayout}, 
    manager::{animation::{AnimationManager, AnimationType}, window::WindowExt}, 
    state::GlobalData
};

pub mod compositor;
pub mod foreign_toplevel;
pub mod layer_shell;
pub mod xdg_shell;

#[cfg(feature = "xwayland")]
pub mod xwayland;

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
        // is unmapped
        if self.window_manager.set_unmapped(window) {
            self.workspace_manager.unmap_window(window, &mut self.animation_manager);
        }

        self.update_keyboard_focus();
    }

    pub fn destroy_window(&mut self, window: &Window) {
        self.unmap_window(window);
        self.window_manager.remove_unmapped(window);
    }
    
    pub fn switch_layout(&mut self, window: &Window, pointer_loc: Point<f64, Logical>) {
        self.workspace_manager.unmap_window(window, &mut self.animation_manager);

        match window.get_layout() {
            WindowLayout::Tiled => {
                // insert floating window
                self.window_manager.switch_layout(window);

                set_pointer_as_center(window, pointer_loc.to_i32_round(), &mut self.animation_manager);

                self.workspace_manager.map_window(None, window.clone(), ResizeEdge::TopLeft, &mut self.animation_manager);
            }
            WindowLayout::Floating => {
                // insert tiled window
                self.window_manager.switch_layout(window);

                if let Some(focus) = self.window_manager.window_under_tiled(pointer_loc, self.workspace_manager.current_workspace().id()) {
                    let focus_rect = focus.get_rect().unwrap();

                    let edge = detect_pointer_quadrant(pointer_loc, focus_rect.to_f64());
                    self.workspace_manager.map_window(Some(&focus), window.clone(), edge, &mut self.animation_manager);
                } else {
                    self.workspace_manager.map_window(
                        None,
                        window.clone(),
                        ResizeEdge::BottomRight,
                        &mut self.animation_manager,
                    );
                }
            }
        }

    }

    pub fn grab_move_request(
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

        // set as floating window
        if let Some(window) = window.cloned() {
            let initial_layout = window.get_layout();

            match initial_layout {
                WindowLayout::Tiled => {
                    self.switch_layout(&window, start_data.location)
                },
                WindowLayout::Floating => { }
            }

            // set pointer state
            let grab = MoveSurfaceGrab::start(
                start_data,
                initial_layout,
                window,
            );
            
            pointer.set_grab(self, grab, serial, Focus::Clear);
        }

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

fn set_pointer_as_center(target: &Window, pointer_loc: Point<i32, Logical>, animation_manager: &mut AnimationManager) {
    let rect = target.get_rect().unwrap();

    let new_loc = (
        pointer_loc.x - rect.size.w / 4,
        pointer_loc.y - rect.size.h / 4,
    ).into();

    let new_rect = Rectangle::new(new_loc, rect.size/2);

    target.set_rect_cache(new_rect);
    // target.send_rect(rect);

    // TODO: not so good, may conflict
    animation_manager.add_animation(
        target.clone(), 
        rect, 
        new_rect, 
        Duration::from_millis(15), 
        AnimationType::EaseInOutQuad
    );
}