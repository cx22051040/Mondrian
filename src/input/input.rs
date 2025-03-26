use smithay::{backend::{input::{AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyState, KeyboardKeyEvent, PointerButtonEvent}, winit::WinitInput}, desktop::WindowSurfaceType, input::{keyboard::{xkb::keysym_get_name, FilterResult, ModifiersState}, pointer::{ButtonEvent, MotionEvent}}, reexports::{input::event::KeyboardEvent, wayland_server::protocol::wl_surface::WlSurface}, utils::SERIAL_COUNTER};

use crate::CalloopData;

pub fn process_input_event(event: InputEvent<WinitInput>, calloop_data: &mut CalloopData) {
	match event {
		InputEvent::Keyboard { event, .. } => {
			let serial = SERIAL_COUNTER.next_serial();
			let time = Event::time_msec(&event);
			let event_state = event.state();

			let keyboard = &mut calloop_data.state.seat.get_keyboard().unwrap();
			
			// TODO: inhabit shift+word when other modifiers are actived

			keyboard.input::<(), _>(
				&mut calloop_data.state,
				event.key_code(),
				event_state,
				serial,
				time,
				|_data, _modifiers, _keysym_handle| {

					if event_state == KeyState::Pressed {
						let mut pressed_keys_name: Vec<String> = keyboard.with_pressed_keysyms(|keysym_handles| {
							keysym_handles
								.iter()
								.map(|keysym_handle|{
									let keysym_value = keysym_handle.modified_sym();
									keysym_get_name(keysym_value)
								})
								.collect()
						});

						pressed_keys_name
							.sort_by_key(|key| calloop_data.configs.conf_priority_map.get(key).cloned().unwrap_or(3));

						let keys = pressed_keys_name.join("+");

						#[cfg(feature = "trace_input")]
						tracing::info!("Keys: {:?}", keys);

						if let Some(command) =  calloop_data.configs.conf_keybindings.get(&keys) {
							tracing::info!("Command: {}", command);
							std::process::Command::new(command).spawn().ok();
						}
					}

					FilterResult::Forward
				},
			);
		}

		InputEvent::PointerMotion { .. } => {
			// TODO
		}

		InputEvent::PointerMotionAbsolute { event } => {
			let output = calloop_data.state.space.outputs().next().unwrap();
			let output_geo = calloop_data.state.space.output_geometry(output).unwrap();
			let position = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

			let serial = SERIAL_COUNTER.next_serial();
			let pointer = calloop_data.state.seat.get_pointer().unwrap();
			let under = {
				calloop_data.state.space.element_under(position)
					.and_then(|(window, location)| {
						window
						.surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
						.map(|(s, p)| (s, (p + location).to_f64()))
					})
			};

			// TODO: change keyboard focus here when any window under the pointer

			pointer.motion(
				&mut calloop_data.state,
				under,
				&MotionEvent{
					location: position,
					serial,
					time: event.time_msec(),
				},
			);
			pointer.frame(&mut calloop_data.state);
		}

		InputEvent::PointerButton { event, .. } => {
			let pointer = calloop_data.state.seat.get_pointer().unwrap();
			let keyboard = calloop_data.state.seat.get_keyboard().unwrap();

			let serial = SERIAL_COUNTER.next_serial();

			let button = event.button_code();
			let button_state = event.state();

			#[cfg(feature = "trace_input")]
			tracing::info!("The PointerButton event, button: {button}, location: {:?}", pointer.current_location());

			if button_state == ButtonState::Pressed && !pointer.is_grabbed() {
				if let Some((window, _loc)) = calloop_data.state
					.space
					.element_under(pointer.current_location())
					.map(|(w, l)| (w.clone(), l))
				{
					calloop_data.state.space.raise_element(&window, true);
					keyboard.set_focus(&mut calloop_data.state, Some(window.toplevel().unwrap().wl_surface().clone()), serial);
				} else {
					calloop_data.state.space.elements().for_each(|window| {
						window.set_activated(false);
						window.toplevel().unwrap().send_pending_configure();
					});
					keyboard.set_focus(&mut calloop_data.state, Option::<WlSurface>::None, serial);
				}
			}

			// modify pointer state
			pointer.button(
				&mut calloop_data.state,
				&ButtonEvent {
						button,
						state: button_state,
						serial,
						time: event.time_msec(),
				},
			);
			pointer.frame(&mut calloop_data.state);
		}

		InputEvent::PointerAxis { .. } => {
			// TODO
		}

		InputEvent::DeviceAdded { device } => {
			// TODO
			#[cfg(feature = "trace_input")]
			tracing::info!("DeviceAdded Event, device: {:?} ", device);
		}
		
		InputEvent::DeviceRemoved { device } => {
			// TODO
			#[cfg(feature = "trace_input")]
			tracing::info!("DeviceRemoved Event, device: {:?} ", device);
		}
		_ => {}
	}
}

// pub fn get_modifiers_name (modifiers: &ModifiersState) -> Vec<String> {
// 	[
//     ("Ctrl", modifiers.ctrl),
//     ("Alt", modifiers.alt),
//     ("Shift", modifiers.shift),
//     ("CapsLock", modifiers.caps_lock),
//     ("Super", modifiers.logo), // Windows/Command 键
//     ("NumLock", modifiers.num_lock),
//     ("AltGr", modifiers.iso_level3_shift),
//     ("ISO_Level5", modifiers.iso_level5_shift),
// 	]
// 	.iter()
// 	.filter_map(|(name, active)| active.then(|| name.to_string()))
// 	.collect()
// }