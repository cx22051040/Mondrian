use std::sync::Arc;

use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf, renderer::ImportDma
    }, delegate_data_device, delegate_dmabuf, delegate_output, delegate_seat, delegate_shm, desktop::{
        find_popup_root_surface, get_popup_toplevel_coords, PopupKind, PopupManager
    }, input::{Seat, SeatHandler, SeatState}, output::Mode as OutputMode, reexports::{
        calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction}, wayland_server::{
            backend::ClientData, protocol::{wl_buffer, wl_surface::WlSurface}, Display, DisplayHandle, Resource
        }
    }, utils::Transform, wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorState
        },
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
        output::OutputHandler,
        security_context::SecurityContext,
        selection::{
            data_device::{
                set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler
            }, SelectionHandler
        },
        shell::xdg::{PopupSurface, XdgShellState},
        shm::{ShmHandler, ShmState},
        socket::ListeningSocketSource,
    }
};

use crate::{
    backend::{self, winit::WinitData}, config::Configs, render::cursor::{CursorManager, CursorTextureCache}, space::{output::OutputManager, workspace::WorkspaceManager}
};

#[derive(Debug, Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    pub _security_context: Option<SecurityContext>,
}

impl ClientData for ClientState {
    fn initialized(&self, client_id: smithay::reexports::wayland_server::backend::ClientId) {
        tracing::info!("client initialized: {:?}", client_id);
    }
    fn disconnected(
        &self,
        client_id: smithay::reexports::wayland_server::backend::ClientId,
        reason: smithay::reexports::wayland_server::backend::DisconnectReason,
    ) {
        tracing::info!(
            "client disconnected: {:?}, the reason: {:?}",
            client_id,
            reason
        );
    }
}

pub struct NuonuoState {
    pub start_time: std::time::Instant,

    pub backend_data: WinitData,
    pub socket_name: String,
    pub loop_handle: LoopHandle<'static, NuonuoState>,
    pub display_handle: DisplayHandle,

    // desktop
    pub workspace_manager: WorkspaceManager,
    pub output_manager: OutputManager,

    // smithay state
    pub compositor_state: CompositorState,
    pub data_device_state: DataDeviceState,
    pub seat_state: SeatState<NuonuoState>,
    pub shm_state: ShmState,
    pub popups: PopupManager,
    pub xdg_shell_state: XdgShellState,
    pub seat: Seat<Self>,

    pub cursor_manager: CursorManager,
    pub cursor_texture_cache: CursorTextureCache,

    // configs
    pub configs: Configs,
}

