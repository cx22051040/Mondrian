#[derive(Debug, Clone)]
pub struct WorkspaceConfigs {
    #[allow(dead_code)]
    pub gap: i32,
}

impl Default for WorkspaceConfigs {
    fn default() -> Self {
        Self { gap: 6 }
    }
}

#[derive(Debug, Clone)]
pub struct Configs {
    #[allow(dead_code)]
    pub conf_workspaces: WorkspaceConfigs,
}

impl Configs {
    pub fn new() -> Self {
        Self {
            conf_workspaces: Default::default(),
        }
    }
}
