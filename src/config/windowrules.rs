use std::{collections::HashMap, fs};

use regex::Regex;

#[derive(Debug, Clone)]
pub struct WindowRulesConfigs {
    pub global_opacity: HashMap<String, f32>, // such: kitty, 0.95
    pub fullscreen: HashMap<String, bool>
}

impl WindowRulesConfigs {
    pub fn default() -> Self {
        Self { 
            global_opacity: HashMap::new(),
            fullscreen: HashMap::new() 
        }
    }

    pub fn load_configs(&mut self, path: &str) -> anyhow::Result<()> {
        let content = fs::read_to_string(path)?;

        let mut global_opacity = HashMap::<String, f32>::new();
        let mut fullscreen = HashMap::<String, bool>::new();

        let re_opacity = 
            // windowrule = opacity 1.00, app_id: ^(firefox)$
            Regex::new(
                r#"(?m)^\s*windowrule\s*=\s*opacity\s+([0-9.]+)\s*,\s*app_id\s*:\s*(.+?)\s*$"#
            ).unwrap();

        let re_fullscreen = 
            // windowrule = fullscreen true, app_id: ^(firefox)$
            Regex::new(
                r#"(?m)^\s*windowrule\s*=\s*fullscreen\s+(true|false)\s*,\s*app_id\s*:\s*(.+?)\s*$"#
            ).unwrap();

        for cap in re_opacity.captures_iter(&content) {
            let opacity_str = cap.get(1).unwrap().as_str();
            let opacity = match opacity_str.parse::<f32>() {
                Ok(val) => val,
                Err(e) => {
                    error!("cannot parse opacity as f32: {:?}", e);
                    continue;
                },
            };
            
            let pattern = cap.get(2).unwrap().as_str();

            // ^(firefox)$ → Exact
            if let Some(caps) = Regex::new(r#"^\^\(([\w\-\.]+)\)\$$"#).unwrap().captures(pattern) {
                global_opacity.insert(caps[1].to_string(), opacity);
            }
            // ^([Ff]irefox)$ → Expand case variants
            else if let Some(caps) = Regex::new(r#"^\^\(\[([a-zA-Z])([a-zA-Z]*)\]([\w\-\.]+)\)\$$"#).unwrap().captures(pattern) {
                let first_chars = vec![caps[1].to_ascii_lowercase(), caps[1].to_ascii_uppercase()];
                let rest = caps[3].to_string();

                for ch in first_chars {
                    let full = format!("{}{}", ch, rest);
                    global_opacity.insert(full, opacity);
                }
            }
        }
        
        for cap in re_fullscreen.captures_iter(&content) {
            let is_fullscreen = cap.get(1)
                .unwrap()
                .as_str();

            let is_fullscreen = match is_fullscreen.parse::<bool>() {
                Ok(val) => val,
                Err(e) => {
                    error!("cannot parse opacity as f32: {:?}", e);
                    continue;
                }
            };
            
            let pattern = cap.get(2).unwrap().as_str();

            // ^(firefox)$ → Exact
            if let Some(caps) = Regex::new(r#"^\^\(([\w\-\.]+)\)\$$"#).unwrap().captures(pattern) {
                fullscreen.insert(caps[1].to_string(), is_fullscreen);
            }
            // ^([Ff]irefox)$ → Expand case variants
            else if let Some(caps) = Regex::new(r#"^\^\(\[([a-zA-Z])([a-zA-Z]*)\]([\w\-\.]+)\)\$$"#).unwrap().captures(pattern) {
                let first_chars = vec![caps[1].to_ascii_lowercase(), caps[1].to_ascii_uppercase()];
                let rest = caps[3].to_string();

                for ch in first_chars {
                    let full = format!("{}{}", ch, rest);
                    fullscreen.insert(full, is_fullscreen);
                }
            }
        }
        
        self.global_opacity = global_opacity;
        self.fullscreen = fullscreen;

        Ok(())
    }
}