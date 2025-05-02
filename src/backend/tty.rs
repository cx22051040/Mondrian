use std::{collections::{HashMap, HashSet}, path::{self, Path}};
use smithay::{backend::{allocator::format::FormatSet, drm::{output::{DrmOutput, DrmOutputRenderElements}, DrmSurface}, renderer::{multigpu::MultiRenderer, DebugFlags, ImportDma}}, reexports::{drm::control::Device as _, wayland_protocols::wp::linux_dmabuf::zv1::server::zwp_linux_dmabuf_feedback_v1, wayland_server::backend::GlobalId}, wayland::dmabuf::{DmabufFeedback, DmabufFeedbackBuilder}};
use smithay::reexports::drm::Device;
use smithay::{backend::drm::output::DrmOutputManager, desktop::utils::OutputPresentationFeedback};
use smithay::{output::{Output, PhysicalProperties}, reexports::drm::control::ModeTypeFlags};
use anyhow::Context;
use smithay::{
    backend::{
        allocator::{gbm::{GbmAllocator, GbmBufferFlags, GbmDevice}, Fourcc}, 
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, DrmEventMetadata, DrmNode, NodeType}, 
        egl::{context::ContextPriority, EGLDevice, EGLDisplay}, input::InputEvent, 
        libinput::{LibinputInputBackend, LibinputSessionInterface}, 
        renderer::{gles::GlesRenderer, multigpu::{gbm::GbmGlesBackend, GpuManager}}, 
        session::{libseat::{self, LibSeatSession}, Event as SessionEvent, Session}, 
        udev::{self, UdevBackend, UdevEvent}
    }, 
    reexports::{
        calloop::{LoopHandle, RegistrationToken}, drm::control::{connector, crtc}, input::Libinput, rustix::fs::OFlags, wayland_server::DisplayHandle
    },
    output::Mode as WlMode,
    utils::DeviceFd, wayland::drm_lease::DrmLease
};
use smithay_drm_extras::{display_info, drm_scanner::{DrmScanEvent, DrmScanner}};


use crate::{render::elements::CustomRenderElements, space::output::OutputManager, state::NuonuoState};

// we cannot simply pick the first supported format of the intersection of *all* formats, because:
// - we do not want something like Abgr4444, which looses color information, if something better is available
// - some formats might perform terribly
// - we might need some work-arounds, if one supports modifiers, but the other does not
//
// So lets just pick `ARGB2101010` (10-bit) or `ARGB8888` (8-bit) for now, they are widely supported.
const SUPPORTED_FORMATS: &[Fourcc] = &[
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];
const SUPPORTED_FORMATS_8BIT_ONLY: &[Fourcc] = &[Fourcc::Abgr8888, Fourcc::Argb8888];

pub type TtyRenderer<'render> = MultiRenderer<
    'render,
    'render,
    GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
    GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
>;

pub struct Tty {
    session: LibSeatSession,
    libinput: Libinput,
    gpu_manager: GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    primary_node: DrmNode,
    primary_render_node: DrmNode,
    devices: HashMap<DrmNode, OutputDevice>,
    seat_name: String,
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
    dmabuf_feedback: Option<SurfaceDmabufFeedback>,
}

pub struct OutputDevice {
    registration_token: RegistrationToken,
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
    pub fn new (
        loop_handle: &LoopHandle<'_, NuonuoState>,
    ) -> anyhow::Result<Self> {
        let (session, notifier) = LibSeatSession::new()?;
        
        let seat_name = session.seat();
    
        info!("Seat name: {}", seat_name);
    
        let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
            session.clone().into(),
        );
        libinput.udev_assign_seat(&seat_name).unwrap();
        let libinput_backend = LibinputInputBackend::new(libinput.clone());
    
        loop_handle
            .insert_source(libinput_backend, |mut event, _, state| {
                if let InputEvent::DeviceAdded { device } = &mut event {
                    info!("Device added: {:?}", device);
                } else if let InputEvent::DeviceRemoved { ref device } = event {
                    info!("Device removed: {:?}", device);
                }
                state.process_input_event(event);
            }).unwrap();

        loop_handle
            .insert_source(notifier, move |event, _, state| {
                match event {
                    SessionEvent::ActivateSession=> {
                        info!("Session activated");
                    },
                    SessionEvent::PauseSession=> {
                        info!("Session paused");
                    }
                }
            }).unwrap();

        let api = GbmGlesBackend::with_context_priority(ContextPriority::High);
        let gpu_manager = GpuManager::new(api).context("error creating the GPU manager")?;
    
