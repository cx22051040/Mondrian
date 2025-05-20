use smithay::{
    input::{
        Seat, SeatState, keyboard::KeyboardHandle, pointer::PointerHandle, touch::TouchHandle,
    },
    reexports::wayland_server::DisplayHandle,
};

use crate::state::GlobalData;

pub struct InputManager {
    pub seat_state: SeatState<GlobalData>,
    pub seat: Seat<GlobalData>,
}

impl InputManager {
    pub fn new(seat_name: String, display_handle: &DisplayHandle) -> Self {
        let mut seat_state = SeatState::new();
        let seat_name = seat_name;
        info!("seat_name: {:?}", seat_name);
        let mut seat = seat_state.new_wl_seat(display_handle, seat_name);

        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        Self { seat_state, seat }
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

