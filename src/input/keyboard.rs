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
                        priority_map.get(key).cloned().unwrap_or(3)
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

    pub fn action_keys(&mut self, keys: String, serial: Serial) {
        let keybindings = self.input_manager.get_keybindings();

        if let Some(command) = keybindings.get(&keys) {
            match command {
                KeyAction::Command(cmd) => {
                    #[cfg(feature = "trace_input")]
                    info!("Command: {}", cmd);
                    std::process::Command::new(cmd).spawn().ok();
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
                    FunctionEnum::Quit => {
                        info!("Quit");
                        std::process::exit(0);
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