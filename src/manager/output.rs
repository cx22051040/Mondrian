use std::sync::Arc;

use smithay::{
    desktop::{Space, Window},
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::wayland_server::DisplayHandle,
    utils::{Logical, Point, Raw, Rectangle, Size, Transform},
    wayland::output::OutputManagerState,
};

use crate::{config::Configs, state::GlobalData};

#[derive(Debug)]
pub struct OutputElement {
    pub output: Output,
    pub activate: bool,
}

impl OutputElement {
    pub fn new(output: Output, activate: bool) -> Self {
        Self { output, activate }
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
    #[allow(dead_code)]
    pub output_manager_state: OutputManagerState,
    // This space does not actually contain any windows, but all outputs are
    // mapped into it
    pub output_space: Space<Window>,

    #[allow(dead_code)]
    pub configs: Arc<Configs>,
}

impl OutputManager {
    pub fn new(display_handle: &DisplayHandle, configs: Arc<Configs>) -> Self {
        let output_manager_state =
            OutputManagerState::new_with_xdg_output::<GlobalData>(display_handle);
        let output_space: Space<Window> = Default::default();

        Self {
            outputs: Vec::new(),
            output_manager_state,
            output_space,
            configs,
        }
    }

    pub fn add_output(
        &mut self,
        name: String,
        size: Size<i32, Raw>,
        subpixel: Subpixel,
        make: String,
        model: String,
        location: Point<i32, Logical>,
        activate: bool,
        display_handle: &DisplayHandle,
    ) {
        let output = Output::new(
            name,
            PhysicalProperties {
                size,
                subpixel,
                make,
                model,
            },
        );
        let _ = output.create_global::<GlobalData>(display_handle);

        self.output_space.map_output(&output, location);

        self.outputs.push(OutputElement::new(output, activate));
    }

    pub fn remove_output(&mut self, output: &Output) {
        if let Some(pos) = self.outputs.iter().position(|o| o.output == *output) {
            self.output_space.unmap_output(output);
            self.outputs.remove(pos);
        } else {
            warn!("Failed to remove output: Output not found in the list");
            return;
        }
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

    pub fn output_geometry(&self, output: &Output) -> Option<Rectangle<i32, Logical>> {
        self.output_space.output_geometry(output)
    }
}
