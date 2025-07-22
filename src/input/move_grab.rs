use smithay::{
    desktop::Window,
    input::pointer::{CursorImageStatus, GrabStartData as PointerGrabStartData, PointerGrab},
    utils::{Logical, Point},
};

use crate::{layout::WindowLayout, state::GlobalData};

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData<GlobalData>,
    pub initial_layout: WindowLayout,
    pub window: Window,
}

impl MoveSurfaceGrab {
    pub fn start(
        start_data: PointerGrabStartData<GlobalData>,
        initial_layout: WindowLayout,
        window: Window,
    ) -> Self {
        Self {
            start_data,
            initial_layout,
            window,
        }
    }
}

impl PointerGrab<GlobalData> for MoveSurfaceGrab {
    fn start_data(&self) -> &PointerGrabStartData<GlobalData> {
        &self.start_data
    }

    fn unset(&mut self, data: &mut GlobalData) {
        data.cursor_manager
            .set_cursor_image(CursorImageStatus::default_named());
    }

    fn frame(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
    ) {
        handle.frame(data);
    }

    fn button(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::ButtonEvent,
    ) {
        handle.button(data, event);

        // The button is a button code as defined in the
        // Linux kernel's linux/input-event-codes.h header file, e.g. BTN_LEFT.
        const BTN_LEFT: u32 = 0x110;

        if !handle.current_pressed().contains(&BTN_LEFT) {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(self, data, event.serial, event.time, true);

            match self.initial_layout {
                WindowLayout::Tiled => {
                    data.switch_layout(&self.window, self.start_data.location);
                }
                _ => { }
            }
        }
    }

    fn motion(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        _focus: Option<(
            <GlobalData as smithay::input::SeatHandler>::PointerFocus,
            Point<f64, Logical>,
        )>,
        event: &smithay::input::pointer::MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        // TODO: this drop the surface focus, so when release the button
        // client can't get the release info
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;

        data.workspace_manager.grab_move(&self.window, delta.to_i32_round(), &mut data.animation_manager);

        self.start_data.location = event.location;
    }

    fn relative_motion(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        focus: Option<(
            <GlobalData as smithay::input::SeatHandler>::PointerFocus,
            Point<f64, Logical>,
        )>,
        event: &smithay::input::pointer::RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn axis(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        details: smithay::input::pointer::AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        event: &smithay::input::pointer::GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }
}
