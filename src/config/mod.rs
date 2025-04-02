use crate::input::keybindings::KeybindingsManager;

pub struct Configs {
    // conf_path: String,
    pub conf_keybinding_manager: KeybindingsManager,
}

impl Configs {
    pub fn new(path: &str) -> Self {
        Self {
            conf_keybinding_manager: KeybindingsManager::new(path),
        }
    }
}
