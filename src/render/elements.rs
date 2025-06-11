use smithay::{
    backend::renderer::element::{
        memory::MemoryRenderBufferRenderElement, surface::WaylandSurfaceRenderElement,
    },
    desktop::space::SpaceRenderElements,
};

use crate::niri_render_elements;

use super::{background::BackgroundRenderElement, border::BorderRenderElement};

niri_render_elements! {
    ShaderRenderElement => {
        Border=BorderRenderElement,
        Background=BackgroundRenderElement,
    }
}

niri_render_elements! {
    CustomRenderElements<R> => {
        Surface=WaylandSurfaceRenderElement<R>,
        NamedPointer=MemoryRenderBufferRenderElement<R>,
        Shader=ShaderRenderElement,
    }
}

niri_render_elements! {
    OutputRenderElements<R> => {
        Space=SpaceRenderElements<R, WaylandSurfaceRenderElement<R>>,
        Custom=CustomRenderElements<R>,
    }
}
