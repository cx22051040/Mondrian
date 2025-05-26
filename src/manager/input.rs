use std::{collections::HashMap, fs};

use itertools::Itertools;
use regex::Regex;

use smithay::{
    input::{
        keyboard::KeyboardHandle, pointer::PointerHandle, touch::TouchHandle, Seat, SeatState
    },
    reexports::wayland_server::DisplayHandle,
};

use crate::{state::GlobalData, utils::errors::AnyHowErr};

#[derive(Debug)]
pub enum FunctionEnum {
    SwitchWorkspace1,
    SwitchWorkspace2,
    InvertWindow,
    Quit,
    Kill,
}

#[derive(Debug)]
pub enum KeyAction {
    Command(String, Vec<String>),
    Internal(FunctionEnum),
}

pub struct InputManager {
    pub seat_state: SeatState<GlobalData>,
    pub seat: Seat<GlobalData>,

    // keyboard
    pub keybindings: HashMap<String, KeyAction>,
    pub priority_map: HashMap<String, i32>,

    // global data
    pub is_mainmod_pressed: bool
}

impl InputManager {
    pub fn new(seat_name: String, display_handle: &DisplayHandle, keybindgings_path: &str) -> anyhow::Result<Self> {
        let mut seat_state = SeatState::new();
        let seat_name = seat_name;
        info!("seat_name: {:?}", seat_name);
        let mut seat = seat_state.new_wl_seat(display_handle, seat_name);

        seat.add_keyboard(Default::default(), 200, 25).anyhow_err("Failed to add keyboard")?;
        seat.add_pointer();

        let keybindings = Self::load_keybindings(keybindgings_path)?;

        // priority: Ctrl > Shift > Alt
        let priority_map: HashMap<String, i32> = [
            ("Control_L", 0),
            ("Control_R", 0),
            ("Shift_L", 1),
            ("Shift_R", 1),
            ("Alt_L", 2),
            ("Alt_R", 2),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        Ok ( Self { seat_state, seat, keybindings, priority_map, is_mainmod_pressed: false } )
    }

    pub fn get_keybindings(&self) -> &HashMap<String, KeyAction> {
        &self.keybindings
    }

    pub fn get_priority_map(&self) -> &HashMap<String, i32> {
        &self.priority_map
    }

    fn load_keybindings(path: &str) -> anyhow::Result<HashMap<String, KeyAction>> {
        let content = fs::read_to_string(path).anyhow_err("Failed to load keybindings config")?;
        let mut bindings = HashMap::<String, KeyAction>::new();

        let re =
            // bind = Ctrl + t, command, "kitty"
            // bind = Ctrl + 1, exec, "func1"
            Regex::new(r#"(?m)^\s*bind\s*=\s*([^,]+?),\s*(command|exec),\s*"([^"]+)"(?:\s*#.*)?$"#)
                .unwrap();

        let modifier_map: HashMap<&str, Vec<&str>> = [
            ("Ctrl", vec!["Control_L", "Control_R"]),
            ("Shift", vec!["Shift_L", "Shift_R"]),
            ("Alt", vec!["Alt_L", "Alt_R"]),
            ("Esc", vec!["Escape"]),
            ("[", vec!["bracketleft"]),
            ("]", vec!["bracketright"]),
            (",", vec!["comma"]),
            (".", vec!["period"]),
            ("/", vec!["slash"]),
            (";", vec!["semicolon"]),
            (".", vec!["period"]),
            ("'", vec!["apostrophe"]),
        ]
        .into_iter()
        .collect();

        for cap in re.captures_iter(&content) {
            let keybind = &cap[1]; // Ctrl+t / Alt+Enter
            let action = &cap[2]; // exec / command
            let command = &cap[3]; // kitty / rofi -show drun

            let keys: Vec<String> = keybind
                .split('+')
                .map(|key| {
                    if let Some(modifiers) = modifier_map.get(key) {
                        modifiers.iter().map(|m| m.to_string()).collect()
                    } else {
                        vec![key.to_string()]
                    }
                })
                .multi_cartesian_product()
                .map(|combination| combination.join("+"))
                .collect();

            for key in keys {
                let action_enum = match action {
                    "command" => {
                        let mut parts = command.split_whitespace();
                        let cmd = parts.next().unwrap_or("").to_string();
                        let args: Vec<String> = parts.map(|s| s.to_string()).collect();

                        KeyAction::Command(cmd, args)
                    },
                    "exec" => {
                        let internal_action = match command.trim() {
                            "workspace-1" => FunctionEnum::SwitchWorkspace1,
                            "workspace-2" => FunctionEnum::SwitchWorkspace2,
                            "invert" => FunctionEnum::InvertWindow,
                            "quit" => FunctionEnum::Quit,
                            "kill" => FunctionEnum::Kill,
                            _ => {
                                tracing::info!(
                                    "Warning: No registered function for exec '{}'",
                                    command
                                );
                                continue;
                            }
                        };
                        KeyAction::Internal(internal_action)
                    }
                    _ => continue,
                };

                bindings.insert(key.to_string(), action_enum);
            }
        }

        #[cfg(feature = "trace_input")]
        for (key, action) in &bindings {
            tracing::info!(%key, action = ?action, "Keybinding registered");
        }
        
        Ok(bindings)
    }

    pub fn get_keyboard(&self) -> Option<KeyboardHandle<GlobalData>> {
        self.seat.get_keyboard()
    }

    pub fn get_pointer(&self) -> Option<PointerHandle<GlobalData>> {
        self.seat.get_pointer()
    }

    pub fn _get_touch(&self) -> Option<TouchHandle<GlobalData>> {
        self.seat.get_touch()
    }
}

