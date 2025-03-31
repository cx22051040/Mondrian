use smithay::{backend::renderer::{element::{memory::MemoryRenderBufferRenderElement, surface::WaylandSurfaceRenderElement}, ImportAll, ImportMem, Renderer}, render_elements};

render_elements! {
    pub CustomRenderElements<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    NamedPointer=MemoryRenderBufferRenderElement<R>,
}

impl<R: Renderer + ImportAll + ImportMem> std::fmt::Debug
    for CustomRenderElements<R>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(arg0) => f.debug_tuple("Surface").field(arg0).finish(),
            Self::NamedPointer(arg0) => f.debug_tuple("NamedPointer").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}

