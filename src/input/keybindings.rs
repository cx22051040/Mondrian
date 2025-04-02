use std::{collections::HashMap, fs};

use regex::Regex;
use itertools::Itertools;

use crate::space::workspace::WorkspaceManager;

// 定义所有的内部可执行函数
#[derive(Debug, Clone)]
pub enum FunctionEnum {
    SwitchWorkspace1,
    SwitchWorkspace2,
}

#[derive(Debug, Clone)]
pub enum KeyAction {
    Command(String),
    Internal(FunctionEnum),
}

pub struct KeybindingsManager {
  pub conf_keybindings: HashMap<String, KeyAction>,
  pub conf_priority_map: HashMap<String, i32>,
}

impl KeybindingsManager {
    pub fn new(path: &str) -> Self {
        let keybindings = Self::load_keybindings(&path);

        // priority: Ctrl > Shift > Alt
        let conf_priority_map: HashMap<String, i32> = [
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

        Self {
            // conf_path: path,
            conf_keybindings: keybindings,
            conf_priority_map,
        }
  }

    fn load_keybindings(path: &str) -> HashMap<String, KeyAction> {
        let content = fs::read_to_string(path).expect("Failed to load keybindings config");
        let mut bindings = HashMap::<String, KeyAction>::new();
        
        let re =
            // bind = Ctrl + t, command, "kitty"
            // bind = Ctrl + 1, exec, "func1"
            Regex::new(r#"(?m)^\s*bind\s*=\s*([\w+]+),\s*(exec|command),\s*"([^"]*)"\s*$"#)
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
            let action = &cap[2];  // exec / command
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
                    "command" => KeyAction::Command(command.trim().to_string()),
                    "exec" => {
                        let internal_action = match command.trim() {
                            "workspace-1" => FunctionEnum::SwitchWorkspace1,
                            "workspace-2" => FunctionEnum::SwitchWorkspace2,
                            _ => {
                                tracing::info!("Warning: No registered function for exec '{}'", command);
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
        tracing::info!("Keybindings: {:?}", bindings);

        bindings
    }

    pub fn switch_workspace1(&self, workspace_manager: &mut WorkspaceManager) {
        workspace_manager.switch_workspace(1);
    }

    pub fn switch_workspace2(&self, workspace_manager: &mut WorkspaceManager) {
        workspace_manager.switch_workspace(2);
    }

}