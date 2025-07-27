use std::{cell::RefCell, time::Duration};

use smithay::{
    desktop::{layer_map_for_output, LayerSurface, Window}, input::pointer::{
        Focus, GrabStartData as PointerGrabStartData, PointerHandle
    }, output::Output, utils::{
        Logical, Point, Rectangle, Serial, SERIAL_COUNTER
    }, wayland::shell::wlr_layer::Layer
};

use crate::{
    input::{
        focus::PointerFocusTarget, move_grab::MoveSurfaceGrab, resize_grab::ResizeSurfaceGrab
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

#[derive(Default)]
pub struct FullscreenSurface(RefCell<Option<(Window, Vec<LayerSurface>)>>);

impl FullscreenSurface {
    pub fn set(&self, window: Window, layer_surfaces: Vec<LayerSurface>) {
        *self.0.borrow_mut() = Some((window, layer_surfaces));
    }

    pub fn get(&self) -> Option<(Window, Vec<LayerSurface>)> {
        self.0.borrow_mut().clone()
    }

    pub fn clear(&self) -> Option<(Window, Vec<LayerSurface>)> {
        self.0.borrow_mut().take()
    }
}

impl GlobalData {
    pub fn map_window(&mut self, window: Window) -> bool {
        // fake fullscreen, no border fullscreen
        if let Some(is_fullscreen) = self.window_manager.get_fullscreen(&window) {
            if is_fullscreen {
                window.set_layout(WindowLayout::Floating);
                self.window_manager.raise_window(&window);

                let output = self.output_manager.current_output();
                let output_rect = self.output_manager.output_geometry(&output).unwrap();
                window.set_rect_cache(output_rect);
                window.send_rect(output_rect);

                self.fullscreen(&window, output);
            }
        }
        
        // map window for current workspace
        let pointer = self.input_manager.get_pointer();
        let pointer = match pointer {
            Some(k) => k,
            None => {
                error!("get pointer error");
                return false;
            }
        };
        let pointer_loc = pointer.current_location();

        let target_tiled = self.window_manager.window_under_tiled(pointer_loc, self.workspace_manager.current_workspace().id());

        let edge = if let Some(target_tiled) = &target_tiled {
            detect_pointer_quadrant(pointer_loc, target_tiled.get_rect().unwrap().to_f64())
        } else {
            ResizeEdge::None
        };

        self.workspace_manager.map_window(
            target_tiled.as_ref(),
            window.clone(),
            edge,
            &mut self.animation_manager,
        )
    }

    pub fn set_mapped(&mut self, window: &Window) {
        self.window_manager.set_mapped(window);
        self.set_keyboard_focus(Some(window.clone().into()), SERIAL_COUNTER.next_serial());
    }

    pub fn unmap_window(&mut self, window: &Window) {
        if let Some(is_fullscreen) = self.window_manager.get_fullscreen(window) {
            if is_fullscreen {
                let output = self.output_manager.current_output().clone();
                self.unfullscreen(&output);
            }
        }

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

    pub fn fullscreen(&self, window: &Window, output: &Output) {
        output.user_data().insert_if_missing(FullscreenSurface::default);

        // hide layer-shell surface
        let mut map = layer_map_for_output(output);
        let mut layer_surfaces = vec![];
        
        for level in [Layer::Overlay, Layer::Top] {
            layer_surfaces.extend(
                map.layers_on(level).cloned()
            );
        }
        for layer_surface in &layer_surfaces {
            map.unmap_layer(layer_surface);
        }
        
        output
            .user_data()
            .get::<FullscreenSurface>()
            .unwrap()
            .set(window.clone(), layer_surfaces);
    }
    
    pub fn unfullscreen(&mut self, output: &Output) {
        if let Some(fullscreen) = output.user_data().get::<FullscreenSurface>() {
            if let Some((_, layer_surfaces)) = fullscreen.get() {
                // restore layer-shell surfaces
                let mut map = layer_map_for_output(output);

                for layer_surface in &layer_surfaces {
                    map.map_layer(layer_surface).unwrap();
                }

                let output_working_geo = map.non_exclusive_zone();
                self.workspace_manager
                    .update_output_rect(output_working_geo, &mut self.animation_manager);
            }

            fullscreen.clear();
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
        pointer_loc.x - rect.size.w * 3 / 8,
        pointer_loc.y - rect.size.h * 3 / 8,
    ).into();

    let new_rect = Rectangle::new(new_loc, rect.size*3/4);

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