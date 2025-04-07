use smithay::desktop::{Space, Window};

#[derive(Debug)]
pub enum LayoutScheme {
    Default,
    BinaryTree,
}

#[derive(Debug)]
pub struct WorkspaceLayout {
    scheme: LayoutScheme,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self {
            scheme: LayoutScheme::Default,
        }
    }
}

impl WorkspaceLayout {
    pub fn set_layout(&mut self, scheme: LayoutScheme) {
        self.scheme = scheme;
    }

    pub fn current_layout(&self) -> &LayoutScheme {
        &self.scheme
    }

    pub fn mapped_windows(&self, space: &mut Space<Window>) {
        match self.scheme {
            LayoutScheme::Default => {
                #[cfg(feature = "trace_layout")]
                tracing::info!("Applying Default Layout");

                let rate = 2;
                


            }
            LayoutScheme::BinaryTree => {
                #[cfg(feature = "trace_layout")]
                tracing::info!("Applying Binary Tree Layout");

                // TODO: future work
            }
        }
    }
}

