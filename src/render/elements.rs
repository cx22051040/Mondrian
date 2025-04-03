use smithay::{
    backend::renderer::
    {element::
        {
            memory::MemoryRenderBufferRenderElement, surface::WaylandSurfaceRenderElement
        }, 
        gles::{element::PixelShaderElement, GlesRenderer}
    }, 
    render_elements
};

render_elements! {
    pub CustomRenderElements<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    NamedPointer=MemoryRenderBufferRenderElement<GlesRenderer>,
    Border=PixelShaderElement,
}

impl std::fmt::Debug for CustomRenderElements
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(arg0) => f.debug_tuple("Surface").field(arg0).finish(),
            Self::NamedPointer(arg0) => f.debug_tuple("NamedPointer").field(arg0).finish(),
            Self::Border(arg0) => f.debug_tuple("Border").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}

