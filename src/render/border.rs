use smithay::{
    backend::renderer::{
        element::{
            Element, Id, Kind, RenderElement, UnderlyingStorage
        }, gles::{GlesError, GlesFrame, GlesRenderer}, utils::{CommitCounter, OpaqueRegions}, ImportAll, Renderer
    }, 
    utils::{
        Buffer, Logical, Physical, Rectangle, Scale, Size
    }
};

#[derive(Debug)]
pub struct BorderRenderElement<R: Renderer> {
    id: Id,
    commit_counter: CommitCounter,
    rec: Rectangle<f64, Logical>,
    opaque_regions: Vec<Rectangle<f64, Logical>>,
    alpha: f32,
    kind: Kind,
    texture: R
}

impl<R: Renderer> BorderRenderElement<R> {

}

impl<R: Renderer + ImportAll> Element for BorderRenderElement<R> {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit_counter
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        Rectangle::from_size(Size::from((1., 1.)))
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.rec.to_physical_precise_round(scale)
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        self
            .opaque_regions
            .iter()
            .map(|region| region.to_physical_precise_down(scale))
            .collect()
    }

    fn alpha(&self) -> f32 {
        self.alpha
    }

    fn kind(&self) -> Kind {
        self.kind
    }
}

impl<R: Renderer + ImportAll> RenderElement<R> for BorderRenderElement<R> {
    fn draw(
            &self,
            frame: &mut R::Frame<'_, '_>,
            src: Rectangle<f64, Buffer>,
            dst: Rectangle<i32, Physical>,
            damage: &[Rectangle<i32, Physical>],
            opaque_regions: &[Rectangle<i32, Physical>],
        ) -> Result<(), R::Error> {
            todo!()
    }

    fn underlying_storage(&self, _renderer: &mut R) -> Option<UnderlyingStorage> {
        // If scanout for things other than Wayland buffers is implemented, this will need to take
        // the target GPU into account.
        None
    }
}