use smithay::{
    desktop::Window,
    input::pointer::{CursorImageStatus, GrabStartData as PointerGrabStartData, PointerGrab},
    utils::{Logical, Point},
};

use crate::state::NuonuoState;

pub struct PointerMoveSurfaceGrab {
    // TODO: can use smaller struct such as InputState
    pub start_data: PointerGrabStartData<NuonuoState>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<NuonuoState> for PointerMoveSurfaceGrab {
    fn start_data(&self) -> &PointerGrabStartData<NuonuoState> {
        &self.start_data
    }

    fn unset(&mut self, state: &mut NuonuoState) {
        state.cursor_manager.set_cursor_image(
            CursorImageStatus::default_named()
        );
    }

    fn frame(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
    ) {
        handle.frame(data);
    }

    fn button(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::ButtonEvent,
    ) {
        handle.button(data, event);

        // The button is a button code as defined in the
        // Linux kernel's linux/input-event-codes.h header file, e.g. BTN_LEFT.
        const BTN_LEFT: u32 = 0x110;

        if !handle.current_pressed().contains(&BTN_LEFT) {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn motion(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        _focus: Option<(
            <NuonuoState as smithay::input::SeatHandler>::PointerFocus,
            Point<f64, Logical>,
        )>,
        event: &smithay::input::pointer::MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;
        data.space
            .map_element(self.window.clone(), new_location.to_i32_round(), true);
    }

    fn relative_motion(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        focus: Option<(
            <NuonuoState as smithay::input::SeatHandler>::PointerFocus,
            Point<f64, Logical>,
        )>,
        event: &smithay::input::pointer::RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn axis(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        details: smithay::input::pointer::AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        event: &smithay::input::pointer::GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }
}

