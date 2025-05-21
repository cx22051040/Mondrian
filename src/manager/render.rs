use std::time::Instant;

use smithay::{
    backend::renderer::{
        element::{
            memory::MemoryRenderBufferRenderElement, 
            surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement}, 
            Kind
        }, gles::{GlesPixelProgram, GlesRenderer, Uniform, UniformName, UniformType}, Color32F
    },
    desktop::space::SpaceRenderElements,
    utils::Scale,
};

use crate::render::{
        background::BackgroundRenderElement, border::BorderRenderElement, elements::{CustomRenderElements, OutputRenderElements}, NuonuoRenderer
    };

use super::{
    input::InputManager, output::OutputManager, window::WindowExt, workspace::WorkspaceManager, cursor::{CursorManager, RenderCursor, XCursor}
};

pub struct RenderManager {
    // no need now
    pub start_time: Instant,
}

pub struct BorderShader(pub GlesPixelProgram);
pub struct Background(pub GlesPixelProgram);

impl RenderManager {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn compile_shaders(&self, renderer: &mut GlesRenderer) {
        // Compile GLSL file into pixel shader.
        let border_shader = renderer
            .compile_custom_pixel_shader(
                include_str!("../render/shaders/border.frag"),
                &[
                    UniformName::new("u_resolution", UniformType::_2f),
                    UniformName::new("border_color", UniformType::_3f),
                    UniformName::new("border_thickness", UniformType::_1f),
                ],
            )
            .unwrap();

        let background = renderer
            .compile_custom_pixel_shader(
                include_str!("../render/shaders/background.frag"),
                &[
                    UniformName::new("u_resolution", UniformType::_2f),
                    UniformName::new("u_time", UniformType::_1f),
                ],
            )
            .unwrap();

        // Save pixel shader in EGL rendering context.
        renderer
            .egl_context()
            .user_data()
            .insert_if_missing(|| BorderShader(border_shader));
        renderer
            .egl_context()
            .user_data()
            .insert_if_missing(|| Background(background));

    }

    pub fn get_render_elements<R: NuonuoRenderer>(
        &self,
        renderer: &mut R,
        output_manager: &OutputManager,
        workspace_manager: &WorkspaceManager,
        cursor_manager: &mut CursorManager,
        input_manager: &InputManager,
    ) -> Vec<OutputRenderElements<R>> {
        let mut output_elements = vec![];

        // First is Cursor
        output_elements.extend(
            self.get_cursor_render_elements(
                renderer,
                output_manager,
                cursor_manager,
                input_manager,
            )
            .into_iter()
            .map(OutputRenderElements::Custom),
        );

        // Then Some Control elements

        // Then Fullscreen
        // TODO:

        // Then LayerShell Overlay and Top
        // TODO:

        // Then Border
        output_elements.extend(
            self.get_border_render_elements(renderer, workspace_manager)
                .into_iter()
                .map(OutputRenderElements::Custom),
        );

        // Then common Windows
        output_elements.extend(
            self.get_windows_render_elements(renderer, output_manager, workspace_manager)
                .into_iter()
                .map(OutputRenderElements::Space),
        );

        // Then Shader and CustomRenderElements

        // Then LayerShell Bottom and Background
        // TODO:

        output_elements.extend(
            self.get_background_render_elements(renderer, output_manager)
                .into_iter()
                .map(OutputRenderElements::Custom),
        );

        output_elements
    }

    pub fn get_windows_render_elements<R: NuonuoRenderer>(
        &self,
        renderer: &mut R,
        output_manager: &OutputManager,
        workspace_manager: &WorkspaceManager,
    ) -> Vec<SpaceRenderElements<R, WaylandSurfaceRenderElement<R>>> {
        let space = &workspace_manager.current_workspace().space;
        let output = output_manager.current_output();

        match space.render_elements_for_output(renderer, output, 1.0) {
            Ok(r) => r,
            Err(err) => {
                warn!("Failed to get windows render elements: {:?}", err);
                return vec![];
            }
        }
    }

