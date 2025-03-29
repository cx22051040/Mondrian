use smithay::{
    backend::renderer::{
        ImportAll, ImportMem, Renderer,
        damage::{Error as OutputDamageTrackerError, OutputDamageTracker, RenderOutputResult},
        element::surface::WaylandSurfaceRenderElement,
    },
    desktop::{Space, Window},
    output::Output,
};

use super::elements::{OutputRenderElements, PointerRenderElement};

pub fn render_output<'d, R>(
    output: &Output,
    space: &Space<Window>,
    elements: Vec<PointerRenderElement<R>>,
    renderer: &mut R,
    framebuffer: &mut R::Framebuffer<'_>,
    damage_tracker: &'d mut OutputDamageTracker,
    age: usize,
) -> Result<RenderOutputResult<'d>, OutputDamageTrackerError<R::Error>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + 'static,
{
    let mut output_render_elements: Vec<OutputRenderElements<R, WaylandSurfaceRenderElement<R>>> =
        elements
            .into_iter()
            .map(OutputRenderElements::from)
            .collect();

    let space_elements = smithay::desktop::space::space_render_elements::<_, Window, _>(
        renderer,
        [space],
        output,
        1.0,
    )
    .expect("output without mode?");

    output_render_elements.extend(space_elements.into_iter().map(OutputRenderElements::Space));

    damage_tracker.render_output(
        renderer,
        framebuffer,
        age,
        &output_render_elements,
        [0.0, 0.0, 1.0, 1.0],
    )
}

