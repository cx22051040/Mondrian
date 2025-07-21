use std::cell::RefCell;

use crate::{
    input::focus::PointerFocusTarget, layout::WindowLayout, manager::window::{CustomWindowSurface, WindowExt}, state::GlobalData
};
use smithay::{
    backend::renderer::utils::with_renderer_surface_state, delegate_xdg_shell, desktop::{layer_map_for_output, LayerSurface, PopupKind, Window}, input::{pointer::PointerHandle, Seat}, output::Output, reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_output::WlOutput, wl_seat, wl_surface::WlSurface}, Resource
        },
    }, utils::Serial, wayland::{
        compositor::{self, with_states},
        shell::{wlr_layer::Layer, xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState, XdgToplevelSurfaceData
        }},
    }
};
use smithay::{
    desktop::{find_popup_root_surface, get_popup_toplevel_coords},
};

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

impl XdgShellHandler for GlobalData {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.state.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let _span = tracy_client::span!("new_xdg_toplevel");

        let window = Window::new_wayland_window(surface.clone());
        window.set_layout(WindowLayout::default());

        // add unmapped window in window_manager
        self.window_manager.add_window_unmapped(
            window,
            self.workspace_manager.current_workspace().id()
        );
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        let _span = tracy_client::span!("new_xdg_popup");

        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.window_manager.get_mapped(&surface.into()) {
            self.destroy_window(&window.clone());
        }
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

    fn fullscreen_request(&mut self, surface: ToplevelSurface, mut wl_output: Option<WlOutput>) {
        if surface
            .current_state()
            .capabilities
            .contains(xdg_toplevel::WmCapabilities::Fullscreen)
        {
            let wl_surface = surface.wl_surface();

            let output = wl_output
                .as_ref()
                .and_then(|wl_output| Output::from_resource(&wl_output))
                .or_else(|| {
                    // If no output was specified, use the current output
                    Some(self.output_manager.current_output().clone())
                });

            if let Some(output) = output {
                let output_geo = self.output_manager.output_geometry(&output).unwrap();
                let client = match self.display_handle.get_client(wl_surface.id()) {
                    Ok(client) => client,
                    Err(err) => {
                        warn!("Failed to get client for surface {:?}: {:?}", wl_surface, err);
                        return;
                    }
                };

                for client_output in output.client_outputs(&client) {
                    wl_output = Some(client_output);
                }

                let window = self.window_manager.get_mapped(&wl_surface.clone().into()).unwrap();

                surface.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Fullscreen);
                    state.size = Some(output_geo.size);
                    state.fullscreen_output = wl_output;
                });
                
                output.user_data().insert_if_missing(FullscreenSurface::default);

                // hide layer-shell surface
                let mut map = layer_map_for_output(&output);
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

            // The protocol demands us to always reply with a configure,
            // regardless of we fulfilled the request or not
            if surface.is_initial_configure_sent() {
                surface.send_configure();
            } else {
                // Will be sent during initial configure
            }
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        if !surface
            .current_state()
            .states
            .contains(xdg_toplevel::State::Fullscreen)
        {
            return;
        }

        let ret = surface.with_pending_state(|state| {
            state.states.unset(xdg_toplevel::State::Fullscreen);
            state.size = None;
            state.fullscreen_output.take()
        });

        if let Some(wl_output) = ret {
            let output = Output::from_resource(&wl_output).unwrap();
            if let Some(fullscreen) = output.user_data().get::<FullscreenSurface>() {
                if let Some((_, layer_surfaces)) = fullscreen.get() {
                    // restore layer-shell surfaces
                    let mut map = layer_map_for_output(&output);

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

        surface.send_pending_configure();
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        if !self.input_manager.is_mainmod_pressed() {
            return
        }

        let pointer = match Seat::from_resource(&seat) {
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

        let start_data = match pointer.grab_start_data() {
            Some(start_data) => start_data,
            None => {
                warn!("Failed to get start_data from: {:?}", pointer);
                return;
            }
        };

        self.grab_move_request(&PointerFocusTarget::WlSurface(surface.wl_surface().clone()), &pointer, start_data, serial);
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

        let start_data = match pointer.grab_start_data() {
            Some(start_data) => start_data,
            None => {
                warn!("Failed to get start_data from: {:?}", pointer);
                return;
            }
        };

        self.resize_move_request(&PointerFocusTarget::WlSurface(surface.wl_surface().clone()), &pointer, start_data, serial);
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
            .window_manager
            .get_mapped(&root.into())
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

        let window_rect = window.get_rect().unwrap();

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_rect.loc;
        
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }

    pub fn xdg_shell_handle_commit(&mut self, surface: &WlSurface) {
        // Handle toplevel commits.
        if let Some(window) = self.window_manager.get_mapped(&surface.clone().into())
        {
            window.on_commit();
        } else if let Some(unmapped) = self.window_manager.get_unmapped(&CustomWindowSurface::WlSurface(surface.clone())) {
            let (initial_configure_sent, configured) = with_states(surface, |states| {
                let guard = states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap();

                (guard.initial_configure_sent, guard.configured)
            });

            #[cfg(feature = "trace_protocol")]
            info!("initial_configure_sent: {}, configured: {}", initial_configure_sent, configured);

            // first configure sent
            if !initial_configure_sent {
                // get client first configuration
                let unmapped = unmapped.clone();
                self.window_manager.get_configure(&unmapped, &mut self.state);

                // will send initial configure - size
                let _success = self.map_window(unmapped.clone());
            } else if configured {
                let has_buffer = with_renderer_surface_state(surface, |state| {
                    state.buffer().is_some()
                });

                if let Some(has_buffer) = has_buffer {
                    // void first frame flash
                    if has_buffer {
                        // symbol the window is mapped
                        let unmapped = unmapped.clone();
                        self.set_mapped(&unmapped);
                    }
                }
            }
        }

        // Handle popup commits.
        let popups = &mut self.popups;
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