use crate::input::keybindings::KeybindingsManager;

#[derive(Debug, Clone)]
pub struct WorkspaceConfigs {
    pub gap: i32,
}

impl Default for WorkspaceConfigs {
    fn default() -> Self {
        Self { gap: 6 }
    }
}

#[derive(Debug, Clone)]
pub struct Configs {
    // conf_path: String,
    pub conf_keybinding_manager: KeybindingsManager,
    pub conf_workspaces: WorkspaceConfigs,
}

impl Configs {
    pub fn new(path: &str) -> Self {
        Self {
            conf_keybinding_manager: KeybindingsManager::new(path),
            conf_workspaces: Default::default(),
        }
    }
}
