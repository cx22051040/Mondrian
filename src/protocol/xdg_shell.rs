use crate::{
    input::resize_grab::ResizeSurfaceGrab, state::GlobalData
};
use smithay::{
    delegate_xdg_shell, desktop::{PopupKind, Window}, input::{pointer::{Focus, PointerHandle}, Seat}, reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel::{self, ResizeEdge},
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface}, Resource
        },
    }, utils::{Logical, Point, Rectangle, Serial, SERIAL_COUNTER}, wayland::{
        compositor::{self, with_states},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState, XdgToplevelSurfaceData
        },
    }
};
use smithay::{
    desktop::{find_popup_root_surface, get_popup_toplevel_coords},
    input::pointer::GrabStartData as PointerGrabStartData,
};

impl XdgShellHandler for GlobalData {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.state.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let _span = tracy_client::span!("new_xdg_toplevel");

        let window = Window::new_wayland_window(surface.clone());

        self.window_manager.add_window(
            window.clone(),
            self.workspace_manager.current_workspace().id(),
            &mut self.state
        );

        // // use the size from the suggested size of the surface if available
        // if let Some(size) = surface.with_pending_state(|state| state.size) {
        //     window.set_rec(size);
        // }

        // TODO:
        let pointer = self.input_manager.get_pointer();
        let pointer = match pointer {
            Some(k) => k,
            None => {
                error!("get pointer error");
                return;
            }
        };
        let pointer_loc = pointer.current_location();

        let edges = match self.workspace_manager.current_workspace().focus() {
            Some (focus) => {
                let window_rec = self.workspace_manager.window_geometry(focus).unwrap();
                detect_pointer_quadrant(pointer_loc, window_rec.to_f64())
            }
            None => {
                ResizeEdge::None
            }
        };

        self.workspace_manager
            .map_element(
                window.clone(),
                edges,
                true,
                &self.loop_handle,
            );

        self.set_keyboard_focus(Some(surface.wl_surface().clone()), SERIAL_COUNTER.next_serial());
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let wl_surface = surface.wl_surface();

        self.window_manager.get_foreign_handle(wl_surface)
            .map(|handle| {
                handle.send_closed();
            });

        match self.window_manager.remove_window(wl_surface) {
            Some(window) => {
                self.workspace_manager.unmap_element(&window, &self.loop_handle);
            }
            None => {
                warn!("Failed to find window for toplevel destroy");
            }
        }
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        if !self.input_manager.is_mainmod_pressed() {
            return
        }

        let pointer: PointerHandle<Self> = match Seat::from_resource(&seat) {
            Some(seat) => {
                match seat.get_pointer() {
                    Some(pointer) => pointer,
                    None => {
                        warn!("Failed to get pointer from {:?}", seat);
                        return
                    }
                }
            }
            None => {
                warn!("Failed to get seat from {:?}", seat);
                return
            }
        };

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&pointer, wl_surface, serial) {
            self.resize_move_request(wl_surface, &pointer, start_data, serial);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        _edges: xdg_toplevel::ResizeEdge,
    ) {
        if !self.input_manager.is_mainmod_pressed() {
            return
        }

        let pointer: PointerHandle<Self> = match Seat::from_resource(&seat) {
            Some(seat) => {
                match seat.get_pointer() {
                    Some(pointer) => pointer,
                    None => {
                        warn!("Failed to get pointer from {:?}", seat);
                        return
                    }
                }
            }
            None => {
                warn!("Failed to get seat from {:?}", seat);
                return
            }
        };

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&pointer, wl_surface, serial) {
            // send resize state
            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });
            surface.send_pending_configure();
            
            self.resize_move_request(wl_surface, &pointer, start_data, serial);
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    fn title_changed(&mut self, surface: ToplevelSurface) {
        let (title, app_id) = 
            compositor::with_states(surface.wl_surface(), |states| {
                let roll= &mut states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap();
                (roll.title.clone(), roll.app_id.clone())
            });

        self.window_manager.get_foreign_handle(surface.wl_surface())
            .map(|handle| {
                handle.send_title(&title.unwrap_or("unkown".to_string()));
                handle.send_app_id(&app_id.unwrap_or("unkown".to_string()));
                handle.send_done();
            });
    }
}
delegate_xdg_shell!(GlobalData);

