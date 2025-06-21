use std::{collections::HashMap, fs};

use itertools::Itertools;
use regex::Regex;

use crate::layout::Direction;

#[derive(Debug, Clone)]
pub enum FunctionEnum {
    InvertWindow,
    Expansion,
    Recover,
    Quit,
    Kill,
    Json,
    Up(Direction),
    Down(Direction),
    Left(Direction),
    Right(Direction),

    SwitchWorkspace(usize),
}

#[derive(Debug, Clone)]
pub enum KeyAction {
    Command(String, Vec<String>),
    Internal(FunctionEnum),
}

#[derive(Debug)]
pub struct KeybindingConfigs {
    pub mainmod: String,

    pub keybindings: HashMap<String, KeyAction>,
    pub priority_map: HashMap<String, i32>,
}

impl KeybindingConfigs {
    pub fn default() -> Self {
        // priority: Ctrl > Shift > Alt
        let priority_map: HashMap<String, i32> = [
                ("Super_L", 0),
                ("Control_L", 1),
                ("Control_R", 1),
                ("Shift_L", 2),
                ("Shift_R", 2),
                ("Alt_L", 3),
                ("Alt_R", 3),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();

        Self {
            mainmod: "Super".to_string(),
            keybindings: HashMap::new(),
            priority_map,
        }
    }

    pub fn load_configs(&mut self, path: &str) -> anyhow::Result<()> {

        let content = fs::read_to_string(path)?;
        let mut bindings = HashMap::<String, KeyAction>::new();

        let re_mainmod = Regex::new(r#"^\s*mainMod\s*=\s*([^\s#]+)"#).unwrap();
        if let Some(cap) = re_mainmod.captures(&content) {
            self.mainmod = cap[1].to_string();
        } else {
            warn!("No mainMod found in the configuration file, use default Super");
        }

        let re_bindings =
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
            ("{NUM}", vec!["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]),
            ("{FNUM}", vec!["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"]),
        ]
        .into_iter()
        .collect();

        for cap in re_bindings.captures_iter(&content) {
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
                        let mut args = vec![];

                        let cmd = parts.next().unwrap_or("").to_string();

                        for arg in parts {
                            let re = Regex::new(r#"\$\{([^}]+)\}"#).unwrap();

                            let mut missing = false;
                            for caps in re.captures_iter(arg) {
                                let var_name = &caps[1];
                                if std::env::var(var_name).is_err() {
                                    warn!("Environment variable `{}` is not set, skipping line", var_name);
                                    missing = true;
                                    break;
                                }
                            }
                            if missing {
                                continue;
                            }
                            
                            args.push(
                                re.replace_all(arg, |caps: &regex::Captures| {
                                    let var_name = &caps[1];
                                    std::env::var(var_name).unwrap()
                                }).to_string()
                            );
                        }

                        KeyAction::Command(cmd, args)
                    }
                    "exec" => {
                        let internal_action = match command.trim() {
                            "invert" => FunctionEnum::InvertWindow,
                            "recover" => FunctionEnum::Recover,
                            "expansion" => FunctionEnum::Expansion,
                            "quit" => FunctionEnum::Quit,
                            "kill" => FunctionEnum::Kill,
                            "json" => FunctionEnum::Json,
                            "up" => FunctionEnum::Up(Direction::Up),
                            "down" => FunctionEnum::Down(Direction::Down),
                            "left" => FunctionEnum::Left(Direction::Left),
                            "right" => FunctionEnum::Right(Direction::Right),
                            "switch workspace" => {
                                let id = key.split('+').find_map(|s| {
                                    s.trim().parse::<usize>().ok()
                                }).unwrap_or(1);

                                // workspace start from 1
                                if id < 1 {
                                    continue;
                                }

                                FunctionEnum::SwitchWorkspace(id)
                            },
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
            info!("Keybinding: {} -> {:?}", key, action);
        }

        self.keybindings = bindings;

        Ok(())
    }
}


