pub mod keyboard;
pub mod move_grab;
pub mod pointer;
pub mod resize_grab;
pub mod focus;

use smithay::{
    backend::input::{
        InputBackend, InputEvent
    }, 
    delegate_primary_selection, delegate_seat, 
    input::{
        Seat, SeatHandler, SeatState
    }, 
    reexports::wayland_server::{protocol::wl_surface::WlSurface, Resource}, 
    wayland::{
        seat::WaylandFocus, 
        selection::{
            data_device::set_data_device_focus, 
            primary_selection::{
                set_primary_focus, PrimarySelectionHandler, PrimarySelectionState
            }
        }
    }
};

use crate::{input::focus::{KeyboardFocusTarget, PointerFocusTarget}, state::GlobalData};

impl GlobalData {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                self.on_keyboard_key_event::<I>(event);
            }
            InputEvent::PointerMotion { event } => {
                self.on_pointer_motion::<I>(event);
            }
            InputEvent::PointerMotionAbsolute { event } => {
                self.on_pointer_motion_absolute::<I>(event);
            }
            InputEvent::PointerButton { event, .. } => {
                self.on_pointer_button::<I>(event);
            }
            InputEvent::PointerAxis { event } => {
                self.on_pointer_axis::<I>(event);
            }
            InputEvent::DeviceAdded { .. } => {
                // TODO
                info!("Device added");
            }
            InputEvent::DeviceRemoved { .. } => {
                // TODO
                info!("Device removed");
            }
            _ => {}
        }
    }
}

impl SeatHandler for GlobalData {
    type KeyboardFocus = KeyboardFocusTarget;
    type PointerFocus = PointerFocusTarget;
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

    fn focus_changed(&mut self, seat: &Seat<Self>, target: Option<&KeyboardFocusTarget>) {
        let display_handle = &self.display_handle;

        let wl_surface = target.and_then(WaylandFocus::wl_surface);

        let client = wl_surface.and_then(|s| display_handle.get_client(s.id()).ok());
        set_data_device_focus(display_handle, seat, client.clone());
        set_primary_focus(display_handle, seat, client);
    }

    fn led_state_changed(
        &mut self,
        _seat: &Seat<Self>,
        _led_state: smithay::input::keyboard::LedState,
    ) {
        // TODO
        info!("led state changed");
    }
}
delegate_seat!(GlobalData);

impl PrimarySelectionHandler for GlobalData {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.state.primary_selection_state
    }
}
delegate_primary_selection!(GlobalData);