    pub fn get_cursor_render_elements<R: NuonuoRenderer>(
        &self,
        renderer: &mut R,
        output_manager: &OutputManager,
        cursor_manager: &mut CursorManager,
        input_manager: &InputManager,
    ) -> Vec<CustomRenderElements<R>> {
        cursor_manager.check_cursor_image_surface_alive();

        let output = output_manager.current_output();
        let output_scale = output.current_scale();

        let output_geo = match output_manager.output_geometry(&output) {
            Some(g) => g,
            None => {
                warn!("Failed to get output {:?} geometry", output);
                return vec![];
            }
        };
        let output_pos = output_geo.loc;

        let pointer = input_manager.get_pointer();
        let pointer = match pointer {
            Some(k) => k,
            None => {
                return vec![];
            }
        };

        let pointer_pos = pointer.current_location();
        let pointer_pos = pointer_pos - output_pos.to_f64();

        let cursor_scale = output_scale.integer_scale();
        let render_cursor = cursor_manager.get_render_cursor(cursor_scale);

        let output_scale = Scale::from(output_scale.fractional_scale());

        let pointer_render_elements: Vec<CustomRenderElements<R>> = match render_cursor {
            RenderCursor::Hidden => vec![],
            RenderCursor::Surface { hotspot, surface } => {
                let real_pointer_pos =
                    (pointer_pos - hotspot.to_f64()).to_physical_precise_round(output_scale);

                render_elements_from_surface_tree(
                    renderer,
                    &surface,
                    real_pointer_pos,
                    output_scale,
                    1.0,
                    Kind::Cursor,
                )
            }
            RenderCursor::Named {
                icon,
                scale,
                cursor,
            } => {
                let (idx, frame) = cursor.frame(self.start_time.elapsed().as_millis() as u32);
                let hotspot = XCursor::hotspot(frame).to_logical(scale);
                let pointer_pos =
                    (pointer_pos - hotspot.to_f64()).to_physical_precise_round(output_scale);

                let texture = cursor_manager
                    .cursor_texture_cache
                    .get(icon, scale, &cursor, idx);
                let mut pointer_elements = vec![];
                let pointer_element = match MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    pointer_pos,
                    &texture,
                    None,
                    None,
                    None,
                    Kind::Cursor,
                ) {
                    Ok(element) => Some(element),
                    Err(err) => {
                        warn!("error importing a cursor texture: {err:?}");
                        None
                    }
                };
                if let Some(element) = pointer_element {
                    pointer_elements.push(CustomRenderElements::NamedPointer(element));
                }
                pointer_elements
            }
        };
        pointer_render_elements
    }

    pub fn get_border_render_elements<R: NuonuoRenderer>(
        &self,
        renderer: &mut R,
        workspace_manager: &WorkspaceManager,
    ) -> Vec<CustomRenderElements<R>> {
        let mut elements: Vec<CustomRenderElements<R>> = vec![];

        let focus = workspace_manager.get_focus();
        if let Some(window) = focus {
            let window_geo = match window.get_rec() {
                Some(g) => g,
                None => {
                    warn!("Failed to get window {:?} geometry", window);
                    return vec![];
                }
            };

            let program = renderer.as_gles_renderer()
                .egl_context()
                .user_data()
                .get::<BorderShader>()
                .unwrap()
                .0
                .clone();

            let point = window_geo.size.to_point();

            // Colors are 24 bits with 8 bits for each red, green, blue value.
            // To get each color, shift the bits over by the offset and zero
            // out the other colors. The bitwise AND 255 does this because it will
            // zero out everything but the last 8 bits. This is where the color
            // has been shifted to.

            let border_color: Color32F = Color32F::from([0.0, 0.0, 1.0, 1.0]);
            let border_thickness = 5.0;

            elements.push(CustomRenderElements::Border(
                BorderRenderElement::new(
                    program,
                    window_geo,
                    None,
                    1.0,
                    vec![
                        Uniform::new("u_resolution", (point.x as f32, point.y as f32)),
                        Uniform::new("border_color", (border_color.r(), border_color.g(), border_color.b())), 
                        Uniform::new("border_thickness", border_thickness),
                    ],
                    Kind::Unspecified,
                )
            ));
        }

        elements
    }
    
    pub fn get_background_render_elements<R: NuonuoRenderer>(
        &self,
        renderer: &mut R,
        output_manager: &OutputManager,
    ) -> Vec<CustomRenderElements<R>> {
        let mut elements: Vec<CustomRenderElements<R>> = vec![];

        let program = renderer.as_gles_renderer()
            .egl_context()
            .user_data()
            .get::<Background>()
            .unwrap()
            .0
            .clone();

        let output_geo = output_manager.output_geometry(output_manager.current_output()).unwrap();
        let point = output_geo.size.to_point();

        elements.push(CustomRenderElements::Background(
            BackgroundRenderElement::new(
                program,
                output_geo,
                None,
                1.0,
                vec![
                    Uniform::new("u_resolution", (point.x as f32, point.y as f32)),
                    Uniform::new("u_time", self.start_time.elapsed().as_secs_f32()),
                ],
                Kind::Unspecified,
            )
        ));
    
        elements
    }

}
