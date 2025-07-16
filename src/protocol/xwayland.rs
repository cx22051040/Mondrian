use std::process::Stdio;

#[cfg(feature = "xwayland")]
use smithay::{delegate_xwayland_keyboard_grab, delegate_xwayland_shell, wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabHandler};
use smithay::{
    desktop::{Window, WindowSurface}, reexports::wayland_server::protocol::wl_surface::WlSurface, utils::{Logical, Rectangle}, wayland::{compositor::CompositorHandler, xwayland_shell::{XWaylandShellHandler, XWaylandShellState}}, xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId}, X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler
    }
};

use crate::{input::focus::KeyboardFocusTarget, state::GlobalData};

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

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) { }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) { }

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        window.set_mapped(true).unwrap();
        let window = Window::new_x11_window(window);
        
        self.map_window(window);
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let _location = window.geometry().loc;
        let _window = Window::new_x11_window(window);

        // TODO: this window don't need tiled
        // use float to map it
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.unmap_window(&WindowSurface::X11(window));
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) { }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _x: Option<i32>,
        _y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // we just set the new size, but don't let windows move themselves around freely
        let mut geo = window.geometry();
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        let _ = window.configure(geo);
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _geometry: Rectangle<i32, Logical>,
        _above: Option<x11rb::protocol::xproto::Window>,
    ) {
        // TODO
    }

    fn resize_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32, _resize_edge: X11ResizeEdge) {
        // TODO
    }

    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {
        // TODO
    }
}

impl XWaylandKeyboardGrabHandler for GlobalData {
    fn keyboard_focus_for_xsurface(&self, surface: &WlSurface) -> Option<Self::KeyboardFocus> {
        let window = self.window_manager.get_window_wayland(surface)?;
        Some(KeyboardFocusTarget::Window(window.clone()))
    }
}

delegate_xwayland_shell!(GlobalData);

delegate_xwayland_keyboard_grab!(GlobalData);