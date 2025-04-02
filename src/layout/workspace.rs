use smithay::desktop::Window;

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
        Self { scheme: LayoutScheme::Default }
  }
}

impl WorkspaceLayout {
    pub fn set_layout (&mut self, scheme: LayoutScheme) {
        self.scheme = scheme;
    }

    pub fn current_layout (&self) -> &LayoutScheme {
        &self.scheme
    }

    pub fn mapped_windows (&self, windows: &mut Vec<Window>) {
        match self.scheme {
            LayoutScheme::Default => {
                #[cfg(feature = "trace_layout")]
                tracing::info!("Applying Default Layout");

                // TODO: detect pointer location to add
                // windows, first horizontal
            }
            LayoutScheme::BinaryTree => {
                #[cfg(feature = "trace_layout")]
                tracing::info!("Applying Binary Tree Layout");

                // TODO: future work
            }
        }
    }

}