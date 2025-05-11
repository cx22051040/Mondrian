use smithay::output::Output;

#[cfg(feature = "tty")]
use crate::backend::tty::{Tty, TtyRenderer};
use crate::manager::workspace::Workspace;

use super::
    elements::CustomRenderElements
;


#[cfg(feature = "tty")]
impl Tty {
    pub fn render_output(&mut self, output: &Output, workspace: &Workspace, custom_elements: Vec<CustomRenderElements<TtyRenderer>>) {
    
    }
}