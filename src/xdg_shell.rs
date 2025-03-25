use smithay::{desktop::{PopupKind, PopupManager, Space, Window}, reexports::wayland_server::protocol::wl_surface::WlSurface, wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData}};

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
