pub mod winit;
#[cfg(feature = "tty")]
pub mod tty;

use smithay::{backend::{allocator::dmabuf::Dmabuf, renderer::{ImportAll, ImportDma as _, ImportMem, Renderer}}, output::Output, reexports::calloop::LoopHandle, wayland::{dmabuf::{DmabufState, ImportNotifier}, shm::ShmState}};
#[cfg(feature = "tty")]
use tty::Tty;
use winit::Winit;

use crate::{render::elements::CustomRenderElements, space::{output::OutputManager, workspace::Workspace}, state::NuonuoState};

pub enum Backend {
    #[cfg(feature = "tty")]
    Tty(Tty),
    Winit(Winit),
}

impl Backend {
    #[cfg(feature = "tty")]
    pub fn tty(&mut self) -> &mut Tty {
        if let Self::Tty(v) = self {
            v
        } else {
            panic!("backend is not Tty");
        }
    }

    pub fn winit(&mut self) -> &mut Winit {
        if let Self::Winit(v) = self {
            v
        } else {
            panic!("backend is not Winit");
        }
    }

    pub fn init(&mut self, output_manager: &mut OutputManager, loop_handle: &LoopHandle<'_, NuonuoState>) {
        if let Self::Winit(v) = self {
            v.init(output_manager);
        } else if let Self::Tty(v) = self {
            v.init(output_manager, loop_handle);
        } else {
            panic!("backend is not Winit");
        }
    }

    pub fn seat_name(&self) -> String {
        if let Self::Winit(_) = self {
            String::from("winit")
        } else if let Self::Tty(v) = self {
            v.seat_name.clone()
        } else {
            panic!("backend is not Winit");
        }
    }

    // pub fn render_output<R>(&mut self, output: &Output, workspace: &Workspace, custom_elements: Vec<CustomRenderElements<R>>) 
    //     where
    //         R: Renderer + ImportAll + ImportMem,
    //         R::TextureId: Clone + 'static,
    // {
    //     if let Self::Winit(v) = self {
    //         v.render_output(output, workspace, custom_elements);
    //     } else if let Self::Tty(v) = self {
    //         v.render_output(output, workspace, custom_elements);
    //     }
    //     else {
    //         panic!("backend is not Winit");
    //     }
    // }
    
}