use std::process::Stdio;

#[cfg(feature = "xwayland")]
use smithay::{delegate_xwayland_keyboard_grab, delegate_xwayland_shell, wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabHandler};
use smithay::{
    desktop::Window, reexports::wayland_server::protocol::wl_surface::WlSurface, utils::{Logical, Rectangle, SERIAL_COUNTER}, wayland::{compositor::CompositorHandler, xwayland_shell::{XWaylandShellHandler, XWaylandShellState}}, xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, WmWindowType, XwmId}, X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler
    }
};

use crate::{input::focus::{KeyboardFocusTarget, PointerFocusTarget}, layout::WindowLayout, manager::window::WindowExt, state::GlobalData};

impl GlobalData {
    pub fn start_xwayland(&mut self) {
        let (xwayland, client) = XWayland::spawn(
            &self.display_handle, 
            None, 
            std::iter::empty::<(String, String)>(), 
            true, 
            Stdio::null(), 
            Stdio::null(), 
            |_| (),
        ).expect("Failed to start XWayland");

        let result = self.loop_handle.insert_source(
            xwayland, 
            move |event, _, data| match event {
                XWaylandEvent::Ready { x11_socket, display_number } => {
                    // TODO:
                    let xwayland_scale = 1.0;
                    data.client_compositor_state(&client)
                        .set_client_scale(xwayland_scale);

                    let wm = X11Wm::start_wm(
                        data.loop_handle.clone(),
                        x11_socket, 
                        client.clone()
                    ).expect("Failed to attach X11 Window Manager");

                    data.state.xwm = Some(wm);
                    data.state.xdisplay = Some(display_number);
                },
                XWaylandEvent::Error => {
                    error!("XWayland crashed on startup");
                }
            }
        );

        if let Err(e) = result {
            error!("Failed to insert the XWaylandSource into the event loop: {}", e);
        } else {
            info!("XWayland started")
        }
    }
}

impl XWaylandShellHandler for GlobalData {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.state.xwayland_shell_state
    }
}

impl XwmHandler for GlobalData {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.state.xwm.as_mut().unwrap()
    }

    fn new_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        // judge layout
        // TODO: a wired bug, some client may start many no type windows
        // and the windows has no info, cannot judge layout
        let layout = if let Some(window_type) = surface.window_type() {
            match window_type {
                WmWindowType::Normal => {
                    if surface.is_popup() || surface.is_transient_for().is_some() {
                        WindowLayout::Floating
                    } else {
                        WindowLayout::Tiled
                    } 
                }
                _ => {
                    WindowLayout::Floating
                }
            }
        } else {
            WindowLayout::Floating
        };

        // create new window
        let window = Window::new_x11_window(surface);
        window.set_layout(layout);

        // add unmapped window in window_manager
        self.window_manager.add_window_unmapped(
            window.clone(),
            self.workspace_manager.current_workspace().id()
        );
    }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        let layout = WindowLayout::Floating;

        // create new window
        let window = Window::new_x11_window(surface);
        window.set_layout(layout);

        // add unmapped window in window_manager
        self.window_manager.add_window_unmapped(
            window.clone(),
            self.workspace_manager.current_workspace().id()
        );
    }

    fn map_window_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        surface.set_mapped(true).unwrap();

        if let Some(window) = self.window_manager.get_unmapped(&surface.into()).cloned() {
            self.set_mapped(&window);
            self.map_window(window);
        }
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) { }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(window) = self.window_manager.get_mapped(&window.into()) {
            self.unmap_window(&window.clone());
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(window) = self.window_manager.get_mapped(&window.into()) {
            self.destroy_window(&window.clone());
        }
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        surface: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // we just set the new size, but don't let windows move themselves around freely
        if let Some(window) = self.window_manager.get_unmapped(&surface.clone().into()) {
            let mut rect = window.geometry();
            if let Some(x) = x {
                rect.loc.x = x;
            }
            if let Some(y) = y {
                rect.loc.y = y;
            }
            if let Some(w) = w {
                rect.size.w = w as i32;
            }
            if let Some(h) = h {
                rect.size.h = h as i32;
            }
            window.set_rect_cache(rect);
            window.send_rect(rect);
        } else if let Some(window) = self.window_manager.get_mapped(&surface.clone().into()) {
            match window.get_layout() {
                WindowLayout::Floating => {
                    let mut rect = window.geometry();
                    if let Some(x) = x {
                        rect.loc.x = x;
                    }
                    if let Some(y) = y {
                        rect.loc.y = y;
                    }
                    if let Some(w) = w {
                        rect.size.w = w as i32;
                    }
                    if let Some(h) = h {
                        rect.size.h = h as i32;
                    }
                    window.set_rect_cache(rect);
                    window.send_rect(rect);
                }
                WindowLayout::Tiled => {
                    let rect = window.get_rect();
                    let _ = surface.configure(rect);
                }
            }
        }
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _geometry: Rectangle<i32, Logical>,
        _above: Option<x11rb::protocol::xproto::Window>,
    ) {
        // modify cache
        // info!("configure_notify");
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(window) = self.window_manager.get_mapped(&surface.clone().into()) {
            let output = self.output_manager.current_output();
            let output_rect = self.output_manager.output_geometry(output).unwrap();
            
            let _ = surface.configure(output_rect);
            
            surface.set_fullscreen(true).unwrap();
            self.fullscreen(window, output);
        }
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(window) = self.window_manager.get_mapped(&surface.clone().into()) {
            surface.set_fullscreen(false).unwrap();

            if let Some(rect) = window.get_rect() {
                let _ = surface.configure(rect);
            }

            let output = self.output_manager.current_output().clone();
            self.unfullscreen(&output);
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32, _resize_edge: X11ResizeEdge) {
        if !self.input_manager.is_mainmod_pressed() {
            return
        }

        let pointer = match self.input_manager.get_pointer() {
            Some(pointer) => pointer,
            None => {
                warn!("Failed to get pointer");
                return
            }
        };
        
        let start_data = match pointer.grab_start_data() {
            Some(start_data) => start_data,
            None => {
                warn!("Failed to get start_data from: {:?}", pointer);
                return;
            }
        };

        self.resize_move_request(&PointerFocusTarget::X11Surface(window), &pointer, start_data, SERIAL_COUNTER.next_serial());
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        if !self.input_manager.is_mainmod_pressed() {
            return
        }

        let pointer = match self.input_manager.get_pointer() {
            Some(pointer) => pointer,
            None => {
                warn!("Failed to get pointer");
                return
            }
        };
        
        let start_data = match pointer.grab_start_data() {
            Some(start_data) => start_data,
            None => {
                warn!("Failed to get start_data from: {:?}", pointer);
                return;
            }
        };

        self.grab_move_request(&PointerFocusTarget::X11Surface(window), &pointer, start_data, SERIAL_COUNTER.next_serial());
    }
}

impl XWaylandKeyboardGrabHandler for GlobalData {
    fn keyboard_focus_for_xsurface(&self, surface: &WlSurface) -> Option<Self::KeyboardFocus> {
        let window = self.window_manager.get_mapped(&surface.clone().into())?;
        Some(KeyboardFocusTarget::Window(window.clone()))
    }
}

delegate_xwayland_shell!(GlobalData);

delegate_xwayland_keyboard_grab!(GlobalData);