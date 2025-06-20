use smithay::{
    desktop::{layer_map_for_output, Space, Window},
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::wayland_server::DisplayHandle,
    utils::{Logical, Point, Raw, Rectangle, Size, Transform},
    wayland::{compositor::send_surface_state, fractional_scale::with_fractional_scale, output::OutputManagerState},
};

use crate::state::GlobalData;

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

    pub fn current_refresh(&self) -> i32 {
        self.output.current_mode().unwrap().refresh
    }
}
pub struct OutputManager {
    pub outputs: Vec<OutputElement>,
    #[allow(dead_code)]
    pub output_manager_state: OutputManagerState,
    // This space does not actually contain any windows, but all outputs are
    // mapped into it
    pub output_space: Space<Window>,
}

impl OutputManager {
    pub fn new(display_handle: &DisplayHandle) -> Self {
        let output_manager_state =
            OutputManagerState::new_with_xdg_output::<GlobalData>(display_handle);
        let output_space: Space<Window> = Default::default();

        Self {
            outputs: Vec::new(),
            output_manager_state,
            output_space,
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

    pub fn current_refresh(&self) -> i32 {
        self.outputs.iter().find(|o| o.activate).unwrap().current_refresh()
    }
}

impl GlobalData {
    pub fn update_output_size(&mut self) {
        let output = self.output_manager.current_output();
        let scale = output.current_scale();
        let transform = output.current_transform();
    
        let mut layer_map = layer_map_for_output(output);
        for layer in layer_map.layers() {
            layer.with_surfaces(|surface, data| {
                send_surface_state(surface, data, scale.integer_scale(), transform);
                with_fractional_scale(data, |fractional| {
                    fractional.set_preferred_scale(scale.fractional_scale());
                });
            });
        }

        layer_map.arrange();

        let output_working_geo = layer_map.non_exclusive_zone();

        self.workspace_manager.update_output_geo(output_working_geo, &self.loop_handle);
    }
}