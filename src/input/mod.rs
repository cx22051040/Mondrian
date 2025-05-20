pub mod keybindings;
pub mod move_grab;
pub mod resize_grab;

use smithay::{
    backend::input::{
        AbsolutePositionEvent, ButtonState, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerButtonEvent, PointerMotionEvent
    },
    desktop::{layer_map_for_output, WindowSurfaceType},
    input::{
        keyboard::{xkb::keysym_get_name, FilterResult},
        pointer::{ButtonEvent, MotionEvent, RelativeMotionEvent},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Serial, SERIAL_COUNTER},
    wayland::{compositor::get_parent, pointer_constraints::{with_pointer_constraint, PointerConstraint}, seat::WaylandFocus, shell::wlr_layer::Layer as WlrLayer},
};

use crate::{
    input::keybindings::{FunctionEnum, KeyAction},
    manager::workspace::WorkspaceId,
    state::GlobalData,
};

impl GlobalData {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let event_state = event.state();
                let conf_priority_map = self
                    .configs
                    .conf_keybinding_manager
                    .conf_priority_map
                    .clone();

                let keyboard = self.input_manager.get_keyboard();
                let keyboard = match keyboard {
                    Some(k) => k,
                    None => {
                        error!("get keyboard error");
                        return;
                    }
                };

