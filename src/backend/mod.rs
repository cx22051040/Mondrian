pub mod winit;
pub mod tty;

use smithay::reexports::calloop::LoopHandle;

use tty::Tty;
use winit::Winit;

use crate::{manager::output::OutputManager, state::GlobalData};

pub enum Backend {
    Tty(Tty),
    Winit(Winit),
}

impl Backend {
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

    pub fn init(&mut self, output_manager: &mut OutputManager, loop_handle: &LoopHandle<'_, GlobalData>) {
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