

use std::sync::Arc;

use smithay::{backend::{allocator::dmabuf::Dmabuf, renderer::{utils::on_commit_buffer_handler, ImportDma}}, delegate_compositor, delegate_data_device, delegate_dmabuf, delegate_output, delegate_seat, delegate_shm, 
  desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKind, PopupManager, Space, Window}, 
  input::{Seat, SeatHandler, SeatState}, 
  reexports::{calloop::LoopHandle,
  wayland_server::{backend::ClientData, 
  protocol::{wl_buffer, wl_surface::WlSurface}, Client, DisplayHandle, Resource}}, 
  wayland::{buffer::BufferHandler, 
  compositor::{get_parent, is_sync_subsurface, CompositorClientState, CompositorHandler, CompositorState}, 
  dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier}, output::{OutputHandler, OutputManagerState}, security_context::SecurityContext, 
  selection::{data_device::{set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler}, SelectionHandler}, 
  shell::xdg::{PopupSurface, XdgShellState}, shm::{ShmHandler, ShmState}, socket::ListeningSocketSource
}};

use crate::{backend::winit::WinitData, config::Configs, CalloopData};
use crate::handler::xdg_shell::handle_commit;

#[derive(Debug, Default)]
pub struct ClientState {
  pub compositor_state: CompositorClientState,
  pub security_context: Option<SecurityContext>,
}
impl ClientData for ClientState {
  fn initialized(&self, client_id: smithay::reexports::wayland_server::backend::ClientId) {
    tracing::info!("client initialized: {:?}", client_id);
  }
  fn disconnected(&self, client_id: smithay::reexports::wayland_server::backend::ClientId, reason: smithay::reexports::wayland_server::backend::DisconnectReason) {
    tracing::info!("client disconnected: {:?}, the reason: {:?}", client_id, reason); 
  }
}

#[derive(Debug)]
pub struct NuonuoState {
  pub start_time: std::time::Instant,

  pub backend_data: WinitData,
  pub socket_name: String,
  pub display_handle: DisplayHandle,

  // desktop
  pub space: Space<Window>,
  
  // smithay state
  pub compositor_state: CompositorState,
  pub data_device_state: DataDeviceState,
  pub output_manager_state: OutputManagerState,
  pub seat_state: SeatState<NuonuoState>,
  pub shm_state: ShmState,
  pub popups: PopupManager,
  pub xdg_shell_state: XdgShellState,
  pub seat: Seat<Self>,
}

impl NuonuoState {
  pub fn new (
    display_handle: DisplayHandle,
    loop_handle: &LoopHandle<'static, CalloopData>,
    backend_data: WinitData,
    configs: &Configs,
  ) -> Self {
    let start_time = std::time::Instant::now();

    // init wayland clients
    let socket_name = Self::init_wayland_listener(loop_handle);

    let compositor_state = CompositorState::new::<Self>(&display_handle);

    let data_device_state = DataDeviceState::new::<Self>(&display_handle);

    let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display_handle);

    let shm_state = ShmState::new::<Self>(&display_handle, vec![]);

    let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
    let popups = PopupManager::default();
    
    let mut seat_state = SeatState::new();
    let seat_name = String::from("winit");
    let mut seat: Seat<Self> = seat_state.new_wl_seat(&display_handle, seat_name);

    let mut space = Space::default();

    space.map_output(&backend_data.output, (0, 0));

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
      display_handle,

      space,
      popups,

      compositor_state,
      data_device_state,
      output_manager_state,
      seat_state,
      shm_state,
      xdg_shell_state,
      seat,
    }

  }

  fn init_wayland_listener (loop_handle: &LoopHandle<'static, CalloopData>,) -> String {
    let source = ListeningSocketSource::new_auto().unwrap();
    let socket_name = source.socket_name().to_string_lossy().into_owned();

    loop_handle
      .insert_source(source, move |client_stream, _, calloop_data| {
        calloop_data.state
          .display_handle
          .insert_client(client_stream, Arc::new(ClientState::default()))
          .unwrap();
      }).expect("Failed to init wayland socket source.");

    tracing::info!(name = socket_name, "Listening on wayland socket.");

    socket_name
  }

  pub fn unconstrain_popup(&self, popup: &PopupSurface) {
    let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
        return;
    };
    let Some(window) = self
        .space
        .elements()
        .find(|w| w.toplevel().unwrap().wl_surface() == &root)
    else {
        return;
    };

    let output = self.space.outputs().next().unwrap();
    let output_geo = self.space.output_geometry(output).unwrap();
    let window_geo = self.space.element_geometry(window).unwrap();

    // The target geometry for the positioner should be relative to its parent's geometry, so
    // we will compute that here.
    let mut target = output_geo;
    target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
    target.loc -= window_geo.loc;

    popup.with_pending_state(|state| {
        state.geometry = state.positioner.get_unconstrained_geometry(target);
    });
  }

}


impl CompositorHandler for NuonuoState {
  fn compositor_state(&mut self) -> &mut CompositorState {
    &mut self.compositor_state
  }

  fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
    &client.get_data::<ClientState>().unwrap().compositor_state
  }

  fn commit(&mut self, surface: &WlSurface) {
    on_commit_buffer_handler::<Self>(surface);
    if !is_sync_subsurface(surface) {
      let mut root = surface.clone();
      while let Some(parent) = get_parent(&root) {
        root = parent;
      }
      if let Some(window) = self
        .space
        .elements()
        .find(|w| w.toplevel().unwrap().wl_surface() == &root)
      {
        window.on_commit();
      }
      handle_commit(&mut self.popups, &self.space, surface);
    };
  }
}
delegate_compositor!(NuonuoState);

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

  fn cursor_image(&mut self, _seat: &Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {}

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

  fn dmabuf_imported(&mut self, _global: &DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier) {
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


