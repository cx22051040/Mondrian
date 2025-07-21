use smithay::{
    backend::input::{Event, InputBackend, KeyState, KeyboardKeyEvent}, desktop::WindowSurface, input::keyboard::{xkb::keysym_get_name, FilterResult}, utils::{Serial, SERIAL_COUNTER}
};

use crate::{
    config::keybinding::{FunctionEnum, KeyAction}, input::focus::KeyboardFocusTarget, manager::workspace::WorkspaceId, state::GlobalData
};

impl GlobalData {
    pub fn on_keyboard_key_event<I: InputBackend>(&mut self, event: I::KeyboardKeyEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time_msec(&event);
        let event_state = event.state();
        let priority_map = self.input_manager.get_priority_map().clone();

        let keyboard = self.input_manager.get_keyboard();
        let keyboard = match keyboard {
            Some(k) => k,
            None => {
                error!("get keyboard error");
                return;
            }
        };

        let main_mod = self.input_manager.get_mainmode().clone();

        keyboard.input::<(), _>(
            self,
            event.key_code(),
            event_state,
            serial,
            time,
            |data, _modifiers, keysym_handle| {
                match event_state {
                    KeyState::Pressed => {
                        let mut pressed_keys_name: Vec<String> =
                            keyboard.with_pressed_keysyms(|keysym_handles| {
                                keysym_handles
                                    .iter()
                                    .map(|keysym_handle| {
                                        let keysym_value = keysym_handle.modified_sym();
                                        let name = keysym_get_name(keysym_value);
                                        if name == main_mod {
                                            #[cfg(feature = "trace_input")]
                                            info!("mainmod_pressed: true");

                                            data.input_manager.set_mainmod(true);
                                        } else if let Some(cap) = name.strip_prefix("XF86Switch_VT_") {
                                            let vt = cap.parse::<usize>().unwrap_or(1);
                                            data.backend.change_vt(vt as i32);
                                        }
                                        name
                                    })
                                    .collect()
                            });

                        pressed_keys_name
                            .sort_by_key(|key| priority_map.get(key).cloned().unwrap_or(3));

                        let keys = pressed_keys_name.join("+");

                        #[cfg(feature = "trace_input")]
                        info!("Keys: {:?}", keys);

                        // if get keybindings, do not send keyboard event to clients
                        if data.action_keys(keys, serial) {
                            return FilterResult::Intercept(());
                        }
                    }
                    KeyState::Released => {
                        let keysym_value = keysym_handle.modified_sym();
                        let name = keysym_get_name(keysym_value);
                        if name == main_mod {
                            #[cfg(feature = "trace_input")]
                            info!("mainmod_pressed: false");

                            data.input_manager.set_mainmod(false);
                        }
                    }
                }
                FilterResult::Forward
            },
        );
    }

    pub fn action_keys(&mut self, keys: String, serial: Serial) -> bool {
        let _span = tracy_client::span!("keyboard_action");

        let keybindings = self.input_manager.get_keybindings();

        if let Some(command) = keybindings.get(&keys) {
            match command {
                KeyAction::Command(cmd, args) => {
                    #[cfg(feature = "trace_input")]
                    info!("Command: {} {}", cmd, args.join(" "));

                    let mut command = std::process::Command::new(cmd);

                    for arg in args {
                        command.arg(arg);
                    }

                    // use current display
                    let mut envs = vec![("WAYLAND_DISPLAY", self.socket_name.clone())];

                    #[cfg(feature = "xwayland")]
                    if let Some(ref xdisplay) = self.state.xdisplay {
                        envs.push(("DISPLAY", format!(":{}", xdisplay)));
                    }

                    command.envs(envs);

                    match command.spawn() {
                        #[cfg(feature = "trace_input")]
                        Ok(child) => {
                            info!("Command spawned with PID: {}", child.id());
                        }
                        Err(e) => {
                            error!(
                                "Failed to execute command '{} {}': {}",
                                cmd,
                                args.join(" "),
                                e
                            );
                        }
                        #[cfg(not(feature = "trace_input"))]
                        _ => {}
                    }

                    return true;
                }
                KeyAction::Internal(func) => match func {
                    FunctionEnum::InvertWindow => {
                        if let Some(KeyboardFocusTarget::Window(target)) = self.input_manager.get_keyboard_focus() {
                            self.workspace_manager.invert_window(&target, &mut self.animation_manager);
                        }
                    }
                    FunctionEnum::Expansion => {
                        self.workspace_manager.tiled_expansion(&mut self.animation_manager);
                    }
                    FunctionEnum::Recover => {
                        self.workspace_manager.tiled_recover(&mut self.animation_manager);
                    }
                    FunctionEnum::Quit => {
                        if let Some(KeyboardFocusTarget::Window(window)) = self.input_manager.get_keyboard_focus() {
                            match window.underlying_surface() {
                                WindowSurface::Wayland(toplevel) => {
                                    toplevel.send_close();
                                },
                                #[cfg(feature = "xwayland")]
                                WindowSurface::X11(x11_surface) => {
                                    let _ = x11_surface.close();
                                }
                            }
                        }
                    }
                    FunctionEnum::Up(edge)
                    | FunctionEnum::Down(edge)
                    | FunctionEnum::Left(edge)
                    | FunctionEnum::Right(edge) => {
                        if let Some(KeyboardFocusTarget::Window(target)) = self.input_manager.get_keyboard_focus() {
                            self.workspace_manager.exchange_window(&target, edge, &mut self.animation_manager);
                        }
                    }
                    FunctionEnum::Kill => {
                        info!("Kill the full compositor");
                        std::process::exit(0);
                    }
                    FunctionEnum::Json => {
                        // TODO
                    }

                    FunctionEnum::SwitchWorkspace(id) => {
                        let output = self.output_manager.current_output();
                        let output_geo = self.output_manager
                            .output_geometry(output).unwrap();

                        self.workspace_manager.switch_workspace(WorkspaceId::new(*id), output_geo, &mut self.animation_manager);

                        self.update_output_working_size();
                        self.set_keyboard_focus(None, serial);
                    }
                },
            }
        
            return true;
        }

        return false;
    }

    pub fn update_keyboard_focus(&mut self) {
        let serial = SERIAL_COUNTER.next_serial();
        
        let pointer = self.input_manager.get_pointer();
        let pointer = match pointer {
            Some(k) => k,
            None => {
                error!("get pointer error");
                return;
            }
        };

        let pointer_loc = pointer.current_location();

        self.focus_target_under(pointer_loc, serial, true);
    }

    pub fn set_keyboard_focus(&mut self, focus_target: Option<KeyboardFocusTarget>, serial: Serial) {
        let keyboard = self.input_manager.get_keyboard();
        let keyboard = match keyboard {
            Some(k) => k,
            None => {
                error!("get keyboard error");
                return;
            }
        };

        keyboard.set_focus(self, focus_target, serial);
    }
}

