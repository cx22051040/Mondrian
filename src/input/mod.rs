pub mod move_grab;
pub mod resize_grab;
pub mod pointer;
pub mod keyboard;

use smithay::{
    backend::input::{
        InputBackend, InputEvent,
    },
};

use crate::state::GlobalData;

impl GlobalData {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => { self.on_keyboard_key_event::<I>(event); }
            InputEvent::PointerMotion { event } => { self.on_pointer_motion::<I>(event); }
            InputEvent::PointerMotionAbsolute { event } => { self.on_pointer_motion_absolute::<I>(event); }
            InputEvent::PointerButton { event, .. } => { self.on_pointer_button::<I>(event); }
            InputEvent::PointerAxis { event } => { self.on_pointer_axis::<I>(event); }
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