                keyboard.input::<(), _>(
                    self,
                    event.key_code(),
                    event_state,
                    serial,
                    time,
                    |data, _modifiers, _keysym_handle| {
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

                            pressed_keys_name.sort_by_key(|key| {
                                conf_priority_map.get(key).cloned().unwrap_or(3)
                            });

                            let keys = pressed_keys_name.join("+");

                            #[cfg(feature = "trace_input")]
                            tracing::info!("Keys: {:?}", keys);

                            data.action_keys(keys, serial);
                        }

                        FilterResult::Forward
                    },
                );
            }

            InputEvent::PointerMotion { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                
                let pointer = self.input_manager.get_pointer();
                let pointer = match pointer {
                    Some(k) => k,
                    None => {
                        error!("get pointer error");
                        return;
                    }
                };

                let mut position = pointer.current_location();

                let under = self.surface_under(position);

                let mut pointer_locked = false;
                let mut pointer_confined = false;
                let mut confine_region = None;
                if let Some((surface, location)) = &under {
                    with_pointer_constraint(surface, &pointer, |constraint| match constraint {
                        Some(constraint) => {
                            if !constraint.region().map_or(true, |x| {
                                x.contains((position - *location).to_i32_round())
                            }) {
                                return;
                            }
                            match &*constraint {
                                PointerConstraint::Locked(_locked) => {
                                    pointer_locked = true;
                                }
                                PointerConstraint::Confined(confine) => {
                                    pointer_confined = true;
                                    confine_region = confine.region().cloned();
                                }
                            }
                        }
                        None => {}
                    });
                }

                pointer.relative_motion(
                    self, 
                    under.clone(), 
                    &RelativeMotionEvent {
                        delta: event.delta(),
                        delta_unaccel: event.delta_unaccel(),
                        utime: event.time(),
                    },
                );

                // If pointer is locked, only emit relative motion
                if pointer_locked {
                    pointer.frame(self);
                    return;
                }

                position += event.delta();

                // clamp to screen limits
                // this event is never generated by winit
                let clamp_position = self.clamp_coords(position);
                let new_under = self.surface_under(clamp_position);

                // If confined, don't move pointer if it would go outside surface or region
                if pointer_confined {
                    if let Some((surface, surface_loc)) = &under {
                        if new_under.as_ref().and_then(|(under, _)| under.wl_surface()) != surface.wl_surface() {
                            pointer.frame(self);
                            return;
                        }
                        if let Some(region) = confine_region {
                            if !region.contains((clamp_position - *surface_loc).to_i32_round()) {
                                pointer.frame(self);
                                return;
                            }
                        }
                    }
                }

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: clamp_position,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);

                // If pointer is now in a constraint region, activate it
                // TODO Anywhere else pointer is moved needs to do this
                if let Some((under, surface_location)) =
                    new_under.and_then(|(target, loc)| Some((target.wl_surface()?.into_owned(), loc)))
                {
                    with_pointer_constraint(&under, &pointer, |constraint| match constraint {
                        Some(constraint) if !constraint.is_active() => {
                            let point = (clamp_position - surface_location).to_i32_round();
                            if constraint.region().map_or(true, |region| region.contains(point)) {
                                constraint.activate();
                            }
                        }
                        _ => {}
                    });
                }   
            }

            InputEvent::PointerMotionAbsolute { event } => {
                let serial = SERIAL_COUNTER.next_serial();

                let output = self.output_manager.current_output();
                let output_geo = match self.output_manager.output_geometry(output) {
                    Some(o) => o,
                    None => {
                        warn!("Failed to get output {:?} geometry", output);
                        return;
                    }
                };

                // because the absolute move, need to plus the output location
                let position =
                    event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

                let pointer = self.input_manager.get_pointer();
                let pointer = match pointer {
                    Some(k) => k,
                    None => {
                        error!("get pointer error");
                        return;
                    }
                };

                let under = self.surface_under(position);

                // set focus
                // self.set_focus(under.clone().map(|(surface, _)| surface));

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: position,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }

            InputEvent::PointerButton { event, .. } => {
                let pointer = self.input_manager.get_pointer();
                let pointer = match pointer {
                    Some(k) => k,
                    None => {
                        error!("get pointer error");
                        return;
                    }
                };

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
                    self.action_pointer_button(position, serial);
                }

                // modify pointer state
                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }

            InputEvent::PointerAxis { .. } => {
                // TODO
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

    pub fn surface_under(
        &mut self,
        position: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        // get the surface under giving position,
        let output = self.output_manager.current_output();
        let output_geo = match self.output_manager.output_geometry(output) {
            Some(o) => o,
            None => {
                warn!("Failed to get output {:?} geometry", output);
                return None;
            }
        };

        let layer_map = layer_map_for_output(output);

        if let Some(layer) = layer_map
            .layer_under(WlrLayer::Overlay, position - output_geo.loc.to_f64())
            .or_else(|| layer_map.layer_under(WlrLayer::Top, position - output_geo.loc.to_f64()))
        {
            let layer_surface_loc = layer_map.layer_geometry(layer).unwrap().loc;
            layer
                .surface_under(
                    position - output_geo.loc.to_f64() - layer_surface_loc.to_f64(),
                    WindowSurfaceType::ALL,
                )
                .map(|(surface, loc)| {
                    (surface, (loc + layer_surface_loc + output_geo.loc).to_f64())
                })
        } else if let Some((surface, location)) = self
            .workspace_manager
            .element_under(position)
            .and_then(|(window, location)| {
                window
                    .surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
        {
            Some((surface, location))
        } else {
            None
        }
    }

    pub fn action_pointer_button(&mut self, position: Point<f64, Logical>, serial: Serial) {
        // TODO: remove clone
        let output = self.output_manager.current_output().clone();
        let output_geo = match self.output_manager.output_geometry(&output) {
            Some(g) => g,
            None => {
                warn!("Failed to get output {:?} geometry", output);
                return;
            }
        };

        let layer_map = layer_map_for_output(&output);

        // TODO: First is full screen
        if let Some(layer) = layer_map
            .layer_under(WlrLayer::Overlay, position - output_geo.loc.to_f64())
            .or_else(|| layer_map.layer_under(WlrLayer::Top, position - output_geo.loc.to_f64()))
        {
            if layer.can_receive_keyboard_focus() {
                if let Some((_, _)) = layer.surface_under(
                    position
                        - output_geo.loc.to_f64()
                        - layer_map.layer_geometry(layer).unwrap().loc.to_f64(),
                    WindowSurfaceType::ALL,
                ) {
                    self.set_focus(Some(layer.wl_surface().clone()), serial);
                    return;
                }
            }
        } else if let Some((window, location)) = self
            .workspace_manager
            .element_under(position)
            .map(|(w, l)| (w.clone(), l.clone()))
        {
            let surface = window
                .surface_under(position - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, _)| s);

            // unfocus all window
            self.modify_all_windows_state(false);

            self.workspace_manager.raise_element(&window, true);

            self.set_focus(surface, serial);
        } else if let Some(layer) = layer_map
            .layer_under(WlrLayer::Bottom, position - output_geo.loc.to_f64())
            .or_else(|| {
                layer_map.layer_under(WlrLayer::Background, position - output_geo.loc.to_f64())
            })
        {
            if layer.can_receive_keyboard_focus() {
                if let Some((_, _)) = layer.surface_under(
                    position
                        - output_geo.loc.to_f64()
                        - layer_map.layer_geometry(layer).unwrap().loc.to_f64(),
                    WindowSurfaceType::ALL,
                ) {
                    self.set_focus(Some(layer.wl_surface().clone()), serial);
                    return;
                }
            }
        } else {
            self.set_focus(None, serial);
        }
    }

    pub fn action_keys(&mut self, keys: String, serial: Serial) {
        let conf_keybindings = self
            .configs
            .conf_keybinding_manager
            .conf_keybindings
            .clone();

        if let Some(command) = conf_keybindings.get(&keys) {
            match command {
                KeyAction::Command(cmd) => {
                    #[cfg(feature = "trace_input")]
                    info!("Command: {}", cmd);
                    std::process::Command::new(cmd).spawn().ok();
                }
                KeyAction::Internal(func) => match func {
                    FunctionEnum::SwitchWorkspace1 => {
                        self.set_focus(None, serial);
                        self.workspace_manager.set_activated(WorkspaceId::new(1));
                    }
                    FunctionEnum::SwitchWorkspace2 => {
                        self.set_focus(None, serial);
                        self.workspace_manager.set_activated(WorkspaceId::new(2));
                    }
                    FunctionEnum::InvertWindow => {
                        self.workspace_manager.invert_window();
                    }
                    FunctionEnum::Quit => {
                        info!("Quit");
                        std::process::exit(0);
                    }
                },
            }
        }
    }

    pub fn modify_all_windows_state(&self, activate: bool) {
        for win in self.workspace_manager.elements() {
            win.set_activated(activate);
            win.toplevel().unwrap().send_pending_configure();
        }
    }

    pub fn set_focus(&mut self, surface: Option<WlSurface>, serial: Serial) {
        let keyboard = self.input_manager.get_keyboard();
        let keyboard = match keyboard {
            Some(k) => k,
            None => {
                error!("get keyboard error");
                return;
            }
        };

        let root = match surface.clone() {
            Some(surface) => {
                let mut root = surface;
                while let Some(parent) = get_parent(&root) {
                    root = parent;
                }
                Some(root)
            }
            None => None,
        };

        // unfocus all window
        self.modify_all_windows_state(false);

        keyboard.set_focus(self, root, serial);

        self.workspace_manager.set_focus(surface);
    }

    pub fn _get_focus(&self) -> Option<WlSurface> {
        let keyboard = self.input_manager.get_keyboard();
        let keyboard = match keyboard {
            Some(k) => k,
            None => {
                error!("get keyboard error");
                return None;
            }
        };

        keyboard.current_focus()
    }

    fn clamp_coords(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        // TODO: finish this
        let output = self.output_manager.current_output();
        let output_geo = self.output_manager.output_geometry(output).unwrap();


        let x = pos.x.clamp(0.0, output_geo.size.w as f64);
        let y = pos.y.clamp(0.0, output_geo.size.h as f64);

        (x, y).into()
    }
}
