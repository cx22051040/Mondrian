use smithay::{backend::{input::{AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyboardKeyEvent, PointerButtonEvent}, winit::WinitInput}, desktop::WindowSurfaceType, input::{keyboard::FilterResult, pointer::{ButtonEvent, MotionEvent}}, reexports::wayland_server::protocol::wl_surface::WlSurface, utils::SERIAL_COUNTER};

use crate::state::NuonuoState;

pub fn process_input_event(event: InputEvent<WinitInput>, state: &mut NuonuoState) {
	match event {
		InputEvent::Keyboard { event, .. } => {
			let serial = SERIAL_COUNTER.next_serial();
			let time = Event::time_msec(&event);

			let keyboard = state.seat.get_keyboard().unwrap();
			keyboard.input::<(), _>(
				state,
				event.key_code(),
				event.state(),
				serial,
				time,
				|_, _, _| FilterResult::Forward,
			);
		}

		InputEvent::PointerMotion { .. } => {
			// TODO
		}

		InputEvent::PointerMotionAbsolute { event } => {
			let output = state.space.outputs().next().unwrap();
			let output_geo = state.space.output_geometry(output).unwrap();
			let position = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

			let serial = SERIAL_COUNTER.next_serial();
			let pointer = state.seat.get_pointer().unwrap();
			let under = {
				state.space.element_under(position)
					.and_then(|(window, location)| {
						window
						.surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
						.map(|(s, p)| (s, (p + location).to_f64()))
					})
			};

			// TODO: change keyboard focus here when any window under the pointer

			pointer.motion(
				state,
				under,
				&MotionEvent{
					location: position,
					serial,
					time: event.time_msec(),
				},
			);
			pointer.frame(state);
		}

		InputEvent::PointerButton { event, .. } => {
			let pointer = state.seat.get_pointer().unwrap();
			let keyboard = state.seat.get_keyboard().unwrap();

			let serial = SERIAL_COUNTER.next_serial();

			let button = event.button_code();
			let button_state = event.state();

			tracing::info!("The PointerButton event, button: {button}, location: {:?}", pointer.current_location());

			if button_state == ButtonState::Pressed {
				if let Some((window, _loc)) = state
					.space
					.element_under(pointer.current_location())
					.map(|(w, l)| (w.clone(), l))
				{
					state.space.raise_element(&window, true);
					keyboard.set_focus(state, Some(window.toplevel().unwrap().wl_surface().clone()), serial);
				} else {
					state.space.elements().for_each(|window| {
						window.set_activated(false);
						window.toplevel().unwrap().send_pending_configure();
					});
					keyboard.set_focus(state, Option::<WlSurface>::None, serial);
				}
			}

			// modify pointer state
			pointer.button(
				state,
				&ButtonEvent {
						button,
						state: button_state,
						serial,
						time: event.time_msec(),
				},
			);
			pointer.frame(state);
		}

		InputEvent::PointerAxis { .. } => {
			// TODO
		}

		InputEvent::DeviceAdded { device } => {
			// TODO
			tracing::info!("DeviceAdded Event, device: {:?} ", device);
		}
		InputEvent::DeviceRemoved { device } => {
			// TODO
			tracing::info!("DeviceRemoved Event, device: {:?} ", device);
		}
		_ => {}
	}
}
