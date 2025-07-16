use std::borrow::Cow;

#[cfg(feature = "xwayland")]
use smithay::xwayland::X11Surface;

use smithay::{
    desktop::{
        LayerSurface, PopupKind, Window, WindowSurface
    }, input::{keyboard::KeyboardTarget, pointer::PointerTarget, touch::TouchTarget}, reexports::wayland_server::{backend::ObjectId, protocol::wl_surface::WlSurface}, utils::{IsAlive, Serial}, wayland::seat::WaylandFocus
};

use crate::state::GlobalData;

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
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

impl KeyboardTarget<GlobalData> for KeyboardFocusTarget {
    fn enter(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        keys: Vec<smithay::input::keyboard::KeysymHandle<'_>>, 
        serial: Serial
    ) {
        match self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => KeyboardTarget::enter(w.wl_surface(), seat, data, keys, serial),
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(s) => KeyboardTarget::enter(s, seat, data, keys, serial),
            },
            KeyboardFocusTarget::LayerSurface(l) => {
                KeyboardTarget::enter(l.wl_surface(), seat, data, keys, serial)
            }
            KeyboardFocusTarget::Popup(p) => KeyboardTarget::enter(p.wl_surface(), seat, data, keys, serial),
        }   
    }

    fn leave(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        serial: Serial
    ) {
        match self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => KeyboardTarget::leave(w.wl_surface(), seat, data, serial),
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(s) => KeyboardTarget::leave(s, seat, data, serial),
            },
            KeyboardFocusTarget::LayerSurface(l) => KeyboardTarget::leave(l.wl_surface(), seat, data, serial),
            KeyboardFocusTarget::Popup(p) => KeyboardTarget::leave(p.wl_surface(), seat, data, serial),
        }
    }
    
    fn key(
        &self,
        seat: &smithay::input::Seat<GlobalData>,
        data: &mut GlobalData,
        key: smithay::input::keyboard::KeysymHandle<'_>,
        state: smithay::backend::input::KeyState,
        serial: Serial,
        time: u32,
    ) {
        match self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => {
                    KeyboardTarget::key(w.wl_surface(), seat, data, key, state, serial, time)
                }
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(s) => KeyboardTarget::key(s, seat, data, key, state, serial, time),
            },
            KeyboardFocusTarget::LayerSurface(l) => {
                KeyboardTarget::key(l.wl_surface(), seat, data, key, state, serial, time)
            }
            KeyboardFocusTarget::Popup(p) => {
                KeyboardTarget::key(p.wl_surface(), seat, data, key, state, serial, time)
            }
        }
    }

    fn modifiers(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        modifiers: smithay::input::keyboard::ModifiersState, 
        serial: Serial
    ) {
        match self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => {
                    KeyboardTarget::modifiers(w.wl_surface(), seat, data, modifiers, serial)
                }
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(s) => KeyboardTarget::modifiers(s, seat, data, modifiers, serial),
            },
            KeyboardFocusTarget::LayerSurface(l) => {
                KeyboardTarget::modifiers(l.wl_surface(), seat, data, modifiers, serial)
            }
            KeyboardFocusTarget::Popup(p) => {
                KeyboardTarget::modifiers(p.wl_surface(), seat, data, modifiers, serial)
            }
        }
    }
}

impl WaylandFocus for KeyboardFocusTarget {
    #[inline]
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            KeyboardFocusTarget::Window(w) => w.wl_surface(),
            KeyboardFocusTarget::LayerSurface(l) => Some(Cow::Borrowed(l.wl_surface())),
            KeyboardFocusTarget::Popup(p) => Some(Cow::Borrowed(p.wl_surface())),
        }
    }
}

impl From<Window> for KeyboardFocusTarget {
    fn from(window: Window) -> Self {
        KeyboardFocusTarget::Window(window)
    }
}

impl From<LayerSurface> for KeyboardFocusTarget {
    fn from(surface: LayerSurface) -> Self {
        KeyboardFocusTarget::LayerSurface(surface)
    }
}

impl From<PopupKind> for KeyboardFocusTarget {
    fn from(popup: PopupKind) -> Self {
        KeyboardFocusTarget::Popup(popup)
    }
}


#[derive(Debug, Clone, PartialEq)]
pub enum PointerFocusTarget {
    WlSurface(WlSurface),
    #[cfg(feature = "xwayland")]
    X11Surface(X11Surface),
}

impl IsAlive for PointerFocusTarget {
    #[inline]
    fn alive(&self) -> bool {
        match self {
            PointerFocusTarget::WlSurface(w) => w.alive(),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => w.alive(),
        }
    }
}

impl PointerTarget<GlobalData> for PointerFocusTarget {
    fn enter(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::MotionEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::enter(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::enter(w, seat, data, event),
        }
    }

    fn motion(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::MotionEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::motion(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::motion(w, seat, data, event),
        }
    }

    fn relative_motion(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::RelativeMotionEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::relative_motion(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::relative_motion(w, seat, data, event),
        }
    }

    fn button(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::ButtonEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::button(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::button(w, seat, data, event),
        }
    }