        let primary_gpu_path = udev::primary_gpu(&seat_name)
            .context("error getting the primary GPU")?
            .context("couldn't find a GPU")?;
        
        let primary_node = DrmNode::from_path(primary_gpu_path)
            .context("error opening the primary GPU DRM node")?;
        
        let primary_render_node = primary_node
            .node_with_type(NodeType::Render)
            .and_then(Result::ok)
            .unwrap_or_else(|| {
                warn!(
                    "error getting the render node for the primary GPU; proceeding anyway"
                );
                primary_node
            });

        let node_path = if let Some(path) = primary_render_node.dev_path() {
            format!("{:?}", path)
        } else {
            format!("{}", primary_render_node)
        };
        info!("using as the render node: {}", node_path);

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

    pub fn init(&mut self, output_manager: &mut OutputManager, loop_handle: &LoopHandle<'_, NuonuoState>) {
        let udev_backend = UdevBackend::new(&self.seat_name).unwrap();

        for (device_id, path) in udev_backend.device_list() {
            if let Ok(node) = DrmNode::from_dev_id(device_id) {
                if let Err(err) = self.device_added(node, &path, output_manager, loop_handle) {
                    warn!("erro adding device: {:?}", err);
                }
            }
        }

        loop_handle
            .insert_source(udev_backend, move |event, _, state| match event {
                UdevEvent::Added { device_id, path } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        if let Err(err) = state.backend.tty().device_added(node, &path, &mut state.output_manager, &state.loop_handle) {
                            warn!("erro adding device: {:?}", err);
                        }
                    }
                }
                UdevEvent::Changed { device_id } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        state.backend.tty().device_changed(node, &mut state.output_manager)
                    }
                }
                UdevEvent::Removed { device_id } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        state.backend.tty().device_removed(node)
                    }
                }
            })
            .unwrap();


    }

    pub fn device_added (
        &mut self, 
        node: DrmNode, 
        path: &Path, 
        output_manager: &mut OutputManager,
        loop_handle: &LoopHandle<'_, NuonuoState>
    ) -> anyhow::Result<()> {
        let open_flags = OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK;
        let fd = self.session.open(path, open_flags)?;
        let device_fd = DrmDeviceFd::new(DeviceFd::from(fd));

        let (drm, drm_notifier) = DrmDevice::new(device_fd.clone(), true)?;
        let gbm = GbmDevice::new(device_fd)?;

        let registration_token = loop_handle
            .insert_source(drm_notifier, move |event, meta, state| {
                let tty = state.backend.tty();
                match event {
                    DrmEvent::VBlank(crtc) => {
                        let meta = meta.expect("VBlank events must have metadata");
                        tty.on_vblank(node, crtc, meta);
                    }
                    DrmEvent::Error(error) => warn!("DRM error: {error}"),
                };
            })
            .unwrap();

        let display = unsafe{ EGLDisplay::new(gbm.clone())? };
        let egl_device = EGLDevice::device_for_display(&display)?;

        let render_node = egl_device
            .try_get_render_node()?
            .context("no render node")?;

        self.gpu_manager
            .as_mut()
            .add_node(render_node, gbm.clone())
            .context("error adding render node to GPU manager")?;

        let allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
        let color_formats = SUPPORTED_FORMATS;

        let mut renderer = self.gpu_manager.single_renderer(&render_node).unwrap();
        let render_formats = renderer.as_mut().egl_context().dmabuf_render_formats().clone();

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
                registration_token,
                drm_scanner: DrmScanner::new(),
                non_desktop_connectors: HashSet::new(),
                render_node,
                drm_output_manager,
                surfaces: HashMap::new(),
                active_leases: Vec::new(),
            }
        );

        self.device_changed(node, output_manager);
        info!("device added: {:?}", node);
        Ok(())

    }

    pub fn device_changed (&mut self, node: DrmNode, output_manager: &mut OutputManager) {
        let device: &mut OutputDevice = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            warn!("not change because of unknown device");
            return
        };

        let scan_result = match device
            .drm_scanner
            .scan_connectors(device.drm_output_manager.device()) {
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
                    self.connector_connected(node, connector, crtc, output_manager);
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
        info!("device changed: {:?}", node);
    }

    pub fn device_removed (&mut self, node: DrmNode) {

    }

    pub fn on_vblank(
        &mut self, 
        node: DrmNode,
        crtc: crtc::Handle,
        meta: DrmEventMetadata,
    ) {
        error!("not finished");
        todo!()
    }

    pub fn connector_connected (&mut self, node: DrmNode, connector: connector::Info, crtc: crtc::Handle, output_manager: &mut OutputManager) {
        let device = if let Some(device) = self.devices.get_mut(&node) {
            device
        } else {
            return;
        };

        let mut renderer = self
            .gpu_manager
            .single_renderer(&device.render_node)
            .unwrap();

        let output_name = format!("{}-{}", connector.interface().as_str(), connector.interface_id());
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

        let display_info = display_info::for_connector(drm_device, connector.handle());

        let make = display_info
            .as_ref()
            .and_then(|info| info.make())
            .unwrap_or_else(|| "Unknown".into());

        let model = display_info
            .as_ref()
            .and_then(|info| info.model())
            .unwrap_or_else(|| "Unknown".into());

        if non_desktop {
            info!("Connector {} is non-desktop", output_name);
            device.non_desktop_connectors.insert((connector.handle(), crtc));
            // TODO: lease the connector for non-desktop connectors
        } else {

            let (phys_w, phys_h) = connector.size().unwrap_or((0, 0));
            output_manager.add_output(
                output_name, 
                (phys_w as i32, phys_h as i32).into(), 
                connector.subpixel().into(), 
                make, 
                model, 
                true
            );

            let mode_id = connector
                .modes()
                .iter()
                .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
                .unwrap_or(0);

            let drm_mode = connector.modes()[mode_id];
            let wl_mode = WlMode::from(drm_mode);

            output_manager.change_current_state(
                Some(wl_mode), 
                None, 
                None, 
                Some((0, 0).into()) // TODO: multiple outputs
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
            if driver.name().to_string_lossy().to_lowercase().contains("nvidia")
                || driver
                    .description()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains("nvidia")
            {
                planes.overlay = vec![];
            }

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
                    warn!("Failed to initialize drm output: {}", err);
                    return;
                }
            };

            let dmabuf_feedback = drm_output.with_compositor(|compositor| {
                compositor
                    .set_debug_flags(DebugFlags::empty());
                get_surface_dmabuf_feedback(
                    self.primary_node,
                    device.render_node,
                    &mut self.gpu_manager,
                    compositor.surface(),
                )
            });

            let surface = Surface {
                device_id: node,
                render_node: device.render_node,
                drm_output,
                dmabuf_feedback,
            };
            
            device.surfaces.insert(crtc, surface);
        }
        info!("connector connected: {:?}", connector);
    }


}

