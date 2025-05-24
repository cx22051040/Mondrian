use crate::{
    input::{move_grab::PointerMoveSurfaceGrab, resize_grab::ResizeSurfaceGrab}, manager::workspace::WindowLayout, state::GlobalData
};
use smithay::{
    delegate_xdg_shell,
    desktop::{PopupKind, PopupManager, Space, Window},
    input::{pointer::{Focus, PointerHandle}, Seat},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface}, Resource
        },
    },
    utils::{Coordinate, Point, Rectangle, Serial},
    wayland::{
        compositor::with_states,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
    },
};
use smithay::{
    desktop::{find_popup_root_surface, get_popup_toplevel_coords},
    input::pointer::{CursorIcon, CursorImageStatus, GrabStartData as PointerGrabStartData},
};

/// Should be called on `WlSurface::commit`
pub fn handle_commit(popups: &mut PopupManager, space: &Space<Window>, surface: &WlSurface) {
    // Handle toplevel commits.
    if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().unwrap().wl_surface() == surface)
        .cloned()
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

impl XdgShellHandler for GlobalData {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.state.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface.clone());
        self.window_manager.add_window(
            window.clone(),
            self.workspace_manager.current_workspace().id(),
        );

        self.workspace_manager
            .map_element(None, window, (0, 0).into(), Some(WindowLayout::Tiled), true);
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
        match self.window_manager.remove_window(wl_surface) {
            Some(window) => {
                self.workspace_manager.unmap_element(&window);
            }
            None => {
                warn!("Failed to find window for toplevel destroy");
            }
        }
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
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
        let wl_surface: &WlSurface = surface.wl_surface();
        self.grab_request(wl_surface, &pointer, serial);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
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
        if let Some(start_data) = check_grab(wl_surface, &pointer, serial) {

            let window = match self.workspace_manager.find_window(wl_surface) {
                Some(w) => w.clone(),
                None => {
                    warn!("Failed to find window for move request");
                    return;
                }
            };
            
            let initial_window_location = match self.workspace_manager.element_location(&window) {
                Some(location) => location,
                None => {
                    warn!("Failed to get location from window: {:?}", window);
                    return;
                }
            };

            let initial_window_size = window.geometry().size;

            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            surface.send_pending_configure();

            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                edges.into(),
                Rectangle::new(initial_window_location, initial_window_size),
            );

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}
}
delegate_xdg_shell!(GlobalData);

fn check_grab(
    surface: &WlSurface,
    pointer: &PointerHandle<GlobalData>,
    serial: Serial,
) -> Option<PointerGrabStartData<GlobalData>> {

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

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

    pub fn grab_request(&mut self, wl_surface: &WlSurface, pointer: &PointerHandle<GlobalData>, serial: Serial) {
        if self.input_manager.is_mainmod_pressed {
            if let Some(start_data) = check_grab(wl_surface, pointer, serial) {
                if let Some((window, mut window_rec)) = self.workspace_manager.check_grab(wl_surface) {
                    let window = window.clone();
                    let pointer_loc = start_data.location;

                    window_rec.size.w = window_rec.size.w * 8 / 10;
                    window_rec.size.h = window_rec.size.h * 8 / 10;

                    let initial_window_location = Point::from(
                        (
                            pointer_loc.x - (window_rec.size.w.to_f64() / 2.0), 
                            pointer_loc.y - (window_rec.size.h.to_f64() / 2.0)
                        ))
                        .to_i32_round();

                    // if window is tiled, change it to floating
                    self.workspace_manager.grab_request(&window, Rectangle { loc: initial_window_location, size: window_rec.size });

                    // set pointer state
                    let grab = PointerMoveSurfaceGrab {
                        start_data,
                        window,
                        initial_window_location,
                    };

                    pointer.set_grab(self, grab, serial, Focus::Keep);
        
                    // change cursor image
                    self.cursor_manager
                        .set_cursor_image(CursorImageStatus::Named(CursorIcon::Grabbing));
                }
            }
        }
    }
}

