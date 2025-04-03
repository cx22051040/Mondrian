use border::BorderShader;
use cursor::{RenderCursor, XCursor};
use elements::CustomRenderElements;
use smithay::{
    backend::renderer::element::{
        memory::MemoryRenderBufferRenderElement, surface::render_elements_from_surface_tree, Kind
    }, 
    utils::Scale
};

use crate::state::NuonuoState;

pub mod cursor;
pub mod elements;
pub mod renders;
pub mod border;

impl NuonuoState {
  pub fn get_cursor_render_elements(&mut self) -> Vec<CustomRenderElements> {
    self.cursor_manager.check_cursor_image_surface_alive();

    let output_scale = self.output_manager.current_output().current_scale();
    let output_pos = self.workspace_manager.current_workspace().space.output_geometry(self.output_manager.current_output()).unwrap().loc;

    let pointer_pos = self.seat.get_pointer().unwrap().current_location();
    let pointer_pos = pointer_pos - output_pos.to_f64();

    let cursor_scale = output_scale.integer_scale();
    let render_cursor = self.cursor_manager.get_render_cursor(cursor_scale);

    let output_scale = Scale::from(output_scale.fractional_scale());

    let pointer_render_elements: Vec<CustomRenderElements> = match render_cursor {
        RenderCursor::Hidden => vec![],
        RenderCursor::Surface { hotspot, surface } => {
            let real_pointer_pos = 
            (pointer_pos - hotspot.to_f64()).to_physical_precise_round(output_scale);

            render_elements_from_surface_tree(
                self.backend_data.backend.renderer(), 
                &surface, 
                real_pointer_pos, 
                output_scale, 
                1.0, 
                Kind::Cursor,
            )
        },
        RenderCursor::Named { 
            icon, 
            scale, 
            cursor 
        } => {
            let (idx, frame) = cursor.frame(self.start_time.elapsed().as_millis() as u32);
            let hotspot = XCursor::hotspot(frame).to_logical(scale);
            let pointer_pos =
                (pointer_pos - hotspot.to_f64()).to_physical_precise_round(output_scale);

            let texture = self.cursor_texture_cache.get(icon, scale, &cursor, idx);
            let mut pointer_elements = vec![];
            let pointer_element = match MemoryRenderBufferRenderElement::from_buffer(
                self.backend_data.backend.renderer(),
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

 
  pub fn get_border_render_elements(&mut self) -> Vec<CustomRenderElements> {
    let mut elements: Vec<CustomRenderElements> = vec![]; 

    // TODO: only for focus window
    // let focus = self.seat.get_keyboard().unwrap().current_focus();

    for window in self.workspace_manager.current_workspace().elements() {
        
        let geometry = self.workspace_manager.current_workspace().element_geometry(window);
        elements.push(
            CustomRenderElements::Border(
                BorderShader::element(
                    &self.backend_data.backend.renderer(), 
                    geometry, 
                    1.0
                )
            )
        );
    }
    elements
  }

}