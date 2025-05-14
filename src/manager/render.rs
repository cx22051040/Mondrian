use std::time::Instant;

use smithay::{
    backend::renderer::{
        ExportMem, ImportAll, ImportMem, ImportMemWl, Renderer, RendererSuper, Texture,
        element::{
            Kind,
            memory::MemoryRenderBufferRenderElement,
            surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
        },
        gles::GlesRenderer,
    },
    desktop::space::SpaceRenderElements,
    utils::Scale,
};

use crate::{
    backend::tty::TtyRenderer,
    render::{
        border::BorderRenderElement,
        cursor::{CursorManager, RenderCursor, XCursor},
        elements::{CustomRenderElements, OutputRenderElements},
    },
};

use super::{
    input::InputManager, output::OutputManager, window::WindowExt, workspace::WorkspaceManager,
};

/// Trait with our main renderer requirements to save on the typing.
pub trait NuonuoRenderer:
    ImportAll
    + ImportMem
    + ExportMem
    + ImportMemWl
    + Renderer<TextureId = Self::NuonuoTextureId, Error = Self::NuonuoError>
    + AsGlesRenderer
{
    // Associated types to work around the instability of associated type bounds.
    type NuonuoTextureId: Texture + Clone + Send + 'static;
    type NuonuoError: std::error::Error
        + Send
        + Sync
        + From<<GlesRenderer as RendererSuper>::Error>
        + 'static;
}

impl<R> NuonuoRenderer for R
where
    R: ImportAll + ImportMem + ImportMemWl + ExportMem + AsGlesRenderer,
    R::TextureId: Texture + Clone + Send + 'static,
    R::Error:
        std::error::Error + Send + Sync + From<<GlesRenderer as RendererSuper>::Error> + 'static,
{
    type NuonuoTextureId = R::TextureId;
    type NuonuoError = R::Error;
}

/// Trait for getting the underlying `GlesRenderer`.
pub trait AsGlesRenderer {
    fn as_gles_renderer(&mut self) -> &mut GlesRenderer;
}

impl AsGlesRenderer for GlesRenderer {
    fn as_gles_renderer(&mut self) -> &mut GlesRenderer {
        self
    }
}

impl AsGlesRenderer for TtyRenderer<'_> {
    fn as_gles_renderer(&mut self) -> &mut GlesRenderer {
        self.as_mut()
    }
}

pub struct RenderManager {
    // no need now
    pub start_time: Instant,
}

impl RenderManager {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn get_render_elements<R: NuonuoRenderer>(
        &self,
        renderer: &mut R,
        output_manager: &OutputManager,
        workspace_manager: &WorkspaceManager,
        cursor_manager: &mut CursorManager,
        input_manager: &InputManager,
    ) -> Vec<OutputRenderElements<R, WaylandSurfaceRenderElement<R>>> {
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

        // Then common Windows
        output_elements.extend(
            self.get_windows_render_elements(renderer, output_manager, workspace_manager)
                .into_iter()
                .map(OutputRenderElements::Space),
        );

        // Then Shader and CustomRenderElements
        output_elements.extend(
            self.get_border_render_elements(renderer, workspace_manager)
                .into_iter()
                .map(OutputRenderElements::Custom),
        );

        // Then LayerShell Bottom and Background
        // TODO:

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
                error!("get pointer error");
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

            let a = renderer.as_gles_renderer();

            // elements.push(CustomRenderElements::Border(BorderRenderElement::element(
            //     renderer,
            //     window_geo,
            //     1.0,
            // )));
        }

        elements
    }
}
