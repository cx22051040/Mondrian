use smithay::backend::renderer::{gles::{GlesFrame, GlesRenderer}, ExportMem, ImportAll, ImportMem, ImportMemWl, Renderer, RendererSuper, Texture};

use crate::backend::tty::{TtyFrame, TtyRenderer};

pub mod border;
pub mod elements;
pub mod shader;
pub mod render_elements;
pub mod background;

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

/// Trait for getting the underlying `GlesFrame`.
pub trait AsGlesFrame<'frame, 'buffer>
where
    Self: 'frame,
{
    fn as_gles_frame(&mut self) -> &mut GlesFrame<'frame, 'buffer>;
}

impl<'frame, 'buffer> AsGlesFrame<'frame, 'buffer> for GlesFrame<'frame, 'buffer> {
    fn as_gles_frame(&mut self) -> &mut GlesFrame<'frame, 'buffer> {
        self
    }
}

impl<'frame, 'buffer> AsGlesFrame<'frame, 'buffer> for TtyFrame<'_, 'frame, 'buffer> {
    fn as_gles_frame(&mut self) -> &mut GlesFrame<'frame, 'buffer> {
        self.as_mut()
    }
}

