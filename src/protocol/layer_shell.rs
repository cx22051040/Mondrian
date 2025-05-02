use smithay::{
    delegate_layer_shell, 
    desktop::{
        layer_map_for_output, LayerSurface, WindowSurfaceType
    }, 
    output::Output, 
    reexports::wayland_server::protocol::wl_surface::WlSurface, 
    wayland::{
        compositor::with_states, 
        shell::wlr_layer::{
            LayerSurfaceData, WlrLayerShellHandler, WlrLayerShellState
        }
    }
};

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

        let mut map = layer_map_for_output(&output);
        map.map_layer(&LayerSurface::new(surface, namespace)).unwrap();
    }
    
    fn layer_destroyed(&mut self, surface: smithay::wayland::shell::wlr_layer::LayerSurface) {
        // TODO: outputs
        let map = layer_map_for_output(self.output_manager.current_output());
        let layer = map
            .layers()
            .find(|&layer| layer.layer_surface() == &surface )
            .cloned();
        let (mut map, layer) = layer.map(|layer| (map, layer)).unwrap();
        map.unmap_layer(&layer);
    }

    fn new_popup(&mut self, _parent: smithay::wayland::shell::wlr_layer::LayerSurface, popup: smithay::wayland::shell::xdg::PopupSurface) {
        self.unconstrain_popup(&popup);
    }
}
delegate_layer_shell!(NuonuoState);

impl NuonuoState {
    pub fn layer_shell_handle_commit(&mut self, surface: &WlSurface) -> bool {

        let output = self
            .output_manager
            .current_output();

        let mut map = layer_map_for_output(output);
        
        if map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).is_some() {
        
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<LayerSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });
    
            map.arrange();
            if !initial_configure_sent {
                let layer = map
                    .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                    .unwrap();
        
                layer.layer_surface().send_configure();
            }

            return true;
        }

        false

        

    }
}