use std::time::Duration;

#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;

use smithay::{
    backend::{
        egl::EGLDevice,
        renderer::{
            ImportDma,
            damage::OutputDamageTracker,
            element::{
                Kind, memory::MemoryRenderBufferRenderElement,
                surface::render_elements_from_surface_tree,
            },
            gles::GlesRenderer,
        },
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{calloop::LoopHandle, wayland_server::DisplayHandle},
    utils::{Physical, Point, Rectangle, Scale, Transform},
    wayland::dmabuf::{DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufState},
};

use crate::{
    CalloopData,
    input::input::process_input_event,
    render::{
        cursor::{RenderCursor, XCursor},
        elements::PointerRenderElement,
        renders::render_output,
    },
    state::NuonuoState,
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
    loop_handle: &LoopHandle<'_, CalloopData>,
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
        .insert_source(winit, move |event, _, calloop_data| {
            let state = &mut calloop_data.state;
            let display_handle = &mut state.display_handle;
            let output = &state.backend_data.output;

            match event {
                WinitEvent::Resized { size, .. } => {
                    output.change_current_state(
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
                    process_input_event(event, calloop_data);
                }
                WinitEvent::Redraw => {
                    let backend = &mut state.backend_data.backend;
                    let size = backend.window_size();
                    let damage = Rectangle::from_size(size);
                    let mut damage_tracker = OutputDamageTracker::from_output(&output);
                    let age = backend.buffer_age().unwrap_or(0);

                    {
                        let (renderer, mut framebuffer) = backend.bind().unwrap();

                        let mut elements = Vec::<PointerRenderElement<GlesRenderer>>::new();
                        
                        // add cursor render elements.
                        {
                            state.cursor_manager.check_cursor_image_surface_alive();

                            let output_scale = output.current_scale();
                            let output_pos = state.space.output_geometry(output).unwrap().loc;

                            let pointer_pos = state.seat.get_pointer().unwrap().current_location();
                            let pointer_pos = pointer_pos - output_pos.to_f64();

                            let cursor_scale = output_scale.integer_scale();
                            let render_cursor = state.cursor_manager.get_render_cursor(cursor_scale);

                            let output_scale = Scale::from(output.current_scale().fractional_scale());
      
                            elements.extend(match render_cursor {
                                RenderCursor::Hidden => vec![],
                                RenderCursor::Surface { hotspot, surface } => {
                                    // Get the real surface location.
                                    let real_pointer_pos: Point<i32, Physical> = (pointer_pos
                                        - hotspot.to_f64())
                                    .to_physical_precise_round(output_scale);
                                    let render_elements: Vec<PointerRenderElement<GlesRenderer>> =
                                        render_elements_from_surface_tree(
                                            renderer,
                                            &surface,
                                            real_pointer_pos,
                                            output_scale,
                                            1.0,
                                            Kind::Cursor,
                                        );
                                    render_elements
                                }
                                RenderCursor::Named {
                                    icon,
                                    scale,
                                    cursor,
                                } => {
                                    let (idx, frame) =
                                        cursor.frame(state.start_time.elapsed().as_millis() as u32);

                                    let hotspot = XCursor::hotspot(frame).to_logical(scale);
                                    let real_pointer_pos: Point<i32, Physical> = (pointer_pos
                                        - hotspot.to_f64())
                                    .to_physical_precise_round(output_scale);

                                    let texture =
                                        state.cursor_texture_cache.get(icon, scale, &cursor, idx);

                                    let elements: Vec<PointerRenderElement<GlesRenderer>> = vec![
                                        PointerRenderElement::<GlesRenderer>::from(
                                            MemoryRenderBufferRenderElement::from_buffer(
                                                renderer,
                                                real_pointer_pos.to_f64(),
                                                &texture,
                                                None,
                                                None,
                                                None,
                                                Kind::Cursor,
                                            )
                                            .expect("Lost system pointer buffer"),
                                        )
                                        .into(),
                                    ];
                                    elements
                                }
                            });
                        } 

                        render_output(
                            &output,
                            &state.space,
                            elements,
                            renderer,
                            &mut framebuffer,
                            &mut damage_tracker,
                            age,
                        )
                        .unwrap();
                    }

                    backend.submit(Some(&[damage])).unwrap();

                    // For each of the windows send the frame callbacks to tell them to draw next frame.
                    state
                        .space
                        .elements()
                        .for_each(|window: &smithay::desktop::Window| {
                            window.send_frame(
                                &output,
                                state.start_time.elapsed(),
                                Some(Duration::ZERO),
                                |_, _| Some(output.clone()),
                            )
                        });

                    // Refresh space state and handle certain events like enter/leave for outputs/windows
                    state.space.refresh();
                    state.popups.cleanup();
                    // Flush the outgoing buffers caontaining events so the clients get them.
                    let _ = display_handle.flush_clients();

                    // Ask for redraw to schedule new frame.
                    backend.window().request_redraw();
                }
                WinitEvent::CloseRequested => {}
                _ => (),
            };
        })
        .unwrap();

    backend_data
}
