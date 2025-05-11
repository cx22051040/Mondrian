use anyhow::Context;
use smithay::{
    backend::{allocator::dmabuf::Dmabuf, renderer::ImportDma}, 
    delegate_data_device, delegate_dmabuf, delegate_output, delegate_seat, delegate_shm, delegate_viewporter, 
    desktop::PopupManager, 
    input::{Seat, SeatHandler, SeatState}, 
    reexports::{
        calloop::LoopHandle,
        wayland_server::{
            backend::ClientData, protocol::{wl_buffer, wl_shm, wl_surface::WlSurface}, DisplayHandle, Resource
        },
    }, utils::{Clock, Monotonic}, wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorState},
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
        output::OutputHandler,
        security_context::SecurityContext,
        selection::{
            data_device::{
                set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler
            }, SelectionHandler
        },
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::{ShmHandler, ShmState},
        viewporter::ViewporterState,
    }
};

#[cfg(feature = "tty")]
use crate::backend::tty::Tty;

use crate::{
    backend::{
        winit::Winit, Backend
    }, 
    config::Configs, 
    manager::{
        input::InputManager, 
        output::OutputManager, 
        render::RenderManager, 
        window::WindowManager, 
        workspace::WorkspaceManager
    }, 
    render::cursor::CursorManager
};

#[derive(Default)]
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

pub struct GlobalData{
    pub backend: Backend,
    pub state: State,

    // manager
    pub output_manager: OutputManager,
    pub workspace_manager: WorkspaceManager,
    pub window_manager: WindowManager,
    pub cursor_manager: CursorManager,
    pub input_manager: InputManager,
    pub popups: PopupManager,
    pub render_manager: RenderManager,

    // handles
    pub loop_handle: LoopHandle<'static, GlobalData>,
    pub display_handle: DisplayHandle,

    // config
    pub configs: Configs,

    // global data
    pub start_time: std::time::Instant,
    pub clock: Clock<Monotonic>,

}

impl GlobalData {
    pub fn new(loop_handle: LoopHandle<'static, GlobalData>, display_handle: DisplayHandle) -> Self {

        // judge the backend type, create base config
        let has_display = std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var_os("WAYLAND_SOCKET").is_some()
            || std::env::var_os("DISPLAY").is_some();
    
        let mut backend = if has_display {
            let winit = Winit::new(&loop_handle, &display_handle).unwrap();
            Backend::Winit(winit)
        } else {
            let tty = Tty::new(&loop_handle).context("error get tty backend").unwrap();
            Backend::Tty(tty)
        };

        // initial global state
        let nuonuo_state = State::new(&display_handle).expect("cannot make global state");

        // initial managers
        let mut output_manager = OutputManager::new(display_handle.clone());
        let mut workspace_manager = WorkspaceManager::new();
        let window_manager = WindowManager::new();
        let cursor_manager = CursorManager::new("default", 24);
        let input_manager = InputManager::new(backend.seat_name(), &display_handle);
        let popups = PopupManager::default();
        let render_manager = RenderManager::new();

        // initial backend
        backend.init(&mut output_manager, &loop_handle);

        // TODO: tidy
        workspace_manager.add_workspace(output_manager.current_output(), (0, 0), None, true);
        workspace_manager.add_workspace(output_manager.current_output(), (0, 0), None, false);
        
        // load configs
        let configs = Configs::new("src/config/keybindings.conf");

        let start_time = std::time::Instant::now();
        let clock = Clock::new();

        Self {
            backend,
            state: nuonuo_state,

            output_manager,
            workspace_manager,
            window_manager,
            cursor_manager,
            input_manager,
            popups,
            render_manager,

            loop_handle,
            display_handle,

            configs,

            start_time,
            clock,
        }
    }
}

pub struct State {
    // smithay state
    pub compositor_state: CompositorState,
    pub data_device_state: DataDeviceState,
    pub shm_state: ShmState,
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: WlrLayerShellState,
    pub viewporter_state: ViewporterState,
}

impl State {
    pub fn new(display_handle: &DisplayHandle) -> anyhow::Result<Self> {
        // init smithay state
        let compositor_state = CompositorState::new::<GlobalData>(display_handle);
        let data_device_state = DataDeviceState::new::<GlobalData>(display_handle);
        let shm_state = ShmState::new::<GlobalData>(display_handle, vec![    
            wl_shm::Format::Argb8888,
            wl_shm::Format::Xrgb8888,
        ]);
        let xdg_shell_state = XdgShellState::new::<GlobalData>(display_handle);
        let layer_shell_state = WlrLayerShellState::new::<GlobalData>(display_handle);
        let viewporter_state = ViewporterState::new::<GlobalData>(display_handle);

        Ok(State {
            compositor_state,
            data_device_state,
            shm_state,
            xdg_shell_state,
            layer_shell_state,
            viewporter_state,
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

impl SeatHandler for GlobalData {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<GlobalData> {
        &mut self.input_manager.seat_state
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
delegate_seat!(GlobalData);

impl BufferHandler for GlobalData {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for GlobalData {
    fn shm_state(&self) -> &ShmState {
        &self.state.shm_state
    }
}
delegate_shm!(GlobalData);

impl DmabufHandler for GlobalData {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.backend.winit().dmabuf_state.0
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self
            .backend
            .winit()
            .backend
            .renderer()
            .import_dmabuf(&dmabuf, None)
            .is_ok()
        {
            let _ = notifier.successful::<GlobalData>();
        } else {
            notifier.failed();
        }
    }
}
delegate_dmabuf!(GlobalData);

delegate_viewporter!(GlobalData);