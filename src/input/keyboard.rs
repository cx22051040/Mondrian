use smithay::{
    backend::input::{
            Event, InputBackend, KeyState, KeyboardKeyEvent
        }, input::keyboard::{
        xkb::keysym_get_name, FilterResult
    }, reexports::wayland_server::protocol::wl_surface::WlSurface, utils::{Serial, SERIAL_COUNTER}
};

use crate::{manager::{input::{FunctionEnum, KeyAction}, workspace::WorkspaceId}, state::GlobalData};

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
                                        if name == "Control_L" {
                                            #[cfg(feature = "trace_input")]
                                            info!("mainmod_pressed: true");
                                            
                                            data.input_manager.is_mainmod_pressed = true;
                                        }
                                        name
                                    })
                                    .collect()
                            });

                        pressed_keys_name.sort_by_key(|key| {
                            priority_map.get(key).cloned().unwrap_or(3)
                        });

                        let keys = pressed_keys_name.join("+");

                        #[cfg(feature = "trace_input")]
                        info!("Keys: {:?}", keys);

                        data.action_keys(keys, serial);
                    }
                    KeyState::Released => {
                        let keysym_value = keysym_handle.modified_sym();
                        let name = keysym_get_name(keysym_value);
                        if name == "Control_L" {
                            #[cfg(feature = "trace_input")]
                            info!("mainmod_pressed: false");

                            data.input_manager.is_mainmod_pressed = false;
                        }
                    }
                }
                FilterResult::Forward
            },
        );
    }

    pub fn action_keys(&mut self, keys: String, serial: Serial) {     
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

                    match command.spawn() {
                        #[cfg(feature = "trace_input")]
                        Ok(child) => {
                            info!("Command spawned with PID: {}", child.id());
                        }
                        Err(e) => {
                            error!("Failed to execute command '{} {}': {}", cmd, args.join(" "), e);
                        }
                        _ => {}
                    }
                }
                KeyAction::Internal(func) => match func {
                    FunctionEnum::SwitchWorkspace1 => {
                        self.set_keyboard_focus(None, serial);
                        self.workspace_manager.set_activated(WorkspaceId::new(1));
                    }
                    FunctionEnum::SwitchWorkspace2 => {
                        self.set_keyboard_focus(None, serial);
                        self.workspace_manager.set_activated(WorkspaceId::new(2));
                    }
                    FunctionEnum::InvertWindow => {
                        self.workspace_manager.invert_window();
                    }
                    FunctionEnum::Expansion => {
                        self.workspace_manager.tiled_expansion();
                    }
                    FunctionEnum::Recover => {
                        self.workspace_manager.tiled_recover();
                    }
                    FunctionEnum::Quit => {
                        if let Some(focus) = &self.workspace_manager.current_workspace().focus {
                            info!("quit");
                            let toplevel = focus.toplevel().unwrap();
                            toplevel.send_close();
                        }
                    }
                    FunctionEnum::Kill => {
                        info!("Kill the full compositor");
                        std::process::exit(0);
                    }
                    FunctionEnum::Json => {
                        // TODO
                        if let Some(tiled_tree) = &mut self.workspace_manager.current_workspace_mut().tiled_tree.as_mut() {
                            tiled_tree.from_json("src/config/group.json");
                        }
                    }
                },
            }
        }
    }

    pub fn set_keyboard_focus(&mut self, surface: Option<WlSurface>, serial: Serial) {
        let keyboard = self.input_manager.get_keyboard();
        let keyboard = match keyboard {
            Some(k) => k,
            None => {
                error!("get keyboard error");
                return;
            }
        };

        keyboard.set_focus(self, surface, serial);
    }
}