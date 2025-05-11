use smithay::{
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::wayland_server::{Display, DisplayHandle},
    utils::{Logical, Point, Raw, Size, Transform},
    wayland::output::OutputManagerState,
};

use crate::state::GlobalData;

#[derive(Debug)]
pub struct OutputElement {
    pub output: Output,
    pub activate: bool,
}

impl OutputElement {
    pub fn new(name: String, size: Size<i32, Raw>, subpixel: Subpixel, make: String, model: String, activate: bool, display_handle: &DisplayHandle) -> Self {
        let output = Output::new(
            name,
            PhysicalProperties {
                size,
                subpixel,
                make,
                model,
            }
        );
        let _ = output.create_global::<GlobalData>(display_handle);

        Self {
            output,
            activate,
        }
    }

    pub fn set_preferred(&mut self, mode: Mode) {
        self.output.set_preferred(mode);
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

    pub fn output(&self) -> &Output {
        &self.output
    }
}

pub struct OutputManager {
    pub outputs: Vec<OutputElement>,
    pub output_manager_state: OutputManagerState,
    pub display_handle: DisplayHandle,
}

impl OutputManager {
    pub fn new(display_handle: DisplayHandle) -> Self {

        let output_manager_state = OutputManagerState::new_with_xdg_output::<GlobalData>(&display_handle);

        Self {
            outputs: Vec::new(),
            output_manager_state,
            display_handle,
        }
    }

    pub fn add_output(&mut self, name: String, size: Size<i32, Raw>, subpixel: Subpixel, make: String, model: String, activate: bool) {
        self
            .outputs
            .push(OutputElement::new(name, size, subpixel, make, model, activate, &self.get_display_handle()));
    }

    pub fn _remove_output() {
        todo!()
    }

    pub fn set_preferred(&mut self, mode: Mode) {
        self.outputs
            .iter_mut()
            .find(|o| o.activate)
            .unwrap()
            .set_preferred(mode);
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

    pub fn get_display_handle(&self) -> &DisplayHandle {
        &self.display_handle
    }
}

