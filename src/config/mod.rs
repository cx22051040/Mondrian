use std::{collections::HashMap, sync::Arc};

use regex::Regex;

use crate::layout::tiled_tree::TiledScheme;

#[derive(Debug, Clone)]
pub struct WorkspaceConfigs {
    pub gap: i32,
    pub scheme: TiledScheme,
}

impl WorkspaceConfigs {
    fn default() -> Self {
        Self { 
            gap: 12,
            scheme: TiledScheme::Default,

        }
    }
}

#[derive(Debug, Clone)]
pub struct Configs {
    pub exec_once_cmds: Vec<(String, Vec<String>)>,
    pub env_vars: HashMap<String, String>,

    pub conf_workspaces: Arc<WorkspaceConfigs>,
}

impl Configs {
    pub fn new() -> Self {
        let content = include_str!("./mondrian.conf").to_string();

        let re_exec = Regex::new(r#"^\s*exec-once\s*=\s*(.+)$"#).unwrap();
        let re_env = Regex::new(r#"^\s*env\s*=\s*([^,\s]+)\s*,\s*(.+)$"#).unwrap();

        let mut exec_once_cmds = Vec::new();
        let mut env_vars = HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
        
            if let Some(cap) = re_exec.captures(line) {
                let mut parts = cap[1].trim().split_whitespace();
                let cmd = parts.next().unwrap_or("").to_string();
                let args: Vec<String> = parts.map(|s| s.to_string()).collect();

                exec_once_cmds.push((cmd, args));

            } else if let Some(cap) = re_env.captures(line) {
                let key = cap[1].trim();
                let val = cap[2].trim();

                env_vars.insert(key.to_string(), val.to_string());
            }
        }

        Self {
            exec_once_cmds,
            env_vars,
            conf_workspaces: Arc::new(WorkspaceConfigs::default()),
        }
    }

    pub fn init(&self) {
        for (cmd, args) in &self.exec_once_cmds {
            let mut command = std::process::Command::new(cmd);
            command.args(args);
            
            match command.spawn() {
                #[cfg(feature="trace_input")]
                Ok(child) => info!("Spawned: {} (PID: {})", cmd, child.id()),
                Err(e) => error!("Failed to run '{}': {}", cmd, e),
                #[cfg(not(feature="trace_input"))]
                _ => { }
            }
        }

        for (key, val) in &self.env_vars {
            unsafe {
                info!("set {} = {}", key, val);
                std::env::set_var(key, val);
            }
        }
    }
}