impl NuonuoState {
    pub fn new(
        loop_handle: LoopHandle<'static, NuonuoState>,
    ) -> Self {
        
        let start_time = std::time::Instant::now();

        let configs = Configs::new("src/config/keybindings.conf");

        let display_handle: DisplayHandle = Self::init_display_handle(&loop_handle);

        // init wayland clients
        let socket_name = Self::init_wayland_listener(&loop_handle);

        let mut workspace_manager = WorkspaceManager::new();
        let mut output_manager = OutputManager::new(&display_handle);

        // init smithay state
        let compositor_state = CompositorState::new::<Self>(&display_handle);
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);
        let shm_state = ShmState::new::<Self>(&display_handle, vec![]);
        let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
        let popups = PopupManager::default();
        let mut seat_state = SeatState::new();
        let seat_name = String::from("winit");
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&display_handle, seat_name);
        
        // TODO: use config file
        let cursor_manager = CursorManager::new("default", 24);
        let cursor_texture_cache = Default::default();

        #[cfg(feature = "winit")]
        let backend_data = backend::winit::init_winit(&loop_handle, &display_handle);

        let size = backend_data.backend.window_size();
        let mode = OutputMode {
            size,
            refresh: 60_000,
        };

        // TODO: manage more output, get the output physical name as output name
        // Now provided we only have one output
        output_manager.add_output("winit", &display_handle, true);
        output_manager.change_current_state(
            Some(mode), 
            Some(Transform::Flipped180),
            None,
            Some((0, 0).into()),
        );
        output_manager.set_preferred(mode);

        workspace_manager.add_workspace(output_manager.current_output(), (0, 0), None, true);
        // TODO: delete this test: add space-2 for test workspace switch
        workspace_manager.add_workspace(output_manager.current_output(), (0, 0), None, false);

        // space_manager.current_space().map_output(&backend_data.output, (0, 0));

        // Notify clients that we have a keyboard, for the sake of the example we assume that keyboard is always present.
        // You may want to track keyboard hot-plug in real compositor.
        seat.add_keyboard(Default::default(), 200, 25).unwrap();

        // Notify clients that we have a pointer (mouse)
        // Here we assume that there is always pointer plugged in
        seat.add_pointer();

        NuonuoState {
            start_time,
            backend_data,
            socket_name,
            loop_handle,
            display_handle,

            workspace_manager,
            output_manager,
            popups,

            compositor_state,
            data_device_state,
            seat_state,
            shm_state,
            xdg_shell_state,
            seat,

            cursor_manager,
            cursor_texture_cache,

            configs,
        }
    }

    fn init_display_handle(loop_handle: &LoopHandle<'static, NuonuoState>) -> DisplayHandle{
        let display: Display<NuonuoState> = Display::new().unwrap();
        let display_handle = display.handle();
    
        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, nuonuo_state| {
                    // Safety: we don't drop the display
                    unsafe {
                        display
                            .get_mut()
                            .dispatch_clients(nuonuo_state)
                            .unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .expect("Failed to init wayland server source");

        display_handle
    }

    fn init_wayland_listener(loop_handle: &LoopHandle<'static, NuonuoState>) -> String {
        let source = ListeningSocketSource::new_auto().unwrap();
        let socket_name = source.socket_name().to_string_lossy().into_owned();

        loop_handle
            .insert_source(source, move |client_stream, _, nuonuo_state| {
                nuonuo_state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("Failed to init wayland socket source.");

        tracing::info!(name = socket_name, "Listening on wayland socket.");

        socket_name
    }

    pub fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .workspace_manager
            .current_workspace()
            .space
            .elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == &root)
        else {
            return;
        };

        let output = self.output_manager.current_output();
        let output_geo = self.workspace_manager.current_workspace().space.output_geometry(output).unwrap();
        let window_geo = self.workspace_manager.current_workspace().space.element_geometry(window).unwrap();

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }

    // TODO: add device event
    // fn on_device_added(&mut self, device: impl Device) {
    //     if device.has_capability(DeviceCapability::TabletTool) {
    //         let tablet_seat = self.seat.tablet_seat();
    //         let desc = TabletDescriptor::from(&device);
    //         tablet_seat.add_tablet::<Self>(&self.display_handle, &desc);
    //     }
    //     if device.has_capability(DeviceCapability::Touch) && self.niri.seat.get_touch().is_none() {
    //         self.seat.add_touch();
    //     }
    // }

    // fn on_device_removed(&mut self, device: impl Device) {
    //     if device.has_capability(DeviceCapability::TabletTool) {
    //         let tablet_seat = self.seat.tablet_seat();

    //         let desc = TabletDescriptor::from(&device);
    //         tablet_seat.remove_tablet(&desc);

    //         // If there are no tablets in seat we can remove all tools
    //         if tablet_seat.count_tablets() == 0 {
    //             tablet_seat.clear_tools();
    //         }
    //     }
    //     if device.has_capability(DeviceCapability::Touch) && self.touch.is_empty() {
    //         self.niri.seat.remove_touch();
    //     }
    // }

}

impl SelectionHandler for NuonuoState {
    type SelectionUserData = ();
}

impl ClientDndGrabHandler for NuonuoState {}
impl ServerDndGrabHandler for NuonuoState {}

impl DataDeviceHandler for NuonuoState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
delegate_data_device!(NuonuoState);

impl OutputHandler for NuonuoState {}
delegate_output!(NuonuoState);

impl SeatHandler for NuonuoState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<NuonuoState> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        image: smithay::input::pointer::CursorImageStatus,
    ) {
        self.cursor_manager.set_cursor_image(image);
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let display_handle = &self.display_handle;
        let client = focused.and_then(|s| display_handle.get_client(s.id()).ok());
        set_data_device_focus(display_handle, seat, client);
    }
}
delegate_seat!(NuonuoState);

impl BufferHandler for NuonuoState {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for NuonuoState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}
delegate_shm!(NuonuoState);

impl DmabufHandler for NuonuoState {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.backend_data.dmabuf_state.0
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self
            .backend_data
            .backend
            .renderer()
            .import_dmabuf(&dmabuf, None)
            .is_ok()
        {
            let _ = notifier.successful::<NuonuoState>();
        } else {
            notifier.failed();
        }
    }
}
delegate_dmabuf!(NuonuoState);