#[derive(Debug, Clone)]
pub struct SurfaceDmabufFeedback {
    pub render_feedback: DmabufFeedback,
    pub scanout_feedback: DmabufFeedback,
}

fn get_surface_dmabuf_feedback(
    primary_gpu: DrmNode,
    render_node: DrmNode,
    gpus: &mut GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    surface: &DrmSurface,
) -> Option<SurfaceDmabufFeedback> {
    let primary_formats = gpus.single_renderer(&primary_gpu).ok()?.dmabuf_formats();
    let render_formats = gpus.single_renderer(&render_node).ok()?.dmabuf_formats();

    let all_render_formats = primary_formats
        .iter()
        .chain(render_formats.iter())
        .copied()
        .collect::<FormatSet>();

    let planes = surface.planes().clone();

    // We limit the scan-out tranche to formats we can also render from
    // so that there is always a fallback render path available in case
    // the supplied buffer can not be scanned out directly
    let planes_formats = surface
        .plane_info()
        .formats
        .iter()
        .copied()
        .chain(planes.overlay.into_iter().flat_map(|p| p.formats))
        .collect::<FormatSet>()
        .intersection(&all_render_formats)
        .copied()
        .collect::<FormatSet>();

    let builder = DmabufFeedbackBuilder::new(primary_gpu.dev_id(), primary_formats);
    let render_feedback = builder
        .clone()
        .add_preference_tranche(render_node.dev_id(), None, render_formats.clone())
        .build()
        .unwrap();

    let scanout_feedback = builder
        .add_preference_tranche(
            surface.device_fd().dev_id().unwrap(),
            Some(zwp_linux_dmabuf_feedback_v1::TrancheFlags::Scanout),
            planes_formats,
        )
        .add_preference_tranche(render_node.dev_id(), None, render_formats)
        .build()
        .unwrap();

    Some(SurfaceDmabufFeedback {
        render_feedback,
        scanout_feedback,
    })
}
