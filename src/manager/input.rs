use std::{collections::HashMap, sync::Arc};

use smithay::{
    input::{
        Seat, SeatState, keyboard::KeyboardHandle, pointer::PointerHandle, touch::TouchHandle,
    },
    reexports::wayland_server::DisplayHandle,
};

use crate::{config::keybinding::{KeyAction, KeybindingConfigs}, state::GlobalData, utils::errors::AnyHowErr};



pub struct InputManager {
    pub seat_state: SeatState<GlobalData>,
    seat: Seat<GlobalData>,

    // global data
    is_mainmod_pressed: bool,

    // keybindings
    configs: Arc<KeybindingConfigs>,
}

impl InputManager {
    pub fn new(
        seat_name: String,
        display_handle: &DisplayHandle,
        configs: Arc<KeybindingConfigs>,
    ) -> anyhow::Result<Self> {
        let mut seat_state = SeatState::new();

        let mut seat = seat_state.new_wl_seat(display_handle, seat_name);

        seat.add_keyboard(Default::default(), 200, 25)
            .anyhow_err("Failed to add keyboard")?;
        seat.add_pointer();

        Ok(Self {
            seat_state,
            seat,
            is_mainmod_pressed: false,
            configs,
        })
    }

    pub fn set_mainmode(&mut self, activate: bool) {
        self.is_mainmod_pressed = activate;
    }

    pub fn is_mainmod_pressed(&self) -> bool {
        self.is_mainmod_pressed
    }

    pub fn get_keybindings(&self) -> &HashMap<String, KeyAction> {
        &self.configs.keybindings
    }

    pub fn get_priority_map(&self) -> &HashMap<String, i32> {
        &self.configs.priority_map
    }

    pub fn get_keyboard(&self) -> Option<KeyboardHandle<GlobalData>> {
        self.seat.get_keyboard()
    }

    pub fn get_pointer(&self) -> Option<PointerHandle<GlobalData>> {
        self.seat.get_pointer()
    }

    pub fn _get_touch(&self) -> Option<TouchHandle<GlobalData>> {
        self.seat.get_touch()
    }
}
