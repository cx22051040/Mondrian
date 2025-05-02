use std::time::Duration;

#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;

use smithay::{
    backend::{
        renderer::gles::GlesRenderer,
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel},
    reexports::{calloop::LoopHandle, wayland_server::DisplayHandle},
    utils::{Rectangle, Scale, Transform},
};

use crate::{render::border::compile_shaders, space::output::OutputManager, NuonuoState};

#[derive(Debug)]
pub struct Winit {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
}

impl Winit {
    pub fn new(
        loop_handle: &LoopHandle<'_, NuonuoState>,
        display_handle: &DisplayHandle,
    ) -> anyhow::Result<Self> {
        let (mut backend, winit) = winit::init::<GlesRenderer>()
            .map_err(|e| anyhow::anyhow!("Winit init error: {}", e))?;

        #[cfg(feature = "egl")]
        if backend.renderer().bind_wl_display(&display_handle).is_ok() {
            tracing::info!("EGL hardware-acceleration enabled");
        };
    
        // TODO: tidy it
        compile_shaders(backend.renderer());

        loop_handle
            .insert_source(winit, move |event, _, state| {
                match event {
                    WinitEvent::Resized { size, .. } => {
                        state.output_manager.change_current_state(
                            Some(OutputMode {
                                size,
                                refresh: 60_000,
                            }),
                            None,
                            None,
                            None,
                        );
                        // TODO: Handle scale change
                        let scale = state.output_manager.current_output().current_scale();
                        let scale = Scale::from(scale.integer_scale());

                        state.workspace_manager.modify_windows(Rectangle::from_size(size.to_logical(scale)));
                    }
                    WinitEvent::Input(event) => {
                        state.process_input_event(event);
                    }
                    WinitEvent::Redraw => {
                        let size = state.backend.winit().backend.window_size();
                        let damage = Rectangle::from_size(size);

                        state.backend.winit().render_output(state.output_manager.current_output(), state.workspace_manager.current_workspace(), vec![]);
    
                        state
                            .backend
                            .winit()
                            .backend
                            .submit(Some(&[damage]))
                            .unwrap();
    
                        // For each of the windows send the frame callbacks to tell them to draw next frame.
                        state.workspace_manager.elements().for_each(
                            |window: &smithay::desktop::Window| {
                                window.send_frame(
                                    state.output_manager.current_output(),
                                    state.start_time.elapsed(),
                                    Some(Duration::ZERO),
                                    |_, _| Some(state.output_manager.current_output().clone()),
                                )
                            },
                        );
    
                        // Refresh space nuonuo_state and handle certain events like enter/leave for outputs/windows
                        state.workspace_manager.refresh();
                        state.popups.cleanup();
                        // Flush the outgoing buffers caontaining events so the clients get them.
                        let _ = state.display_handle.flush_clients();
    
                        // Ask for redraw to schedule new frame.
                        state.backend.winit().backend.window().request_redraw();
                    }
                    WinitEvent::CloseRequested => {}
                    _ => (),
                };
            })
            .unwrap();
    
        Ok(Self { 
            backend,
        }
        )
    }

    pub fn init(&self, output_manager: &mut OutputManager) {
        output_manager.add_output(
            "winit".to_string(), 
            (0, 0).into(), 
            Subpixel::Unknown, 
            "Smithay".into(), 
            "Winit".into(), 
            true
        );
        
        let mode = OutputMode {
            size: self.backend.window_size(),
            refresh: 60_000,
        };

        output_manager.change_current_state(
            Some(mode), 
            Some(Transform::Flipped180), 
            None, 
            Some((0, 0).into())
        );
        output_manager.set_preferred(mode);
    }
}
