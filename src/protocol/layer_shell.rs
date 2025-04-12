use smithay::{delegate_layer_shell, desktop::{layer_map_for_output, LayerSurface, WindowSurfaceType}, output::Output, reexports::wayland_server::protocol::wl_surface::WlSurface, wayland::{compositor::{get_parent, send_surface_state, with_states}, fractional_scale::with_fractional_scale, shell::wlr_layer::{LayerSurfaceData, WlrLayerShellHandler, WlrLayerShellState}}};

use crate::state::NuonuoState;

impl WlrLayerShellHandler for NuonuoState{
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
            &mut self,
            surface: smithay::wayland::shell::wlr_layer::LayerSurface,
            wl_output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
            _layer: smithay::wayland::shell::wlr_layer::Layer,
            namespace: String,
        ) {
        let output = if let Some(wl_output) = &wl_output {
            Output::from_resource(wl_output)
        } else {
            // TODO: output_manager -> Option<Output>
            Some(self.output_manager.current_output().clone())
        };

        let Some(output) = output else {
            warn!("no output for new layer surface, closing");
            surface.send_close();
            return;
        };

        let is_new = self.unmapped_layer_surfaces.insert(surface.wl_surface().clone());
        assert!(is_new);

        let mut map = layer_map_for_output(&output);
        map.map_layer(&LayerSurface::new(surface, namespace)).unwrap();
    }
    
    fn layer_destroyed(&mut self, surface: smithay::wayland::shell::wlr_layer::LayerSurface) {

    }

    fn new_popup(&mut self, parent: smithay::wayland::shell::wlr_layer::LayerSurface, popup: smithay::wayland::shell::xdg::PopupSurface) {
        todo!()
    }
}
delegate_layer_shell!(NuonuoState);

impl NuonuoState {
    pub fn layer_shell_handle_commit(&mut self, surface: &WlSurface) -> bool {
        let mut root_surface = surface.clone();
        while let Some(parent) = get_parent(&root_surface) {
            root_surface = parent;
        }

        let output = self
            .output_manager
            .current_output();

        let map = layer_map_for_output(output);
        if map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).is_none() {
            return false
        };

        if surface == &root_surface {
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<LayerSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });

            let mut map = layer_map_for_output(output);
            map.arrange();

            let layer = map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).unwrap();

            if initial_configure_sent {
                self.unmapped_layer_surfaces.remove(surface);
                self.mapped_layer_surfaces.insert(surface.clone());
            } else {
                let scale = output.current_scale();
                let transform = output.current_transform();
                with_states(surface, |data| {
                    send_surface_state(surface, data, scale.integer_scale(), transform);
                    with_fractional_scale(data, |fractional| {
                        fractional.set_preferred_scale(scale.fractional_scale());
                    });
                });

                layer.layer_surface().send_configure();
            }
            drop(map);
        }

        true

    }
}