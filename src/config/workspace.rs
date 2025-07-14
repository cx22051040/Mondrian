use std::fs;

use crate::layout::TiledScheme;

#[derive(Debug, Clone)]
pub struct WorkspaceConfigs {
    pub gap: i32,
    pub scheme: TiledScheme,
}

impl WorkspaceConfigs {
    pub fn default() -> Self {
        Self {
            gap: 12,
            scheme: TiledScheme::Default,
        }
    }

    pub fn load_configs(&mut self, path: &str) -> anyhow::Result<()> {
        let _content = fs::read_to_string(path)?;
        Ok(())
    }
}