use std::{collections::HashMap, fs};

use regex::Regex;

#[derive(Debug, Clone)]
pub struct WindowRulesConfigs {
    pub global_opacity: HashMap<String, f32> // such: kitty, 0.95
}

impl WindowRulesConfigs {
    pub fn default() -> Self {
        Self { global_opacity: HashMap::new() }
    }

    pub fn load_configs(&mut self, path: &str) -> anyhow::Result<()> {
        let content = fs::read_to_string(path)?;

        let mut global_opacity = HashMap::<String, f32>::new();

        let re_windowrule = 
            // windowrule = opacity 1.00, app_id: ^(firefox)$
            Regex::new(
                r#"(?m)^\s*windowrule\s*=\s*opacity\s+([0-9.]+)\s*,\s*app_id\s*:\s*(.+?)\s*$"#
            ).unwrap();

        for cap in re_windowrule.captures_iter(&content) {
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
        
        #[cfg(feature = "trace_config")]
        info!("{:?}", global_opacity);
        
        self.global_opacity = global_opacity;

        Ok(())
    }
}