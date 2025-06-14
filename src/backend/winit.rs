use std::time::Duration;

#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;

use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        egl::EGLDevice,
        renderer::{Color32F, ImportDma, damage::OutputDamageTracker, gles::GlesRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode as OutputMode, Subpixel},
    reexports::{calloop::LoopHandle, wayland_server::DisplayHandle},
    utils::{Rectangle, Scale, Transform},
    wayland::dmabuf::DmabufFeedbackBuilder,
};

use crate::{
    manager::{
        cursor::CursorManager, input::InputManager, output::OutputManager, render::RenderManager,
        workspace::WorkspaceManager,
    },
    state::{GlobalData, State},
};

#[derive(Debug)]
pub struct Winit {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
}
impl Winit {
    pub fn new(loop_handle: &LoopHandle<'_, GlobalData>) -> anyhow::Result<Self> {
        let (backend, winit) = winit::init::<GlesRenderer>()
            .map_err(|e| anyhow::anyhow!("Failed to initialize Winit backend: {}", e))?;

        loop_handle
            .insert_source(winit, move |event, _, data| {
                match event {
                    WinitEvent::Resized { size, .. } => {
                        data.output_manager.change_current_state(
                            Some(OutputMode {
                                size,
                                refresh: 60_000,
                            }),
                            None,
                            None,
                            None,
                        );
                        let scale = data.output_manager.current_output().current_scale();
                        let scale = Scale::from(scale.integer_scale());

                        data.workspace_manager.modify_windows(
                            Rectangle::from_size(size.to_logical(scale)),
                            &data.loop_handle,
                        );
                    }
                    WinitEvent::Input(event) => {
                        data.process_input_event(event);
                    }
                    WinitEvent::Redraw => {
                        let size = data.backend.winit().backend.window_size();
                        let damage = Rectangle::from_size(size);

                        let damage_traker = &mut OutputDamageTracker::from_output(
                            data.output_manager.current_output(),
                        );
                        data.backend.winit().render_output(
                            damage_traker,
                            &mut data.render_manager,
                            &data.output_manager,
                            &data.workspace_manager,
                            &mut data.cursor_manager,
                            &data.input_manager,
                        );

                        match data.backend.winit().backend.submit(Some(&[damage])) {
                            Ok(_) => {}
                            Err(err) => {
                                warn!("Winit: Failed to submit frame: {:?}", err);
                            }
                        }

                        // For each of the windows send the frame callbacks to tell them to draw next frame.
                        data.workspace_manager.elements().for_each(|window| {
                            window.send_frame(
                                data.output_manager.current_output(),
                                data.start_time.elapsed(),
                                Some(Duration::ZERO),
                                |_, _| Some(data.output_manager.current_output().clone()),
                            )
                        });

                        // Refresh space nuonuo_state and handle certain events like enter/leave for outputs/windows
                        data.workspace_manager.refresh();
                        data.popups.cleanup();
                        // Flush the outgoing buffers caontaining events so the clients get them.
                        let _ = data.display_handle.flush_clients();

                        // Ask for redraw to schedule new frame.
                        data.backend.winit().backend.window().request_redraw();
                    }
                    WinitEvent::CloseRequested => {}
                    _ => (),
                };
            })
            .unwrap();

        Ok(Self { backend })
    }

    pub fn init(
        &mut self,
        display_handle: &DisplayHandle,
        output_manager: &mut OutputManager,
        render_manager: &RenderManager,
        state: &mut State,
    ) {
        // add output
        output_manager.add_output(
            "winit".to_string(),
            (0, 0).into(),
            Subpixel::Unknown,
            "Smithay".into(),
            "Winit".into(),
            (0, 0).into(),
            true,
            display_handle,
        );

        let mode = OutputMode {
            size: self.backend.window_size(),
            refresh: 60_000,
        };

        output_manager.change_current_state(
            Some(mode),
            Some(Transform::Flipped180),
            None,
            Some((0, 0).into()),
        );
        output_manager.set_preferred(mode);

        // initial dmabuf
        #[cfg(feature = "egl")]
        if self.get_renderer().bind_wl_display(display_handle).is_ok() {
            tracing::info!("EGL hardware-acceleration enabled");
        };

        let render_node =
            EGLDevice::device_for_display(self.get_renderer().egl_context().display())
                .and_then(|device| device.try_get_render_node());

        let dmabuf_default_feedback = match render_node {
            Ok(Some(node)) => {
                let dmabuf_format = self.get_renderer().dmabuf_formats();
                let dmabuf_default_feedback =
                    DmabufFeedbackBuilder::new(node.dev_id(), dmabuf_format)
                        .build()
                        .unwrap();
                Some(dmabuf_default_feedback)
            }
            Ok(None) => {
                warn!("Failed to query render node, dmabuf will use v3");
                None
            }
            Err(err) => {
                warn!(?err, "Failed to egl device for display, dmabuf will use v3");
                None
            }
        };

        if let Some(default_feedback) = dmabuf_default_feedback {
            let _dmabuf_global = state
                .dmabuf_state
                .create_global_with_default_feedback::<GlobalData>(
                    display_handle,
                    &default_feedback,
                );
        } else {
            let dmabuf_formats = self.get_renderer().dmabuf_formats();
            let _dmabuf_global = state
                .dmabuf_state
                .create_global::<GlobalData>(display_handle, dmabuf_formats);
        };

        // compile shaders
        render_manager.compile_shaders(self.get_renderer());
    }

    pub fn render_output(
        &mut self,
        damage_tracker: &mut OutputDamageTracker,
        render_manager: &mut RenderManager,
        output_manager: &OutputManager,
        workspace_manager: &WorkspaceManager,
        cursor_manager: &mut CursorManager,
        input_manager: &InputManager,
    ) {
        let _span = tracy_client::span!("winit_render");

        if let Ok((renderer, mut framebuffer)) = self.backend.bind() {
            let elements = render_manager.get_render_elements(
                renderer,
                output_manager,
                workspace_manager,
                cursor_manager,
                input_manager,
            );

            let _ = damage_tracker.render_output(
                renderer,
                &mut framebuffer,
                0,
                &elements,
                Color32F::from([0.0; 4]),
            );
        } else {
            warn!("Winit: Failed to get renderer & framebuffer");
            return;
        }
    }

    fn get_renderer(&mut self) -> &mut GlesRenderer {
        self.backend.renderer()
    }

    pub fn dmabuf_imported(&mut self, dmabuf: &Dmabuf) -> bool {
        match self.get_renderer().import_dmabuf(dmabuf, None) {
            Ok(_) => true,
            Err(err) => {
                warn!("error importing dmabuf: {:?}", err);
                false
            }
        }
    }
}