impl GlobalData {
    pub fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .workspace_manager
            .find_window(&root)
        else {
            return;
        };

        let output = self.output_manager.current_output();
        let output_geo = match self.output_manager.output_geometry(&output) {
            Some(g) => g,
            None => {
                warn!("Failed to get output {:?} geometry", output);
                return;
            }
        };

        let window_geo = match self.workspace_manager.window_geometry(window) {
            Some(g) => g,
            None => {
                warn!("Failed to get window {:?} geometry", window);
                return;
            }
        };

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;
        
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }

    pub fn grab_move_request(&mut self, wl_surface: &WlSurface, _pointer: &PointerHandle<GlobalData>, _start_data: PointerGrabStartData<GlobalData>, _serial: Serial) {
        if let Some((..)) = self.workspace_manager.check_grab(wl_surface) {
            // TODO
        }
    }

    pub fn resize_move_request(&mut self, wl_surface: &WlSurface, pointer: &PointerHandle<GlobalData>, start_data: PointerGrabStartData<GlobalData>, serial: Serial) {
        if let Some((window, window_rec)) = self.workspace_manager.check_grab(wl_surface) {
            let window = window.clone();
            let pointer_loc = start_data.location;

            let edges = detect_pointer_quadrant(pointer_loc, window_rec.to_f64());

            // set pointer state
            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                edges,
                window_rec,
            );
            
            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    pub fn xdg_shell_handle_commit(&mut self, surface: &WlSurface) {
        let popups = &mut self.popups;

        // Handle toplevel commits.
        if let Some(window) = self.workspace_manager.find_window(surface)
        {
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });

            if !initial_configure_sent {
                window.toplevel().unwrap().send_configure();
            }
        }

        // Handle popup commits.
        popups.commit(surface);
        if let Some(popup) = popups.find_popup(surface) {
            match popup {
                PopupKind::Xdg(ref xdg) => {
                    if !xdg.is_initial_configure_sent() {
                        // NOTE: This should never fail as the initial configure is always
                        // allowed.
                        xdg.send_configure().expect("initial configure failed");
                    }
                }
                PopupKind::InputMethod(ref _input_method) => {}
            }
        }
    }

}

fn check_grab(
    pointer: &PointerHandle<GlobalData>, 
    wl_surface: &WlSurface, 
    serial: Serial
) -> Option<PointerGrabStartData<GlobalData>> {
    if !pointer.has_grab(serial) {
        warn!("pointer don't have grab state");
        return None;
    }

    let start_data = match pointer.grab_start_data() {
        Some(start_data) => start_data,
        None => {
            warn!("Failed to get start_data from: {:?}", pointer);
            return None;
        }
    };

    let focus= match start_data.focus.as_ref() {
        Some((focus, _)) => focus,
        None => {
            warn!("Failed to get start_data from: {:?}", pointer);
            return None;
        }
    };

    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&wl_surface.id()) {
        warn!("the focus was for a different surface");
        return None;
    }

    Some(start_data)
}

fn detect_pointer_quadrant(
    pointer_loc: Point<f64, Logical>,
    window_rec: Rectangle<f64, Logical>,
) -> ResizeEdge {
    let center_x = window_rec.loc.x + window_rec.size.w / 2.0;
    let center_y = window_rec.loc.y + window_rec.size.h / 2.0;

    let dx = pointer_loc.x - center_x;
    let dy = pointer_loc.y - center_y;

    match (dx >= 0., dy >= 0.) {
        (true, false) => ResizeEdge::TopRight,
        (false, false) => ResizeEdge::TopLeft,
        (false, true) => ResizeEdge::BottomLeft,
        (true, true) => ResizeEdge::BottomRight,
    }
}