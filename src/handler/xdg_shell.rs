use smithay::{delegate_xdg_shell, desktop::{PopupKind, PopupManager, Space, Window}, input::{pointer::Focus, Seat}, reexports::{wayland_protocols::xdg::shell::server::xdg_toplevel, wayland_server::{protocol::{wl_seat, wl_surface::WlSurface}, Resource}}, utils::Serial, wayland::{compositor::with_states,
    shell::xdg::{PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState, XdgToplevelSurfaceData}}};
use smithay::input::pointer::GrabStartData as PointerGrabStartData;
use crate::{input::move_grab::MoveSurfaceGrab, state::NuonuoState};

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

impl XdgShellHandler for NuonuoState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }
  
    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        self.space.map_element(window, (0, 0), false);
    }
  
    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }
  
    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }
  
    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat: Seat<NuonuoState> = Seat::from_resource(&seat).unwrap();
	    let wl_surface = surface.wl_surface();

		if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
			let pointer = seat.get_pointer().unwrap();
			// TODO: Maybe can improve this find action
			let window = self.space.elements()
				.find(|w| w.toplevel().unwrap().wl_surface() == wl_surface)
				.unwrap()
				.clone();
			
			let initial_window_location = self.space.element_location(&window).unwrap();
			let grab = MoveSurfaceGrab {
				start_data,
				window,
				initial_window_location,
			};
		
			pointer.set_grab(self, grab, serial, Focus::Clear);
		}
    }
  
    fn resize_request(
        &mut self,
        _surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
        _edges: xdg_toplevel::ResizeEdge,
    ) {
      // TODO
    }
  
    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // TODO popup grabs
    }
}
delegate_xdg_shell!(NuonuoState);

fn check_grab(
	seat: &Seat<NuonuoState>,
	surface: &WlSurface,
	serial: Serial,
) -> Option<PointerGrabStartData<NuonuoState>> {
	let pointer = seat.get_pointer()?;

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