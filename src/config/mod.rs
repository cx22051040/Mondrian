use std::{fs, sync::Arc};

use regex::Regex;

pub mod keybinding;
pub mod workspace;
pub mod windowrules;

use crate::config::{
    keybinding::KeybindingConfigs, 
    workspace::WorkspaceConfigs,
    windowrules::WindowRulesConfigs,
};

#[derive(Debug, Clone)]
pub struct Configs {
    #[allow(dead_code)]
    pub home: String,

    pub exec_once_cmds: Vec<(String, Vec<String>)>,

    pub conf_workspaces: Arc<WorkspaceConfigs>,
    pub conf_keybindings: Arc<KeybindingConfigs>,
    pub conf_windowrules: Arc<WindowRulesConfigs>
}

impl Configs {
    pub fn new() -> Self {
        let re_exec = Regex::new(r#"^\s*exec-once\s*=\s*(.+)$"#).unwrap();
        let re_env = Regex::new(r#"^\s*env\s*=\s*([^,\s]+)\s*,\s*(.+)$"#).unwrap();
        let re_source = Regex::new(r#"^\s*source\s*=\s*([^\s#]+)"#).unwrap();
        
        let home = dirs::home_dir()
            .and_then(|path| path.to_str().map(String::from)).unwrap();
        info!("Using home directory: {}", home);
        
        let config_path = home.clone() + "/.config/Mondrian/mondrian.conf";
        let content = fs::read_to_string(&config_path);
    
        let mut exec_once_cmds = Vec::new();

        let mut conf_workspaces = WorkspaceConfigs::default();
        let mut conf_keybindings = KeybindingConfigs::default();
        let mut conf_windowrules = WindowRulesConfigs::default();

        if let Ok(content) = content {
            info!("using config from {:?}", config_path);
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
    
                // handle environment variable
                let re = Regex::new(r#"\$\{([^}]+)\}"#).unwrap();

                let mut missing = false;
                for caps in re.captures_iter(line) {
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
            
                let line = &re.replace_all(line, |caps: &regex::Captures| {
                    let var_name = &caps[1];
                    std::env::var(var_name).unwrap()
                }).to_string();

                if let Some(cap) = re_exec.captures(line) {
                    let mut parts = cap[1].trim().split_whitespace();
                    let cmd = parts.next().unwrap_or("").to_string();
                    let args: Vec<String> = parts.map(|s| s.to_string()).collect();
    
                    exec_once_cmds.push((cmd, args));
                } else if let Some(cap) = re_env.captures(line) {
                    let key = cap[1].trim();
                    let val = cap[2].trim();

                    #[cfg(feature = "trace_config")]
                    info!("set {} = {}", key, val);
                    
                    unsafe {
                        std::env::set_var(key, val);
                    }
                    
                } else if let Some(cap) = re_source.captures(line) {
                    let source_file = cap[1].trim();
    
                    #[cfg(feature = "trace_config")]
                    info!("Loading source file: {}", source_file);
    
                    if source_file.contains("workspace") {
                        match conf_workspaces.load_configs(source_file) {
                            Ok(_) => info!("Loaded workspace configs from {}", source_file),
                            Err(e) => error!("Failed to load workspace configs: {}", e),
                        }
                    } else if source_file.contains("keybinding") {
                        match conf_keybindings.load_configs(source_file) {
                            Ok(_) => info!("Loaded keybindings configs from {}", source_file),
                            Err(e) => error!("Failed to load keybindings configs: {}", e),
                        }
                    } else if source_file.contains("windowrules") {
                        match conf_windowrules.load_configs(source_file) {
                            Ok(_) => info!("Loaded windowrules configs from {}", source_file),
                            Err(e) => error!("Failed to load keybindings configs: {}", e),
                        }
                    }
                }
            }
        } else {
            warn!("Failed to read mondrian.conf, using default configurations");
        }

        Self {
            home,

            exec_once_cmds,
            conf_workspaces: Arc::new(conf_workspaces),
            conf_keybindings: Arc::new(conf_keybindings),
            conf_windowrules: Arc::new(conf_windowrules)
        }
    }

    pub fn init(&self) {
        for (cmd, args) in &self.exec_once_cmds {
            let mut command = std::process::Command::new(cmd);
            command.args(args);

            match command.spawn() {
                #[cfg(feature = "trace_input")]
                Ok(child) => info!("Spawned: {} (PID: {})", cmd, child.id()),
                Err(e) => error!("Failed to run '{}': {}", cmd, e),

                #[cfg(not(feature = "trace_input"))]
                _ => {}
            }
        }
    }
}
