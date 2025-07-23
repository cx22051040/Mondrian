use smithay::{
    delegate_foreign_toplevel_list, wayland::foreign_toplevel_list::ForeignToplevelListHandler,
};

use crate::state::GlobalData;

impl ForeignToplevelListHandler for GlobalData {
    fn foreign_toplevel_list_state(&mut self) -> &mut smithay::wayland::foreign_toplevel_list::ForeignToplevelListState {
        &mut self.state.foreign_toplevel_state
    }
}
delegate_foreign_toplevel_list!(GlobalData);

