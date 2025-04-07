use smithay::{
    backend::{
        input::{
            AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyState, KeyboardKeyEvent,
            PointerButtonEvent,
        },
        winit::WinitInput,
    },
    desktop::WindowSurfaceType,
    input::{
        keyboard::{FilterResult, xkb::keysym_get_name},
        pointer::{ButtonEvent, MotionEvent},
    },
    utils::SERIAL_COUNTER,
};

use crate::{
    NuonuoState,
    input::keybindings::{FunctionEnum, KeyAction},
};

pub fn process_input_event(event: InputEvent<WinitInput>, nuonuo_state: &mut NuonuoState) {
    match event {
        InputEvent::Keyboard { event, .. } => {
            let serial = SERIAL_COUNTER.next_serial();
            let time = Event::time_msec(&event);
            let event_state = event.state();
            let conf_priority_map = nuonuo_state
                .configs
                .conf_keybinding_manager
                .conf_priority_map
                .clone();
            let conf_keybindings = nuonuo_state
                .configs
                .conf_keybinding_manager
                .conf_keybindings
                .clone();

            let keyboard = &mut nuonuo_state.seat.get_keyboard().unwrap();

            // TODO: inhabit shift+word when other modifiers are actived

            keyboard.input::<(), _>(
                nuonuo_state,
                event.key_code(),
                event_state,
                serial,
                time,
                |state, _modifiers, _keysym_handle| {
                    if event_state == KeyState::Pressed {
                        let mut pressed_keys_name: Vec<String> =
                            keyboard.with_pressed_keysyms(|keysym_handles| {
                                keysym_handles
                                    .iter()
                                    .map(|keysym_handle| {
                                        let keysym_value = keysym_handle.modified_sym();
                                        keysym_get_name(keysym_value)
                                    })
                                    .collect()
                            });

                        pressed_keys_name
                            .sort_by_key(|key| conf_priority_map.get(key).cloned().unwrap_or(3));

                        let keys = pressed_keys_name.join("+");

                        #[cfg(feature = "trace_input")]
                        tracing::info!("Keys: {:?}", keys);

                        if let Some(command) = conf_keybindings.get(&keys) {
                            match command {
                                KeyAction::Command(cmd) => {
                                    tracing::info!("Command: {}", cmd);
                                    std::process::Command::new(cmd).spawn().ok();
                                }
                                KeyAction::Internal(func) => {
                                    match func {
                                        FunctionEnum::SwitchWorkspace1 => {
                                            let serial = SERIAL_COUNTER.next_serial();
                                            // TODO: set focus to the first window, also move cursor to it
                                            keyboard.set_focus(state, None, serial);
                                            state
                                                .configs
                                                .conf_keybinding_manager
                                                .switch_workspace1(&mut state.workspace_manager);
                                        }
                                        FunctionEnum::SwitchWorkspace2 => {
                                            let serial = SERIAL_COUNTER.next_serial();
                                            keyboard.set_focus(state, None, serial);
                                            state
                                                .configs
                                                .conf_keybinding_manager
                                                .switch_workspace2(&mut state.workspace_manager);
                                        }
                                    }
                                }
                            }
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
            let output = nuonuo_state.output_manager.current_output();
            let output_geo = nuonuo_state.workspace_manager.output_geometry(output);
            // because the absolute move, need to plus the output location
            let position = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

            let serial = SERIAL_COUNTER.next_serial();
            let pointer = nuonuo_state.seat.get_pointer().unwrap();
            let keyboard = nuonuo_state.seat.get_keyboard().unwrap();

            let under = nuonuo_state
                .workspace_manager
                .element_under(position)
                .map(|(w, l)| (w.clone(), l.clone()));

            let current_focus = keyboard.current_focus();

            if let Some((window, location)) = under {
                let under_surface = window
                    .surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, _)| s);

                // modify when focus changed
                if current_focus != under_surface {
                    nuonuo_state.workspace_manager.raise_element(&window, true);

                    // modify all window
                    for win in nuonuo_state.workspace_manager.elements() {
                        win.toplevel().unwrap().send_pending_configure();
                    }

                    keyboard.set_focus(
                        nuonuo_state,
                        Some(window.toplevel().unwrap().wl_surface().clone()),
                        serial,
                    );
                }
            } else if current_focus.is_some() {
                // have prev focus, but get none
                for win in nuonuo_state.workspace_manager.elements() {
                    win.set_activated(false);
                    win.toplevel().unwrap().send_pending_configure();
                }
                keyboard.set_focus(nuonuo_state, None, serial);
            }

            // TODO:
            let under_surface = nuonuo_state
                .workspace_manager
                .element_under(position)
                .and_then(|(window, location)| {
                    window
                        .surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
                        .map(|(s, p)| (s, (p + location).to_f64()))
                });

            pointer.motion(
                nuonuo_state,
                under_surface,
                &MotionEvent {
                    location: position,
                    serial,
                    time: event.time_msec(),
                },
            );
            pointer.frame(nuonuo_state);
        }

        InputEvent::PointerButton { event, .. } => {
            let pointer = nuonuo_state.seat.get_pointer().unwrap();
            let keyboard = nuonuo_state.seat.get_keyboard().unwrap();

            let serial = SERIAL_COUNTER.next_serial();

            let button = event.button_code();
            let button_state = event.state();

            #[cfg(feature = "trace_input")]
            tracing::info!(
                "The PointerButton event, button: {button}, location: {:?}",
                pointer.current_location()
            );

            if button_state == ButtonState::Pressed && !pointer.is_grabbed() {
                let position = pointer.current_location();

                let under = nuonuo_state
                    .workspace_manager
                    .element_under(position)
                    .map(|(w, l)| (w.clone(), l.clone()));

                let current_focus = keyboard.current_focus();

                if let Some((window, location)) = under {
                    let under_surface = window
                        .surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
                        .map(|(s, _)| s);

                    // modify when focus changed
                    if current_focus != under_surface {
                        nuonuo_state.workspace_manager.raise_element(&window, true);

                        // modify all window
                        for win in nuonuo_state.workspace_manager.elements() {
                            win.toplevel().unwrap().send_pending_configure();
                        }

                        keyboard.set_focus(
                            nuonuo_state,
                            Some(window.toplevel().unwrap().wl_surface().clone()),
                            serial,
                        );
                    }
                } else if current_focus.is_some() {
                    // have prev focus, but click none
                    for win in nuonuo_state.workspace_manager.elements() {
                        win.set_activated(false);
                        win.toplevel().unwrap().send_pending_configure();
                    }
                    keyboard.set_focus(nuonuo_state, None, serial);
                }
            }

            // modify pointer state
            pointer.button(
                nuonuo_state,
                &ButtonEvent {
                    button,
                    state: button_state,
                    serial,
                    time: event.time_msec(),
                },
            );
            pointer.frame(nuonuo_state);
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
