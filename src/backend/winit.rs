use std::time::Duration;

#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;

use smithay::{
    backend::{
        egl::EGLDevice,
        renderer::{
            damage::OutputDamageTracker, gles::GlesRenderer, ImportDma
        },
        winit::{self, WinitEvent, WinitGraphicsBackend},
    }, 
    desktop::space::render_output, output::{Mode, Output, PhysicalProperties, Subpixel}, 
    reexports::{calloop::LoopHandle, wayland_server::DisplayHandle}, 
    utils::{Rectangle, Transform}, 
    wayland::dmabuf::{DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufState}
};

use crate::{
    input::input::process_input_event, render::{
        border::{compile_shaders, BorderShader}, elements::CustomRenderElements
    }, NuonuoState
};

pub const OUTPUT_NAME: &str = "winit";

#[derive(Debug)]
pub struct WinitData {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    pub damage_tracker: OutputDamageTracker,
    pub dmabuf_state: (DmabufState, DmabufGlobal, Option<DmabufFeedback>),
    pub output: Output,
}

pub fn init_winit(
    loop_handle: &LoopHandle<'_, NuonuoState>,
    display_handle: &DisplayHandle,
) -> WinitData {
    let (mut backend, winit) = winit::init::<GlesRenderer>().unwrap();

    let size = backend.window_size();

    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let output = Output::new(
        OUTPUT_NAME.to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
    );

    let _global = output.create_global::<NuonuoState>(&display_handle);
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);

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
        let damage_tracker = OutputDamageTracker::from_output(&output);

        WinitData {
            backend,
            damage_tracker,
            dmabuf_state,
            output,
        }
    };

    loop_handle
        .insert_source(winit, move |event, _, nuonuo_state| {
            match event {
                WinitEvent::Resized { size, .. } => {
                    nuonuo_state.backend_data.output.change_current_state(
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
                    let mut damage_tracker = OutputDamageTracker::from_output(&nuonuo_state.backend_data.output);
                    
                    {
                        let mut custom_elements: Vec<CustomRenderElements> = vec![];

                        // add pointer elements
                        custom_elements.extend(
                            nuonuo_state.get_cursor_render_elements()
                        );

                        // add window's border
                        custom_elements.extend(
                            nuonuo_state.get_border_render_elements()
                        );

                        let (renderer, mut framebuffer) = nuonuo_state.backend_data.backend.bind().unwrap();
                        
                        render_output::<_, CustomRenderElements, _, _>(
                            &nuonuo_state.backend_data.output,
                            renderer,
                            &mut framebuffer,
                            1.0,
                            0,
                            [&nuonuo_state.space],
                            custom_elements.as_slice(),
                            &mut damage_tracker,
                            [0.0, 0.0, 1.0, 1.0],
                        )
                        .unwrap();
                    }

                    nuonuo_state.backend_data.backend.submit(Some(&[damage])).unwrap();

                    // For each of the windows send the frame callbacks to tell them to draw next frame.
                    nuonuo_state
                        .space
                        .elements()
                        .for_each(|window: &smithay::desktop::Window| {
                            window.send_frame(
                                &nuonuo_state.backend_data.output,
                                nuonuo_state.start_time.elapsed(),
                                Some(Duration::ZERO),
                                |_, _| Some(nuonuo_state.backend_data.output.clone()),
                            )
                        });

                    // Refresh space nuonuo_state and handle certain events like enter/leave for outputs/windows
                    nuonuo_state.space.refresh();
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
