use std::time::Duration;

#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;

use smithay::{
    backend::{
        egl::EGLDevice,
        renderer::{ImportDma, damage::OutputDamageTracker, gles::GlesRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::Mode,
    reexports::{calloop::LoopHandle, wayland_server::DisplayHandle},
    utils::Rectangle,
    wayland::dmabuf::{DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufState},
};

use crate::{NuonuoState, input::process_input_event, render::border::compile_shaders};

#[derive(Debug)]
pub struct WinitData {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    pub dmabuf_state: (DmabufState, DmabufGlobal, Option<DmabufFeedback>),
}

pub fn init_winit(
    loop_handle: &LoopHandle<'_, NuonuoState>,
    display_handle: &DisplayHandle,
) -> WinitData {
    let (mut backend, winit) = winit::init::<GlesRenderer>().unwrap();

    let render_node = EGLDevice::device_for_display(backend.renderer().egl_context().display())
        .and_then(|device| device.try_get_render_node());
    let dmabuf_default_feedback = match render_node {
        Ok(Some(node)) => {
            let dmabuf_formats = backend.renderer().dmabuf_formats();
            let dmabuf_default_feedback = DmabufFeedbackBuilder::new(node.dev_id(), dmabuf_formats)
                .build()
                .unwrap();
            Some(dmabuf_default_feedback)
        }
        Ok(None) => {
            warn!("failed to query render node, dmabuf will use v3");
            None
        }
        Err(err) => {
            warn!(?err, "failed to egl device for display, dmabuf will use v3");
            None
        }
    };

    let dmabuf_state = if let Some(default_feedback) = dmabuf_default_feedback {
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_global = dmabuf_state
            .create_global_with_default_feedback::<NuonuoState>(&display_handle, &default_feedback);
        (dmabuf_state, dmabuf_global, Some(default_feedback))
    } else {
        let dmabuf_formats = backend.renderer().dmabuf_formats();
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_global =
            dmabuf_state.create_global::<NuonuoState>(&display_handle, dmabuf_formats);
        (dmabuf_state, dmabuf_global, None)
    };

    #[cfg(feature = "egl")]
    if backend.renderer().bind_wl_display(&display_handle).is_ok() {
        tracing::info!("EGL hardware-acceleration enabled");
    };

    // TODO: tidy it
    compile_shaders(backend.renderer());

    let backend_data = {
        WinitData {
            backend,
            dmabuf_state,
        }
    };

    loop_handle
        .insert_source(winit, move |event, _, nuonuo_state| {
            match event {
                WinitEvent::Resized { size, .. } => {
                    nuonuo_state.output_manager.change_current_state(
                        Some(Mode {
                            size,
                            refresh: 60_000,
                        }),
                        None,
                        None,
                        None,
                    );
                }
                WinitEvent::Input(event) => {
                    process_input_event(event, nuonuo_state);
                }
                WinitEvent::Redraw => {
                    let size = nuonuo_state.backend_data.backend.window_size();
                    let damage = Rectangle::from_size(size);
                    let mut damage_tracker = OutputDamageTracker::from_output(
                        nuonuo_state.output_manager.current_output(),
                    );

                    nuonuo_state.render_output(&mut damage_tracker);

                    nuonuo_state
                        .backend_data
                        .backend
                        .submit(Some(&[damage]))
                        .unwrap();

                    // For each of the windows send the frame callbacks to tell them to draw next frame.
                    nuonuo_state.workspace_manager.elements().for_each(
                        |window: &smithay::desktop::Window| {
                            window.send_frame(
                                nuonuo_state.output_manager.current_output(),
                                nuonuo_state.start_time.elapsed(),
                                Some(Duration::ZERO),
                                |_, _| Some(nuonuo_state.output_manager.current_output().clone()),
                            )
                        },
                    );

                    // Refresh space nuonuo_state and handle certain events like enter/leave for outputs/windows
                    nuonuo_state.workspace_manager.refresh();
                    nuonuo_state.popups.cleanup();
                    // Flush the outgoing buffers caontaining events so the clients get them.
                    let _ = nuonuo_state.display_handle.flush_clients();

                    // Ask for redraw to schedule new frame.
                    nuonuo_state.backend_data.backend.window().request_redraw();
                }
                WinitEvent::CloseRequested => {}
                _ => (),
            };
        })
        .unwrap();

    backend_data
}
