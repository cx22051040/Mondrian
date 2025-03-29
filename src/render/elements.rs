use smithay::{
    backend::renderer::{
        ImportAll, ImportMem, Renderer,
        element::{
            RenderElement, memory::MemoryRenderBufferRenderElement,
            surface::WaylandSurfaceRenderElement,
        },
    },
    desktop::space::SpaceRenderElements,
    render_elements,
};

// This macro combines the two possible elements into one, a WaylandSurfaceRenderElement which
// is provided by the client, or the TextureRenderElement which is the default cursor.
render_elements! {
    pub PointerRenderElement<R> where
      R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    Memory=MemoryRenderBufferRenderElement<R>,
}

impl<R: Renderer> std::fmt::Debug for PointerRenderElement<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(arg0) => f.debug_tuple("Surface").field(arg0).finish(),
            Self::Memory(arg0) => f.debug_tuple("Memory").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}

render_elements! {
    pub OutputRenderElements<R, E> where R: ImportAll + ImportMem;
    Space = SpaceRenderElements<R, E>,
    Pointer = PointerRenderElement<R>,
}

impl<R: Renderer + ImportAll + ImportMem, E: RenderElement<R> + std::fmt::Debug> std::fmt::Debug
    for OutputRenderElements<R, E>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Space(arg0) => f.debug_tuple("Space").field(arg0).finish(),
            Self::Pointer(argo0) => f.debug_tuple("Pointer").field(argo0).finish(),
            Self::_GenericCatcher(argo0) => f.debug_tuple("_GenericCatcher").field(argo0).finish(),
        }
    }
}
