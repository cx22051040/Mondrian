#[cfg(feature = "xwayland")]
use smithay::xwayland::XWaylandClientData;
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor,
    reexports::{calloop::Interest, wayland_server::{protocol::wl_surface::WlSurface, Client, Resource}},
    wayland::{compositor::{
        add_blocker, add_pre_commit_hook, get_parent, is_sync_subsurface, with_states, BufferAssignment, CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes
    }, dmabuf::get_dmabuf, drm_syncobj::DrmSyncobjCachedState},
};

use crate::state::{ClientState, GlobalData};

impl CompositorHandler for GlobalData {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.state.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        #[cfg(feature = "xwayland")]
        if let Some(state) = client.get_data::<XWaylandClientData>() {
            return &state.compositor_state;
        }
        if let Some(state) = client.get_data::<ClientState>() {
            return &state.compositor_state;
        }
        panic!("Unknown client data type");
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        add_pre_commit_hook::<Self, _>(surface, move |data, _, surface| {
            let _span = tracy_client::span!("new_surface");

            let mut acquire_point = None;
            let maybe_dmabuf = with_states(surface, |surface_data| {
                acquire_point.clone_from(
                    &surface_data
                    .cached_state
                    .get::<DrmSyncobjCachedState>()
                    .pending()
                    .acquire_point,  
                );

                surface_data
                    .cached_state
                    .get::<SurfaceAttributes>()
                    .pending()
                    .buffer
                    .as_ref()
                    .and_then(|assignment| match assignment {
                        BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).cloned().ok(),
                        _ => None,
                    })
            });

            if let Some(dmabuf) = maybe_dmabuf {
                if let Some(acquire_point) = acquire_point {
                    if let Ok((blocker, source)) = acquire_point.generate_blocker() {
                        let client = surface.client().unwrap();
                        let res = data.loop_handle.insert_source(source, move |_, _, data| {
                            let _span = tracy_client::span!("acquire_point_blocker");
                            
                            let dh = data.display_handle.clone();
                            data.client_compositor_state(&client).blocker_cleared(data, &dh);
                            Ok(())
                        });
                        
                        if res.is_ok() {
                            add_blocker(surface, blocker);
                            return;
                        }
                    }
                }
            
                if let Ok((blocker, source)) = dmabuf.generate_blocker(Interest::READ) {
                    if let Some(client) = surface.client() {
                        let res = data.loop_handle.insert_source(source, move |_, _, data| {
                            let dh = data.display_handle.clone();
                            data.client_compositor_state(&client).blocker_cleared(data, &dh);
                            Ok(())
                        });

                        if res.is_ok() {
                            add_blocker(surface, blocker);
                            return;
                        }
                    }
                }
            }
        });
    }

    fn commit(&mut self, surface: &WlSurface) {
        let _span = tracy_client::span!("commit_all");

        on_commit_buffer_handler::<Self>(surface);
        self.backend.early_import(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }

            if self.layer_shell_handle_commit(&root) {
                return;
            }

            if let Some(window) = self.window_manager.get_window_wayland(&root) {
                window.on_commit();
            }

            self.xdg_shell_handle_commit(surface);
            // resize_grab::handle_commit(&mut self.workspace_manager, surface);
        };
    }
}
delegate_compositor!(GlobalData);
