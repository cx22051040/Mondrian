use std::sync::Arc;

use anyhow::Context;

#[cfg(feature = "xwayland")]
use smithay::{
    wayland::{
        xwayland_keyboard_grab::XWaylandKeyboardGrabState, 
        xwayland_shell
    }, 
    xwayland::X11Wm
};

use smithay::{
    backend::allocator::dmabuf::Dmabuf, delegate_data_device, delegate_dmabuf, delegate_drm_syncobj, delegate_output, delegate_shm, delegate_viewporter, desktop::PopupManager, reexports::{
        calloop::LoopHandle,
        wayland_server::{
            backend::ClientData, protocol::{wl_buffer, wl_shm}, DisplayHandle,
        },
    }, 
    utils::{Clock, Monotonic}, 
    wayland::{
        buffer::BufferHandler, compositor::{
            CompositorClientState, CompositorState
        }, dmabuf::{
            DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier
        }, drm_syncobj::{
            DrmSyncobjHandler, DrmSyncobjState
        }, foreign_toplevel_list::ForeignToplevelListState, output::{
            OutputHandler, OutputManagerState
        }, security_context::SecurityContext, selection::{
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler
            }, 
            primary_selection::PrimarySelectionState, SelectionHandler
        }, shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState}, shm::{ShmHandler, ShmState}, socket::ListeningSocketSource, viewporter::ViewporterState
    }
};

use crate::{
    backend::Backend, config::Configs, manager::{
        animation::AnimationManager, cursor::CursorManager, input::InputManager, output::OutputManager, render::RenderManager, window::WindowManager, workspace::{WorkspaceId, WorkspaceManager}
    }
};

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    pub _security_context: Option<SecurityContext>,
}

impl ClientData for ClientState {
    fn initialized(&self, client_id: smithay::reexports::wayland_server::backend::ClientId) {
        tracing::debug!("client initialized: {:?}", client_id);
    }
    fn disconnected(
        &self,
        client_id: smithay::reexports::wayland_server::backend::ClientId,
        reason: smithay::reexports::wayland_server::backend::DisconnectReason,
    ) {
        tracing::debug!(
            "client disconnected: {:?}, the reason: {:?}",
            client_id,
            reason
        );
    }
}

pub struct GlobalData {
    // config
    pub configs: Configs,

    pub socket_name: String,

    pub backend: Backend,
    pub state: State,

    // manager
    pub output_manager: OutputManager,
    pub workspace_manager: WorkspaceManager,
    pub window_manager: WindowManager,
    pub input_manager: InputManager,
    pub render_manager: RenderManager,
    pub animation_manager: AnimationManager,
    
    pub popups: PopupManager,
    pub cursor_manager: CursorManager,

    // handles
    pub loop_handle: LoopHandle<'static, GlobalData>,
    pub display_handle: DisplayHandle,

    // global data
    pub start_time: std::time::Instant,
    pub clock: Clock<Monotonic>,
}

impl GlobalData {
    pub fn new(
        loop_handle: LoopHandle<'static, GlobalData>,
        display_handle: DisplayHandle,
    ) -> anyhow::Result<Self> {
        // load configs
        let configs = Configs::new();

        // initial listening socket source
        let source = ListeningSocketSource::new_auto().context("Failed to init socket source")?;
        let socket_name = source.socket_name().to_string_lossy().into_owned();
        loop_handle
            .insert_source(source, move |client_stream, _, data| {
                data.display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .expect("Failed to insert client");
            })
            .context("Failed to init socket source")?;

        info!(name = socket_name, "Listening on wayland socket.");

        // init backend
        let mut backend = Backend::new(&loop_handle).context("Failed to create backend")?;

        // initial global state
        let mut nuonuo_state =
            State::new(&display_handle).context("Failed to create global state")?;

        // initial managers
        let mut output_manager = OutputManager::new();
        let mut workspace_manager = WorkspaceManager::new(configs.conf_workspaces.clone());
        let window_manager = WindowManager::new();
        let input_manager = InputManager::new(
                backend.seat_name(),
                &display_handle,
                configs.conf_keybindings.clone()
            )
            .context("Failed to create input_manager")?;
        let render_manager = RenderManager::new();
        let animation_manager = AnimationManager::new();
    
        let popups = PopupManager::default();
        let cursor_manager = CursorManager::new("default", 24);

        let start_time = std::time::Instant::now();
        let clock = Clock::new();

        // initial backend
        backend.init(
            &loop_handle,
            &display_handle,
            &mut output_manager,
            &render_manager,
            &mut nuonuo_state,
        );
        
        // set display env
        unsafe { std::env::set_var("WAYLAND_DISPLAY", &socket_name) };

        // TODO: test
        let output = output_manager.current_output();
        let output_geo = output_manager
            .output_geometry(output)
            .context("workspace add test error")?;

        workspace_manager.add_workspace(WorkspaceId::next(), output_geo, None, true);

        Ok(Self {
            backend,
            state: nuonuo_state,

            socket_name,

            output_manager,
            workspace_manager,
            window_manager,
            input_manager,
            render_manager,
            animation_manager,
            
            popups,
            cursor_manager,

            loop_handle,
            display_handle,

            configs,

            start_time,
            clock,
        })
    }

