use smithay::{
    desktop::Window,
    input::pointer::{CursorImageStatus, GrabStartData as PointerGrabStartData, PointerGrab},
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Rectangle},
};

use crate::{layout::ResizeEdge, state::GlobalData};

pub struct ResizeSurfaceGrab {
    start_data: PointerGrabStartData<GlobalData>,
    window: Window,
    #[allow(dead_code)]
    edge: ResizeEdge,
    #[allow(dead_code)]
    initial_rect: Rectangle<i32, Logical>,
}

impl ResizeSurfaceGrab {
    pub fn start(
        start_data: PointerGrabStartData<GlobalData>,
        window: Window,
        edge: ResizeEdge,
        initial_rect: Rectangle<i32, Logical>,
    ) -> Self {
        let xdg = window.toplevel().unwrap();
        xdg.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
        });
        xdg.send_pending_configure();

        Self {
            start_data,
            window,
            edge,
            initial_rect,
        }
    }
}

impl PointerGrab<GlobalData> for ResizeSurfaceGrab {
    fn start_data(&self) -> &PointerGrabStartData<GlobalData> {
        &self.start_data
    }

    fn unset(&mut self, state: &mut GlobalData) {
        let toplevel = self.window.toplevel().unwrap();
        toplevel.with_pending_state(|state| {
            state.states.unset(xdg_toplevel::State::Resizing);
        });
        toplevel.send_pending_configure();

        state
            .cursor_manager
            .set_cursor_image(CursorImageStatus::default_named());
    }

    fn motion(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        _focus: Option<(
            <GlobalData as smithay::input::SeatHandler>::PointerFocus,
            smithay::utils::Point<f64, Logical>,
        )>,
        event: &smithay::input::pointer::MotionEvent,
    ) {
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;

        data.workspace_manager
            .resize(&self.edge, delta.to_i32_round(), &data.loop_handle);

        self.start_data.location = event.location;

        // let mut new_window_width = self.initial_rect.size.w;
        // let mut new_window_height = self.initial_rect.size.h;

        // if self.edges.intersects(ResizeEdge::LEFT | ResizeEdge::RIGHT) {
        //     // if self.edges.intersects(ResizeEdge::LEFT) {
        //     //     delta.x = -delta.x
        //     // }
        //     new_window_width = (self.initial_rect.size.w as f64 + delta.x) as i32;
        // }
        // if self.edges.intersects(ResizeEdge::TOP | ResizeEdge::BOTTOM) {
        //     // if self.edges.intersects(ResizeEdge::TOP) {
        //     //     delta.y = -delta.y;
        //     // }
        //     new_window_height = (self.initial_rect.size.h as f64 + delta.y) as i32;
        // }

        // let (min_size, max_size) =
        //     compositor::with_states(self.window.toplevel().unwrap().wl_surface(), |states| {
        //         let mut guard = states.cached_state.get::<SurfaceCachedState>();
        //         let data = guard.current();
        //         (data.min_size, data.max_size)
        //     });

        // let min_width = min_size.w.max(1);
        // let min_height = min_size.h.max(1);

        // let max_width = (max_size.w == 0).then(i32::max_value).unwrap_or(max_size.w);
        // let max_height = (max_size.h == 0).then(i32::max_value).unwrap_or(max_size.h);

        // self.last_window_size = Size::from((
        //     new_window_width.max(min_width).min(max_width),
        //     new_window_height.max(min_height).min(max_height),
        // ));

        // let xdg = self.window.toplevel().unwrap();
        // xdg.with_pending_state(|state| {
        //     state.states.set(xdg_toplevel::State::Resizing);
        //     state.size = Some(self.last_window_size);
        // });

        // xdg.send_pending_configure();
    }

    fn relative_motion(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        focus: Option<(
            <GlobalData as smithay::input::SeatHandler>::PointerFocus,
            smithay::utils::Point<f64, Logical>,
        )>,
        event: &smithay::input::pointer::RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
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
            handle.unset_grab(self, data, event.serial, event.time, true);

            let xdg = self.window.toplevel().unwrap();
            xdg.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Resizing);
            });

            xdg.send_pending_configure();
        }
    }

    fn axis(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
        details: smithay::input::pointer::AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(
        &mut self,
        data: &mut GlobalData,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, GlobalData>,
    ) {
        handle.frame(data);
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

// TODO: only for floating window
// pub fn handle_commit(workspace_manager: &mut WorkspaceManager, surface: &WlSurface) -> Option<()> {
//     let window = match workspace_manager.find_window(surface) {
//         Some(window) => window,
//         None => {
//             return None;
//         }
//     };

//     let mut window_loc = match workspace_manager.window_geometry(&window) {
//         Some(rec) => rec.loc,
//         None => {
//             warn!("Failed to get location from window: {:?}", window);
//             return None;
//         }
//     };

//     let geometry = window.geometry();

//     let new_loc: Point<Option<i32>, Logical> = ResizeSurfaceState::with(surface, |state| {
//         state
//             .commit()
//             .and_then(|(edges, initial_rect)| {
//                 // If the window is being resized by top or left, its location must be adjusted
//                 // accordingly.
//                 edges.intersects(ResizeEdge::TOP_LEFT).then(|| {
//                     let new_x = edges
//                         .intersects(ResizeEdge::LEFT)
//                         .then_some(initial_rect.loc.x + (initial_rect.size.w - geometry.size.w));
//                     let new_y = edges
//                         .intersects(ResizeEdge::TOP)
//                         .then_some(initial_rect.loc.y + (initial_rect.size.h - geometry.size.h));
//                     (new_x, new_y).into()
//                 })
//             })
//             .unwrap_or_default()
//     });

//     if let Some(new_x) = new_loc.x {
//         window_loc.x = new_x;
//     }
//     if let Some(new_y) = new_loc.y {
//         window_loc.y = new_y;
//     }

//     if new_loc.x.is_some() || new_loc.y.is_some() {
//         // If TOP or LEFT side of the window got resized, we have to move it
//         workspace_manager.map_element(None, window.clone(), window_loc, None, false);
//     }
//     Some(())
// }
