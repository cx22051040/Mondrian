use smithay::{
    backend::renderer::{
        element::{
            memory::MemoryRenderBufferRenderElement, 
            surface::WaylandSurfaceRenderElement
        },
        gles::element::PixelShaderElement,
    },
    desktop::space::SpaceRenderElements,
};

use crate::niri_render_elements;

use super::border::BorderRenderElement;

niri_render_elements! {
    CustomRenderElements<R> => {
        Surface=WaylandSurfaceRenderElement<R>,
        NamedPointer=MemoryRenderBufferRenderElement<R>,
        Border=BorderRenderElement,
    }
}

niri_render_elements! {
    OutputRenderElements<R> => {
        Space=SpaceRenderElements<R, WaylandSurfaceRenderElement<R>>,
        Custom=CustomRenderElements<R>,
    }
}

