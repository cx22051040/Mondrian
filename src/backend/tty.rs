use anyhow::Context;
use smithay::backend::renderer::element::RenderElement;
use smithay::reexports::drm::Device;
use smithay::{
    backend::drm::output::DrmOutputManager,
    desktop::utils::{
        OutputPresentationFeedback, surface_presentation_feedback_flags_from_states,
        surface_primary_scanout_output, update_surface_primary_scanout_output,
    },
};
use smithay::{
    output::Mode as WlMode,
    backend::{
        SwapBuffersError,
        drm::{
            DrmAccessError, DrmError, DrmSurface,
            compositor::{DrmCompositor, FrameFlags},
            output::{DrmOutput, DrmOutputRenderElements},
            DrmDevice, DrmDeviceFd, DrmEvent, DrmEventMetadata, DrmNode, NodeType
        },
        renderer::{
            Color32F, DebugFlags, ImportDma,
            damage::Error as OutputDamageTrackerError,
            element::{RenderElementStates, default_primary_scanout_output_compare},
            gles::GlesRenderer,
            multigpu::{GpuManager, MultiRenderer, gbm::GbmGlesBackend},
        },
        allocator::{
            Fourcc,
            format::FormatSet,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        egl::{EGLDevice, EGLDisplay, context::ContextPriority},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{self, UdevBackend, UdevEvent},
    },
    desktop::{Space, Window},
    reexports::{
        calloop::{LoopHandle, RegistrationToken, timer::{TimeoutAction, Timer}},
        drm::control::{connector, crtc, Device as _},
        gbm::Modifier,
        wayland_protocols::wp::{
            linux_dmabuf::zv1::server::zwp_linux_dmabuf_feedback_v1::{self, TrancheFlags},
            presentation_time::server::wp_presentation_feedback,
        },
        rustix::fs::OFlags,
        input::Libinput,
    },
    utils::{Clock, Monotonic, Time, DeviceFd},
    wayland::{
        dmabuf::{DmabufFeedback, DmabufFeedbackBuilder},
        presentation::Refresh,
        drm_lease::DrmLease
    },
};
use smithay::{output::Output, reexports::drm::control::ModeTypeFlags};
use smithay_drm_extras::{
    display_info,
    drm_scanner::{DrmScanEvent, DrmScanner},
};
use std::{
    collections::{HashMap, HashSet},
    io,
    path::Path,
    time::Duration,
};

use crate::render::elements::OutputRenderElements;
use crate::state::GlobalData;
use crate::{
    render::elements::CustomRenderElements,
    manager::{output::OutputManager, workspace::WorkspaceManager},
    state::State,
};

// we cannot simply pick the first supported format of the intersection of *all* formats, because:
// - we do not want something like Abgr4444, which looses color information, if something better is available
// - some formats might perform terribly
// - we might need some work-arounds, if one supports modifiers, but the other does not
//
// So lets just pick `ARGB2101010` (10-bit) or `ARGB8888` (8-bit) for now, they are widely supported.
const SUPPORTED_COLOR_FORMATS: [Fourcc; 4] = [
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];

pub type TtyRenderer<'render> = MultiRenderer<
    'render,
    'render,
    GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
    GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
>;

pub struct Tty {
    pub session: LibSeatSession,
    pub libinput: Libinput,
    pub gpu_manager: GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    pub primary_node: DrmNode,
    pub primary_render_node: DrmNode,
    pub devices: HashMap<DrmNode, OutputDevice>,
    pub seat_name: String,
}

pub struct Surface {
    device_id: DrmNode,
    render_node: DrmNode,
    drm_output: DrmOutput<
        GbmAllocator<DrmDeviceFd>,
        GbmDevice<DrmDeviceFd>,
        Option<OutputPresentationFeedback>,
        DrmDeviceFd,
    >,
}

pub struct OutputDevice {
    render_node: DrmNode,
    drm_scanner: DrmScanner,
    surfaces: HashMap<crtc::Handle, Surface>,
    active_leases: Vec<DrmLease>,
    drm_output_manager: DrmOutputManager<
        GbmAllocator<DrmDeviceFd>,
        GbmDevice<DrmDeviceFd>,
        Option<OutputPresentationFeedback>,
        DrmDeviceFd,
    >,

    // record non_desktop connectors such as VR headsets
    // we need to handle them differently
    non_desktop_connectors: HashSet<(connector::Handle, crtc::Handle)>,
}

impl Tty {
    pub fn new(loop_handle: &LoopHandle<'_, GlobalData>) -> anyhow::Result<Self> {

        // Initialize session
        let (session, notifier) = LibSeatSession::new()?;
        let seat_name = session.seat();

        let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
            session.clone().into(),
        );
        libinput.udev_assign_seat(&seat_name).unwrap();
        let libinput_backend = LibinputInputBackend::new(libinput.clone());

        loop_handle
            .insert_source(libinput_backend, |mut event, _, data| {
                if let InputEvent::DeviceAdded { device } = &mut event {
                    info!("libinput Device added: {:?}", device);
                } else if let InputEvent::DeviceRemoved { ref device } = event {
                    info!("libinput Device removed: {:?}", device);
                }
                data.process_input_event(event);
            })
            .unwrap();

        loop_handle
            .insert_source(notifier, move |event, _, _| match event {
                SessionEvent::ActivateSession => {
                    info!("Session activated");
                }
                SessionEvent::PauseSession => {
                    info!("Session paused");
                }
            })
            .unwrap();

        // Initialize Gpu manager
        let api = GbmGlesBackend::with_context_priority(ContextPriority::Medium);
        let gpu_manager = GpuManager::new(api).context("error creating the GPU manager")?;

        let primary_gpu_path = udev::primary_gpu(&seat_name)
            .context("error getting the primary GPU")?
            .context("couldn't find a GPU")?;

        info!("using as the primary node: {:?}", primary_gpu_path);

        let primary_node = DrmNode::from_path(primary_gpu_path)
            .context("error opening the primary GPU DRM node")?;

        info!("Primary GPU: {:?}", primary_node);

        // get render node if exit - /renderD128
        let primary_render_node = primary_node
            .node_with_type(NodeType::Render)
            .and_then(Result::ok)
            .unwrap_or_else(|| {
                warn!("error getting the render node for the primary GPU; proceeding anyway");
                primary_node
            });

        let primary_render_node_path = if let Some(path) = primary_render_node.dev_path() {
            format!("{:?}", path)
        } else {
            format!("{}", primary_render_node)
        };
        info!("using as the render node: {}", primary_render_node_path);

        Ok(Self {
            session,
            libinput,
            gpu_manager,
            primary_node,
            primary_render_node,
            devices: HashMap::new(),
            seat_name,
        })
    }

    pub fn init(
        &mut self,
        output_manager: &mut OutputManager,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        let udev_backend = UdevBackend::new(&self.seat_name).unwrap();

        for (device_id, path) in udev_backend.device_list() {
            if let Ok(node) = DrmNode::from_dev_id(device_id) {
                if let Err(err) = self.device_added(node, &path, output_manager, loop_handle) {
                    warn!("erro adding device: {:?}", err);
                }
            }
        }

        loop_handle
            .insert_source(udev_backend, move |event, _, data| match event {
                UdevEvent::Added { device_id, path } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        if let Err(err) = data.backend.tty().device_added(
                            node,
                            &path,
                            &mut data.output_manager,
                            &data.loop_handle,
                        ) {
                            warn!("erro adding device: {:?}", err);
                        }
                    }
                }
                UdevEvent::Changed { device_id } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        data.backend.tty().device_changed(
                            node,
                            &mut data.output_manager,
                            &data.loop_handle,
                        )
                    }
                }
                UdevEvent::Removed { device_id } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        data.backend.tty().device_removed(node)
                    }
                }
            })
            .unwrap();
    }

    pub fn device_added(
        &mut self,
        node: DrmNode,
        path: &Path,
        output_manager: &mut OutputManager,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) -> anyhow::Result<()> {
        info!("device added: {:?}", node);
        let fd = self.session.open(path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)?;
        let device_fd = DrmDeviceFd::new(DeviceFd::from(fd));

        let (drm, drm_notifier) = DrmDevice::new(device_fd.clone(), true)?;
        let gbm = GbmDevice::new(device_fd)?;

        loop_handle
            .insert_source(drm_notifier, move |event, meta, data| {
                match event {
                    DrmEvent::VBlank(crtc) => {
                        let meta = meta.expect("VBlank events must have metadata");
                        data.backend.tty().on_vblank(
                            node,
                            crtc,
                            meta,
                            data.output_manager.current_output(),
                            &data.clock,
                            &data.loop_handle,
                        );
                    }
                    DrmEvent::Error(error) => warn!("DRM Vblank error: {error}"),
                };
        })
        .unwrap();
    
        let egl_display = unsafe { EGLDisplay::new(gbm.clone())? };
        let egl_device = EGLDevice::device_for_display(&egl_display)?;

        // get render_node, if not, using node
        let render_node = egl_device.try_get_render_node()?.unwrap_or(node);

        self.gpu_manager
            .as_mut()
            .add_node(render_node, gbm.clone())
            .context("error adding render node to GPU manager")?;

        let allocator = GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );
        let color_formats = SUPPORTED_COLOR_FORMATS;

        let mut renderer = self.gpu_manager.single_renderer(&render_node).unwrap();
        let render_formats = renderer
            .as_mut()
            .egl_context()
            .dmabuf_render_formats()
            .clone();

        let drm_output_manager = DrmOutputManager::new(
            drm,
            allocator,
            gbm.clone(),
            Some(gbm),
            color_formats.iter().copied(),
            render_formats,
        );

        self.devices.insert(
            node,
            OutputDevice {
                drm_scanner: DrmScanner::new(),
                non_desktop_connectors: HashSet::new(),
                render_node,
                drm_output_manager,
                surfaces: HashMap::new(),
                active_leases: Vec::new(),
            },
        );

        self.device_changed(node, output_manager, loop_handle);

        Ok(())
    }

    pub fn device_changed(
        &mut self,
        node: DrmNode,
        output_manager: &mut OutputManager,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        info!("device changed: {:?}", node);
        let device: &mut OutputDevice = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            warn!("not change because of unknown device");
            return;
        };

        let scan_result = match device
            .drm_scanner
            .scan_connectors(device.drm_output_manager.device())
        {
            Ok(x) => x,
            Err(err) => {
                warn!("error scanning connectors: {err:?}");
                return;
            }
        };

        for event in scan_result {
            match event {
                DrmScanEvent::Connected {
                    connector,
                    crtc: Some(crtc),
                } => {
                    self.connector_connected(node, connector, crtc, output_manager, loop_handle);
                }
                DrmScanEvent::Disconnected {
                    connector,
                    crtc: Some(crtc),
                } => {
                    // self.connector_disconnected(node, connector, crtc);
                }
                _ => {}
            }
        }
    }

    pub fn device_removed(&mut self, node: DrmNode) {}

    pub fn on_vblank(
        &mut self,
        node: DrmNode,
        crtc: crtc::Handle,
        meta: DrmEventMetadata,
        output: &Output,
        clock: &Clock<Monotonic>,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        let device = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            error!("Trying to finish frame on non-existent backend");
            return;
        };

        let surface = if let Some(surface) = device.surfaces.get_mut(&crtc) {
            surface
        } else {
            error!("Trying to finish frame on non-existent backend surface");
            return;
        };

        let tp = match meta.time {
            smithay::backend::drm::DrmEventTime::Monotonic(tp) => Some(tp),
            smithay::backend::drm::DrmEventTime::Realtime(_) => None,
        };

        let seq = meta.sequence;

        let (clock, flags) = if let Some(tp) = tp {
            (
                tp.into(),
                wp_presentation_feedback::Kind::Vsync
                    | wp_presentation_feedback::Kind::HwClock
                    | wp_presentation_feedback::Kind::HwCompletion,
            )
        } else {
            (clock.now(), wp_presentation_feedback::Kind::Vsync)
        };

        let submit_result = surface
            .drm_output
            .frame_submitted()
            .map_err(Into::<SwapBuffersError>::into);

        let Some(frame_duration) = output
            .current_mode()
            .map(|mode| Duration::from_secs_f64(1_000f64 / mode.refresh as f64))
        else {
            return;
        };

        let schedule_render = match submit_result {
            Ok(user_data) => {
                if let Some(mut feedback) = user_data.flatten() {
                    feedback.presented(clock, Refresh::fixed(frame_duration), seq as u64, flags);
                }

                true
            }
            Err(err) => {
                warn!("Error during rendering: {:?}", err);
                match err {
                    SwapBuffersError::AlreadySwapped => true,
                    // If the device has been deactivated do not reschedule, this will be done
                    // by session resume
                    SwapBuffersError::TemporaryFailure(err)
                        if matches!(
                            err.downcast_ref::<DrmError>(),
                            Some(&DrmError::DeviceInactive)
                        ) =>
                    {
                        false
                    }
                    SwapBuffersError::TemporaryFailure(err) => matches!(
                        err.downcast_ref::<DrmError>(),
                        Some(DrmError::Access(DrmAccessError {
                            source,
                            ..
                        })) if source.kind() == io::ErrorKind::PermissionDenied
                    ),
                    SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
                }
            }
        };

        if schedule_render {
            let next_frame_target = clock + frame_duration;
            let repaint_delay = Duration::from_secs_f64(frame_duration.as_secs_f64() * 0.6f64);

            let timer = if self.primary_node != surface.render_node {
                trace!("scheduling repaint timer immediately on {:?}", crtc);
                Timer::immediate()
            } else {
                trace!(
                    "scheduling repaint timer with delay {:?} on {:?}",
                    repaint_delay, crtc
                );
                Timer::from_duration(repaint_delay)
            };

            loop_handle
                .insert_source(timer, move |_, _, data| {
                    data.backend.tty().render(
                        node,
                        Some(crtc),
                        &data.workspace_manager.current_workspace().space,
                        &data.output_manager.current_output(),
                        next_frame_target,
                        &data.loop_handle,
                        &data.clock,
                    );
                    TimeoutAction::Drop
                })
                .expect("failed to schedule frame timer");
        }
    }

    pub fn connector_connected(
        &mut self,
        node: DrmNode,
        connector: connector::Info,
        crtc: crtc::Handle,
        output_manager: &mut OutputManager,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) {
        info!("connector connected: {:?}", connector);

        let device = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            return;
        };

        let output_name = format!(
            "{}-{}",
            connector.interface().as_str(),
            connector.interface_id()
        );
        info!(?crtc, "Trying to setup connector {}", output_name);

        let drm_device = device.drm_output_manager.device();
        let non_desktop = drm_device
            .get_properties(connector.handle())
            .ok()
            .and_then(|props| {
                let (info, value) = props
                    .into_iter()
                    .filter_map(|(handle, value)| {
                        let info = drm_device.get_property(handle).ok()?;

                        Some((info, value))
                    })
                    .find(|(info, _)| info.name().to_str() == Ok("non-desktop"))?;

                info.value_type().convert_value(value).as_boolean()
            })
            .unwrap_or(false);

        if non_desktop {
            info!("Connector {} is non-desktop", output_name);
            device
                .non_desktop_connectors
                .insert((connector.handle(), crtc));
            // TODO: lease the connector for non-desktop connectors
        } else {
            let display_info = display_info::for_connector(drm_device, connector.handle());

            let make = display_info
                .as_ref()
                .and_then(|info| info.make())
                .unwrap_or_else(|| "Unknown".into());

            let model = display_info
                .as_ref()
                .and_then(|info| info.model())
                .unwrap_or_else(|| "Unknown".into());

            let mode_id = connector
                .modes()
                .iter()
                .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                .unwrap_or(0);

            let drm_mode = connector.modes()[mode_id];
            let wl_mode = WlMode::from(drm_mode);

            let (phys_w, phys_h) = connector.size().unwrap_or((0, 0));
            info!("Connector {} size: {}x{}", output_name, phys_w, phys_h);
            output_manager.add_output(
                output_name,
                (phys_w as i32, phys_h as i32).into(),
                connector.subpixel().into(),
                make,
                model,
                (0, 0).into(),
                true,
            );

            output_manager.change_current_state(
                Some(wl_mode),
                None,
                None,
                Some((0, 0).into()), // TODO: multiple outputs
            );
            output_manager.set_preferred(wl_mode);

            let driver = match drm_device.get_driver() {
                Ok(driver) => driver,
                Err(err) => {
                    warn!("error getting driver: {:?}", err);
                    return;
                }
            };

            let mut planes = match drm_device.planes(&crtc) {
                Ok(planes) => planes,
                Err(err) => {
                    warn!("error getting planes: {:?}", err);
                    return;
                }
            };

            // Using an overlay plane on a nvidia card breaks
            if driver
                .name()
                .to_string_lossy()
                .to_lowercase()
                .contains("nvidia")
                || driver
                    .description()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains("nvidia")
            {
                info!("Nvidia driver detected, disabling overlay planes");
                planes.overlay = vec![];
            }        
            
            let mut renderer = self
                .gpu_manager
                .single_renderer(&device.render_node)
                .unwrap();

            let drm_output = match device
                .drm_output_manager
                .initialize_output::<_, CustomRenderElements<TtyRenderer>>(
                    crtc,
                    drm_mode,
                    &[connector.handle()],
                    output_manager.current_output(),
                    Some(planes),
                    &mut renderer,
                    &DrmOutputRenderElements::default(),
                ) {
                Ok(drm_output) => drm_output,
                Err(err) => {
                    warn!("error initializing output: {:?}", err);
                    return;
                }
            };

            let surface = Surface {
                device_id: node,
                render_node: device.render_node,
                drm_output,
            };

            device.surfaces.insert(crtc, surface);

            loop_handle.insert_idle(move |data| {
                data.backend.tty().render_surface(
                    node,
                    crtc,
                    &data.workspace_manager.current_workspace().space,
                    &data.output_manager.current_output(),
                    data.clock.now(),
                    &data.loop_handle,
                    &data.clock,
                );
            });
        }
    }

    // If crtc is `Some()`, render it, else render all crtcs
    pub fn render(
        &mut self,
        node: DrmNode,
        crtc: Option<crtc::Handle>,
        space: &Space<Window>,
        output: &Output,
        frame_target: Time<Monotonic>,
        loop_handle: &LoopHandle<'_, GlobalData>,
        clock: &Clock<Monotonic>,
    ) {
        let device = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            return;
        };

        if let Some(crtc) = crtc {
            self.render_surface(
                node,
                crtc,
                space,
                output,
                frame_target,
                loop_handle,
                clock,
            );
        } else {
            let crtcs: Vec<_> = device.surfaces.keys().copied().collect();
            for crtc in crtcs {
                self.render_surface(
                    node,
                    crtc,
                    space,
                    output,
                    frame_target,
                    loop_handle,
                    clock,
                );
            }
        };
    }

    pub fn render_surface(
        &mut self,
        node: DrmNode,
        crtc: crtc::Handle,
        space: &Space<Window>,
        output: &Output,
        frame_target: Time<Monotonic>,
        loop_handle: &LoopHandle<'_, GlobalData>,
        clock: &Clock<Monotonic>,
    ) {
        info!("rendering surface: {:?}", crtc);
        let device = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            return;
        };

        let surface = if let Some(surface) = device.surfaces.get_mut(&crtc) {
            surface
        } else {
            return;
        };

        let mut renderer = self
            .gpu_manager
            .single_renderer(&surface.render_node)
            .unwrap();

        let elements = space
            .render_elements_for_output(&mut renderer, output, 1.0)
            .unwrap();

        info!("elements: {:?}", elements);

        let (rendered, states) = surface
            .drm_output
            .render_frame(
                &mut renderer,
                &elements,
                Color32F::new(1.0, 1.0, 0.0, 1.0),
                FrameFlags::DEFAULT,
            )
            .map(|render_frame_result| (!render_frame_result.is_empty, render_frame_result.states))
            .map_err(|err| match err {
                smithay::backend::drm::compositor::RenderFrameError::PrepareFrame(err) => {
                    SwapBuffersError::from(err)
                }
                smithay::backend::drm::compositor::RenderFrameError::RenderFrame(
                    OutputDamageTrackerError::Rendering(err),
                ) => SwapBuffersError::from(err),
                _ => unreachable!(),
            })
            .unwrap();

        update_primary_scanout_output(
            space,
            output,
            &states,
        );
        info!("rendered: {:?}", rendered);

        if !self.session.is_active() {
            error!("Session not active");
        }

        if rendered {
            let output_presentation_feedback = take_presentation_feedback(
                output,
                space,
                &states,
            );
            surface
                .drm_output
                .queue_frame(Some(output_presentation_feedback))
                .map_err(Into::<SwapBuffersError>::into)
                .unwrap();
        }

        if !rendered {
            let next_frame_target = frame_target + Duration::from_millis(1000 / 60);
            let reschedule_timeout =
                Duration::from(next_frame_target).saturating_sub(clock.now().into());
            let timer = Timer::from_duration(reschedule_timeout);
            loop_handle
                .insert_source(timer, move |_, _, state| {
                    state.backend.tty().render(
                        node,
                        Some(crtc),
                        &state.workspace_manager.current_workspace().space,
                        &state.output_manager.current_output(),
                        next_frame_target,
                        &state.loop_handle,
                        &state.clock,
                    );
                    TimeoutAction::Drop
                })
                .expect("failed to schedule timer");
        }

    }
}

pub fn take_presentation_feedback(
    output: &Output,
    space: &Space<Window>,
    render_element_states: &RenderElementStates,
) -> OutputPresentationFeedback {
    let mut output_presentation_feedback = OutputPresentationFeedback::new(output);

    space.elements().for_each(|window| {
        if space.outputs_for_element(window).contains(output) {
            window.take_presentation_feedback(
                &mut output_presentation_feedback,
                surface_primary_scanout_output,
                |surface, _| {
                    surface_presentation_feedback_flags_from_states(surface, render_element_states)
                },
            );
        }
    });

    output_presentation_feedback
}

pub fn update_primary_scanout_output(
    space: &Space<Window>,
    output: &Output,
    render_element_states: &RenderElementStates,
) {
    space.elements().for_each(|window| {
        window.with_surfaces(|surface, states| {
            update_surface_primary_scanout_output(
                surface,
                output,
                states,
                render_element_states,
                default_primary_scanout_output_compare,
            );
        });
    });
}