    pub fn refresh(&mut self) {
        // TODO: release death data
        self.animation_manager.refresh();
    }
}

pub struct State {
    // smithay state
    pub compositor_state: CompositorState,
    pub data_device_state: DataDeviceState,
    #[allow(dead_code)]
    pub output_manager_state: OutputManagerState,
    pub shm_state: ShmState,
    pub dmabuf_state: DmabufState,
    pub syncobj_state: Option<DrmSyncobjState>,
    pub primary_selection_state: PrimarySelectionState,

    // xwayland state
    #[cfg(feature = "xwayland")]
    pub xwayland_shell_state: xwayland_shell::XWaylandShellState,
    #[cfg(feature = "xwayland")]
    pub xwm: Option<X11Wm>,
    #[cfg(feature = "xwayland")]
    pub xdisplay: Option<u32>,

    // protocol state
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: WlrLayerShellState,
    #[allow(dead_code)]
    pub viewporter_state: ViewporterState,
    pub foreign_toplevel_state: ForeignToplevelListState,
}

impl State {
    pub fn new(display_handle: &DisplayHandle) -> anyhow::Result<Self> {
        // init smithay state
        let compositor_state = CompositorState::new::<GlobalData>(display_handle);
        let data_device_state = DataDeviceState::new::<GlobalData>(display_handle);
        let output_manager_state =
            OutputManagerState::new_with_xdg_output::<GlobalData>(display_handle);
        let shm_state = ShmState::new::<GlobalData>(
            display_handle,
            vec![wl_shm::Format::Argb8888, wl_shm::Format::Xrgb8888],
        );
        let dmabuf_state = DmabufState::new();
        let primary_selection_state = PrimarySelectionState::new::<GlobalData>(display_handle);
        
        // init xwayland state
        #[cfg(feature = "xwayland")]
        let xwayland_shell_state = xwayland_shell::XWaylandShellState::new::<GlobalData>(display_handle);
        #[cfg(feature = "xwayland")]
        XWaylandKeyboardGrabState::new::<GlobalData>(display_handle);

        // init protocol state
        let xdg_shell_state = XdgShellState::new::<GlobalData>(display_handle);
        let layer_shell_state = WlrLayerShellState::new::<GlobalData>(display_handle);
        let viewporter_state = ViewporterState::new::<GlobalData>(display_handle);
        let foreign_toplevel_state = ForeignToplevelListState::new::<GlobalData>(display_handle);

        Ok(State {
            compositor_state,
            data_device_state,
            output_manager_state,
            shm_state,
            dmabuf_state,
            syncobj_state: None,
            primary_selection_state,

            xwayland_shell_state,
            xwm: None,
            xdisplay: None,

            xdg_shell_state,
            layer_shell_state,
            viewporter_state,
            foreign_toplevel_state,
        })
    }
}

impl SelectionHandler for GlobalData {
    type SelectionUserData = ();
}

impl ClientDndGrabHandler for GlobalData {}
impl ServerDndGrabHandler for GlobalData {}

impl DataDeviceHandler for GlobalData {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.state.data_device_state
    }
}
delegate_data_device!(GlobalData);

impl OutputHandler for GlobalData {}
delegate_output!(GlobalData);

impl BufferHandler for GlobalData {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for GlobalData {
    fn shm_state(&self) -> &ShmState {
        &self.state.shm_state
    }
}
delegate_shm!(GlobalData);

impl DrmSyncobjHandler for GlobalData {
    fn drm_syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        self.state.syncobj_state.as_mut()
    }
}
delegate_drm_syncobj!(GlobalData);

impl DmabufHandler for GlobalData {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.state.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self.backend.dmabuf_imported(&dmabuf) {
            let _ = notifier.successful::<GlobalData>();
        } else {
            notifier.failed();
        }
    }
}
delegate_dmabuf!(GlobalData);

delegate_viewporter!(GlobalData);