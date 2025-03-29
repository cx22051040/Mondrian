use itertools::Itertools;
use regex::Regex;
use std::collections::HashMap;
use std::fs;

pub struct Configs {
    // conf_path: String,
    pub conf_keybindings: HashMap<String, String>,
    pub conf_priority_map: HashMap<String, i32>,
}

impl Configs {
    pub fn new(path: String) -> Self {
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

        Configs {
            // conf_path: path,
            conf_keybindings: keybindings,
            conf_priority_map,
        }
    }

    fn load_keybindings(path: &str) -> HashMap<String, String> {
        let content = fs::read_to_string(path).expect("Failed to load keybindings config");
        let mut bindings = HashMap::<String, String>::new();
        let re =
            Regex::new(r#"[\n]*bind[\s]*=[\s]*([\w]+[\+]?[\w]*),[\s]*exec,[\s]*"([^"]*)",[\n]*"#)
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
            let keys: Vec<String> = cap[1]
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

            keys.iter().for_each(|key| {
                bindings.insert(key.to_string(), cap[2].trim().to_string());
            });
        }

        #[cfg(feature = "trace_input")]
        tracing::info!("Keybindings: {:?}", bindings);

        bindings
    }
}