    fn axis(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        frame: smithay::input::pointer::AxisFrame
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::axis(w, seat, data, frame),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::axis(w, seat, data, frame),
        }   
    }

    fn frame(&self, seat: &smithay::input::Seat<GlobalData>, data: &mut GlobalData) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::frame(w, seat, data),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::frame(w, seat, data),
        }
    }

    fn leave(&self, seat: &smithay::input::Seat<GlobalData>, data: &mut GlobalData, serial: Serial, time: u32) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::leave(w, seat, data, serial, time),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::leave(w, seat, data, serial, time),
        }
    }

    fn gesture_swipe_begin(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::GestureSwipeBeginEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::gesture_swipe_begin(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::gesture_swipe_begin(w, seat, data, event),
        }
    }

    fn gesture_swipe_update(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::GestureSwipeUpdateEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::gesture_swipe_update(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::gesture_swipe_update(w, seat, data, event),
        } 
    }

    fn gesture_swipe_end(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::GestureSwipeEndEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::gesture_swipe_end(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::gesture_swipe_end(w, seat, data, event),
        } 
    }

    fn gesture_pinch_begin(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::GesturePinchBeginEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::gesture_pinch_begin(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::gesture_pinch_begin(w, seat, data, event),
        } 
    }

    fn gesture_pinch_update(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::GesturePinchUpdateEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::gesture_pinch_update(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::gesture_pinch_update(w, seat, data, event),
        } 
    }


    fn gesture_pinch_end(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::GesturePinchEndEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::gesture_pinch_end(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::gesture_pinch_end(w, seat, data, event),
        } 
    }

    fn gesture_hold_begin(
        &self, 
        seat: &smithay::input::Seat<GlobalData>, 
        data: &mut GlobalData, 
        event: &smithay::input::pointer::GestureHoldBeginEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::gesture_hold_begin(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::gesture_hold_begin(w, seat, data, event),
        } 
    }

    fn gesture_hold_end(
        &self,
        seat: &smithay::input::Seat<GlobalData>, 
        data: 
        &mut GlobalData, 
        event: &smithay::input::pointer::GestureHoldEndEvent
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::gesture_hold_end(w, seat, data, event),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => PointerTarget::gesture_hold_end(w, seat, data, event),
        } 
    }
}

impl TouchTarget<GlobalData> for PointerFocusTarget {
    fn down(
        &self,
        seat: &smithay::input::Seat<GlobalData>,
        data: &mut GlobalData,
        event: &smithay::input::touch::DownEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::down(w, seat, data, event, seq),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => TouchTarget::down(w, seat, data, event, seq),
        }
    }

    fn up(
        &self,
        seat: &smithay::input::Seat<GlobalData>,
        data: &mut GlobalData,
        event: &smithay::input::touch::UpEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::up(w, seat, data, event, seq),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => TouchTarget::up(w, seat, data, event, seq),
        }
    }

    fn motion(
        &self,
        seat: &smithay::input::Seat<GlobalData>,
        data: &mut GlobalData,
        event: &smithay::input::touch::MotionEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::motion(w, seat, data, event, seq),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => TouchTarget::motion(w, seat, data, event, seq),
        }
    }

    fn frame(&self, seat: &smithay::input::Seat<GlobalData>, data: &mut GlobalData, seq: Serial) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::frame(w, seat, data, seq),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => TouchTarget::frame(w, seat, data, seq),
        }
    }

    fn cancel(&self, seat: &smithay::input::Seat<GlobalData>, data: &mut GlobalData, seq: Serial) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::cancel(w, seat, data, seq),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => TouchTarget::cancel(w, seat, data, seq),
        }
    }

    fn shape(
        &self,
        seat: &smithay::input::Seat<GlobalData>,
        data: &mut GlobalData,
        event: &smithay::input::touch::ShapeEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::shape(w, seat, data, event, seq),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => TouchTarget::shape(w, seat, data, event, seq),
        }
    }

    fn orientation(
        &self,
        seat: &smithay::input::Seat<GlobalData>,
        data: &mut GlobalData,
        event: &smithay::input::touch::OrientationEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::orientation(w, seat, data, event, seq),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => TouchTarget::orientation(w, seat, data, event, seq),
        }
    }
}

impl WaylandFocus for PointerFocusTarget {
    #[inline]
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            PointerFocusTarget::WlSurface(w) => w.wl_surface(),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => w.wl_surface().map(Cow::Owned),
        }
    }
    #[inline]
    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            PointerFocusTarget::WlSurface(w) => w.same_client_as(object_id),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => w.same_client_as(object_id),
        }
    }
}

impl From<WlSurface> for PointerFocusTarget {
    #[inline]
    fn from(value: WlSurface) -> Self {
        PointerFocusTarget::WlSurface(value)
    }
}

impl From<PopupKind> for PointerFocusTarget {
    #[inline]
    fn from(value: PopupKind) -> Self {
        PointerFocusTarget::from(value.wl_surface().clone())
    }
}

#[cfg(feature = "xwayland")]
impl From<X11Surface> for PointerFocusTarget {
    #[inline]
    fn from(value: X11Surface) -> Self {
        PointerFocusTarget::X11Surface(value)
    }
}

impl From<KeyboardFocusTarget> for PointerFocusTarget {
    #[inline]
    fn from(value: KeyboardFocusTarget) -> Self {
        match value {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => PointerFocusTarget::from(w.wl_surface().clone()),
                #[cfg(feature = "xwayland")]
                WindowSurface::X11(s) => PointerFocusTarget::from(s.clone()),
            },
            KeyboardFocusTarget::LayerSurface(surface) => PointerFocusTarget::from(surface.wl_surface().clone()),
            KeyboardFocusTarget::Popup(popup) => PointerFocusTarget::from(popup.wl_surface().clone()),
        }
    }
}

