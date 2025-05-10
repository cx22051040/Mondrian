use smithay::{
    backend::renderer::{
        element::{solid::SolidColorRenderElement, memory::MemoryRenderBufferRenderElement, surface::WaylandSurfaceRenderElement, RenderElement},
        gles::{element::PixelShaderElement, GlesRenderer}, ImportAll, ImportMem, Renderer,
    },
    desktop::space::SpaceRenderElements,
    
    render_elements,
};

render_elements! {
    pub CustomRenderElements<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    NamedPointer=MemoryRenderBufferRenderElement<R>,
    // Border=PixelShaderElement,
}

impl <R: Renderer + ImportAll + ImportMem> std::fmt::Debug for CustomRenderElements<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(arg0) => f.debug_tuple("Surface").field(arg0).finish(),
            Self::NamedPointer(arg0) => f.debug_tuple("NamedPointer").field(arg0).finish(),
            // Self::Border(arg0) => f.debug_tuple("Border").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}

render_elements! {
    pub OutputRenderElements<R, E> where R: ImportAll + ImportMem;
    Space=SpaceRenderElements<R, E>,
    Custom=CustomRenderElements<R>,
}

impl <R: Renderer + ImportAll + ImportMem, E: RenderElement<R> + std::fmt::Debug> std::fmt::Debug for OutputRenderElements<R, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Space(arg0) => f.debug_tuple("Space").field(arg0).finish(),
            Self::Custom(arg0) => f.debug_tuple("Custom").field(arg0).finish(),
            // Self::Border(arg0) => f.debug_tuple("Border").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}

render_elements!(
    pub WindowRenderElement<R> where R: ImportAll + ImportMem;
    Window=WaylandSurfaceRenderElement<R>,
    Decoration=SolidColorRenderElement,
);

impl<R: Renderer> std::fmt::Debug for WindowRenderElement<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Window(arg0) => f.debug_tuple("Window").field(arg0).finish(),
            Self::Decoration(arg0) => f.debug_tuple("Decoration").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}