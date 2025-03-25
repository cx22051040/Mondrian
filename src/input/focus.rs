use smithay::{desktop::{LayerSurface, PopupKind, Window}, reexports::wayland_server::protocol::wl_surface::WlSurface, utils::IsAlive};

#[derive(Debug)]
pub enum KeyboardFocusTarget {
  Window(Window),
  LayerSurface(LayerSurface),
  Popup(PopupKind),
}

impl IsAlive for KeyboardFocusTarget {
  #[inline]
  fn alive(&self) -> bool {
      match self {
          KeyboardFocusTarget::Window(w) => w.alive(),
          KeyboardFocusTarget::LayerSurface(l) => l.alive(),
          KeyboardFocusTarget::Popup(p) => p.alive(),
      }
  }
}

pub enum PointerFocusTarget {
  WlSurface(WlSurface),
}