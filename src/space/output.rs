use smithay::{
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::wayland_server::DisplayHandle,
    utils::{Logical, Point, Transform},
    wayland::output::OutputManagerState,
};

use crate::state::NuonuoState;

pub struct OutputElement {
    pub output: Output,
    pub activate: bool,
}

impl OutputElement {
    pub fn new(name: &str, display_handle: &DisplayHandle, activate: bool) -> Self {
        let output = Output::new(
            name.to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "Smithay".into(),
                model: "Winit".into(),
            },
        );
        let _ = output.create_global::<NuonuoState>(display_handle);
        Self {
            output,
            activate,
        }
    }

    pub fn change_current_state(
        &mut self,
        mode: Option<Mode>,
        transform: Option<Transform>,
        scale: Option<Scale>,
        location: Option<Point<i32, Logical>>,
    ) {
        self.output
            .change_current_state(mode, transform, scale, location);
    }

    pub fn set_preferred(&mut self, mode: Mode) {
        self.output.set_preferred(mode);
    }

    pub fn output(&self) -> &Output {
        &self.output
    }
}

pub struct OutputManager {
    pub outputs: Vec<OutputElement>,
    pub output_manager_state: OutputManagerState,
}

impl OutputManager {
    pub fn new(display_handle: &DisplayHandle) -> Self {
        let output_manager_state =
            OutputManagerState::new_with_xdg_output::<NuonuoState>(&display_handle);

        Self {
            outputs: Vec::new(),
            output_manager_state,
        }
    }

    pub fn add_output(&mut self, name: &str, display_handle: &DisplayHandle, activate: bool) {
        self.outputs
            .push(OutputElement::new(name, display_handle, activate));
    }

    pub fn _remove_output() {
        todo!()
    }

    pub fn current_output(&self) -> &Output {
        self.outputs.iter().find(|o| o.activate).unwrap().output()
    }

    pub fn change_current_state(
        &mut self,
        mode: Option<Mode>,
        transform: Option<Transform>,
        scale: Option<Scale>,
        location: Option<Point<i32, Logical>>,
    ) {
        self.outputs
            .iter_mut()
            .find(|o| o.activate)
            .unwrap()
            .change_current_state(mode, transform, scale, location);
    }

    pub fn set_preferred(&mut self, mode: Mode) {
        self.outputs
            .iter_mut()
            .find(|o| o.activate)
            .unwrap()
            .set_preferred(mode);
    }
}

