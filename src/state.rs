use std::sync::Arc;

use anyhow::Context;
use smithay::{
    backend::allocator::dmabuf::Dmabuf,
    delegate_data_device, delegate_dmabuf, delegate_output, delegate_seat, delegate_shm,
    delegate_viewporter,
    desktop::PopupManager,
    input::{Seat, SeatHandler, SeatState},
    reexports::{
        calloop::LoopHandle,
        wayland_server::{
            DisplayHandle, Resource,
            backend::ClientData,
            protocol::{wl_buffer, wl_shm, wl_surface::WlSurface},
        },
    },
    utils::{Clock, Monotonic},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorState},
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
        foreign_toplevel_list::ForeignToplevelListState,
        output::OutputHandler,
        security_context::SecurityContext,
        selection::{
            SelectionHandler,
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
                set_data_device_focus,
            },
        },
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::{ShmHandler, ShmState},
        viewporter::ViewporterState,
    },
};

use crate::{
    backend::Backend,
    config::Configs,
    layout::tiled_tree::TiledScheme,
    manager::{
        cursor::CursorManager, input::InputManager, output::OutputManager, render::RenderManager,
        window::WindowManager, workspace::WorkspaceManager,
    },
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
    pub configs: Arc<Configs>,

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
        let configs = Arc::new(Configs::new());

        // init backend
        let mut backend = Backend::new(&loop_handle).context("Failed to create backend")?;

        // initial global state
        let mut nuonuo_state =
            State::new(&display_handle).context("Failed to create global state")?;

        // initial managers
        let mut output_manager = OutputManager::new(&display_handle, configs.clone());
        let mut workspace_manager = WorkspaceManager::new(configs.conf_workspaces.clone());
        let window_manager = WindowManager::new();
        let cursor_manager = CursorManager::new("default", 24);
        let input_manager = InputManager::new(
            backend.seat_name(),
            &display_handle,
            "src/config/keybindings.conf",
        )
        .context("Failed to create input_manager")?;
        let popups = PopupManager::default();
        let render_manager = RenderManager::new();

        // initial backend
        backend.init(
            &loop_handle,
            &display_handle,
            &mut output_manager,
            &render_manager,
            &mut nuonuo_state,
        );

        // TODO: just easy for test workspace exchange
        let output = output_manager.current_output();
        let output_geo = output_manager
            .output_geometry(output)
            .context("workspace add test error")?;

        workspace_manager.add_workspace(output, output_geo, Some(TiledScheme::Default), true);
        workspace_manager.add_workspace(output, output_geo, Some(TiledScheme::Spiral), false);

        let start_time = std::time::Instant::now();
        let clock = Clock::new();

        Ok(Self {
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
        })
    }
}

pub struct State {
    // smithay state
    pub compositor_state: CompositorState,
    pub data_device_state: DataDeviceState,
    pub shm_state: ShmState,
    pub dmabuf_state: DmabufState,

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
        let shm_state = ShmState::new::<GlobalData>(
            display_handle,
            vec![wl_shm::Format::Argb8888, wl_shm::Format::Xrgb8888],
        );
        let dmabuf_state = DmabufState::new();

        let xdg_shell_state = XdgShellState::new::<GlobalData>(display_handle);
        let layer_shell_state = WlrLayerShellState::new::<GlobalData>(display_handle);
        let viewporter_state = ViewporterState::new::<GlobalData>(display_handle);
        let foreign_toplevel_state = ForeignToplevelListState::new::<GlobalData>(display_handle);

        Ok(State {
            compositor_state,
            data_device_state,
            shm_state,
            dmabuf_state,

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

    fn led_state_changed(
        &mut self,
        _seat: &Seat<Self>,
        _led_state: smithay::input::keyboard::LedState,
    ) {
        info!("led state changed");
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
