use std::{cell::RefCell, collections::HashMap, env, fs::File, io::Read, rc::Rc};

use anyhow::{Context, Ok, anyhow};
use smithay::{
    backend::renderer::element::memory::MemoryRenderBuffer,
    input::pointer::{CursorIcon, CursorImageStatus, CursorImageSurfaceData},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{IsAlive, Logical, Physical, Point, Transform},
    wayland::compositor::with_states,
};

use xcursor::{
    CursorTheme,
    parser::{Image, parse_xcursor},
};

use smithay::backend::allocator::Fourcc;

static FALLBACK_CURSOR_DATA: &[u8] = include_bytes!("../../resource/cursor.rgba");

type XCursorCache = HashMap<(CursorIcon, i32), Option<Rc<XCursor>>>;

pub struct XCursor {
    // The image for the underlying named cursor.
    images: Vec<Image>,
    // The total duration of the animation.
    animation_duration: u32,
}

impl XCursor {
    /// Given a time, calculate which frame to show, and how much time remains until the next frame.
    ///
    /// Time will wrap, so if for instance the cursor has an animation lasting 100ms,
    /// then calling this function with 5ms and 105ms as input gives the same output.
    pub fn frame(&self, mut millis: u32) -> (usize, &Image) {
        // static icon.
        if self.animation_duration == 0 {
            return (0, &self.images[0]);
        }

        millis %= self.animation_duration;

        let mut res = 0;
        for (i, image) in self.images.iter().enumerate() {
            if millis < image.delay {
                res = i;
                break;
            }
            millis -= image.delay;
        }

        (res, &self.images[res])
    }

    pub fn get_frames(&self) -> &[Image] {
        &self.images
    }

    pub fn _is_animated_cursor(&self) -> bool {
        self.images.len() > 1
    }

    pub fn hotspot(image: &Image) -> Point<i32, Physical> {
        (image.xhot as i32, image.yhot as i32).into()
    }
}

pub struct CursorManager {
    pub theme: CursorTheme,
    pub size: u8,
    pub current_cursor: CursorImageStatus,
    pub named_cursor_cache: RefCell<XCursorCache>,
    pub cursor_texture_cache: CursorTextureCache,
}

impl CursorManager {
    /// Set the common XCURSOR env variables.
    fn ensure_env(theme: &str, size: u8) {
        unsafe { env::set_var("XCURSOR_THEME", theme) };
        unsafe { env::set_var("XCURSOR_SIZE", size.to_string()) };
    }

    pub fn new(theme: &str, size: u8) -> Self {
        Self::ensure_env(theme, size);
        let theme = CursorTheme::load(theme);

        Self {
            theme,
            size,
            current_cursor: CursorImageStatus::default_named(),
            named_cursor_cache: Default::default(),
            cursor_texture_cache: Default::default(),
        }
    }

    pub fn _reload(&mut self, theme: &str, size: u8) {
        Self::ensure_env(theme, size);
        self.theme = CursorTheme::load(theme);
        self.size = size;
        self.named_cursor_cache.get_mut().clear();
    }

    // if surface cursor is not alive, clean it and back to default
    pub fn check_cursor_image_surface_alive(&mut self) {
        if let CursorImageStatus::Surface(surface) = &self.current_cursor {
            if !surface.alive() {
                self.current_cursor = CursorImageStatus::default_named();
            }
        }
    }

    /// Load the cursor with the given `name` from the file system picking the closest
    /// one to the given `size`.
    fn load_xcursor(theme: &CursorTheme, name: &str, size: i32) -> anyhow::Result<XCursor> {
        let _span = tracy_client::span!("load_xcursor");
        let path = theme
            .load_icon(name)
            .ok_or_else(|| anyhow!("no default icon"))?;

        let mut file = File::open(path).context("error opening cursor icon file")?;
        let mut buf = vec![];
        file.read_to_end(&mut buf)
            .context("error reading cursor icon file")?;

        let mut images = parse_xcursor(&buf).context("error parsing cursor icon file")?;

        let (width, height) = images
            .iter()
            .min_by_key(|image| (size - image.size as i32).abs())
            .map(|image| (image.width, image.height))
            .unwrap();

        images.retain(move |image| image.width == width && image.height == height);

        let animation_duration = images.iter().fold(0, |acc, image| acc + image.delay);

        Ok(XCursor {
            images,
            animation_duration,
        })
    }

    fn fallback_cursor() -> XCursor {
        let images = vec![Image {
            size: 32,
            width: 64,
            height: 64,
            xhot: 1,
            yhot: 1,
            delay: 0,
            pixels_rgba: Vec::from(FALLBACK_CURSOR_DATA),
            pixels_argb: vec![],
        }];

        XCursor {
            images,
            animation_duration: 0,
        }
    }

    pub fn get_cursor_with_name(&self, icon: CursorIcon, scale: i32) -> Option<Rc<XCursor>> {
        self.named_cursor_cache
            .borrow_mut()
            .entry((icon, scale))
            .or_insert_with_key(|(icon, scale)| {
                let size = self.size as i32 * scale;
                let mut cursor = Self::load_xcursor(&self.theme, icon.name(), size);

                // Check alternative names to account for non-compliant themes
                if cursor.is_err() {
                    for name in icon.alt_names() {
                        cursor = Self::load_xcursor(&self.theme, name, size);
                        if cursor.is_ok() {
                            break;
                        }
                    }
                }

                if let Err(err) = &cursor {
                    warn!("error loading xcursor {}@{size}: {err:?}", icon.name());
                }

                // The default cursor must always have a fallback
                if *icon == CursorIcon::Default && cursor.is_err() {
                    cursor = Ok(Self::fallback_cursor());
                }

                cursor.ok().map(Rc::new)
            })
            .clone()
    }

    pub fn get_default_cursor(&self, scale: i32) -> Rc<XCursor> {
        // The default cursor always has a fallback
        self.get_cursor_with_name(CursorIcon::Default, scale)
            .unwrap()
    }

    pub fn get_render_cursor(&self, scale: i32) -> RenderCursor {
        match self.current_cursor.clone() {
            CursorImageStatus::Hidden => RenderCursor::Hidden,
            CursorImageStatus::Surface(surface) => {
                let hotspot = with_states(&surface, |states| {
                    states
                        .data_map
                        .get::<CursorImageSurfaceData>()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .hotspot
                });
                RenderCursor::Surface { hotspot, surface }
            }
            CursorImageStatus::Named(icon) => self
                .get_cursor_with_name(icon, scale)
                .map(|cursor| RenderCursor::Named {
                    icon,
                    scale,
                    cursor,
                })
                .unwrap_or_else(|| RenderCursor::Named {
                    icon: Default::default(),
                    scale,
                    cursor: self.get_default_cursor(scale),
                }),
        }
    }

    pub fn _is_current_cursor_animated(&self, scale: i32) -> bool {
        match &self.current_cursor {
            CursorImageStatus::Hidden => false,
            CursorImageStatus::Surface(_) => false,
            CursorImageStatus::Named(icon) => self
                .get_cursor_with_name(*icon, scale)
                .unwrap_or_else(|| self.get_default_cursor(scale))
                ._is_animated_cursor(),
        }
    }

    pub fn _cursor_image(&self, _scale: i32) -> &CursorImageStatus {
        &self.current_cursor
    }

    pub fn set_cursor_image(&mut self, cursor: CursorImageStatus) {
        self.current_cursor = cursor;
    }
}

/// The cursor prepared for renderer.
pub enum RenderCursor {
    Hidden,
    Surface {
        hotspot: Point<i32, Logical>,
        surface: WlSurface,
    },
    Named {
        icon: CursorIcon,
        scale: i32,
        cursor: Rc<XCursor>,
    },
}

type TextureCache = HashMap<(CursorIcon, i32), Vec<MemoryRenderBuffer>>;

#[derive(Default)]
pub struct CursorTextureCache {
    cache: RefCell<TextureCache>,
}

impl CursorTextureCache {
    pub fn _clear(&mut self) {
        self.cache.get_mut().clear();
    }

    pub fn get(
        &self,
        icon: CursorIcon,
        scale: i32,
        cursor: &XCursor,
        idx: usize,
    ) -> MemoryRenderBuffer {
        self.cache
            .borrow_mut()
            .entry((icon, scale))
            .or_insert_with(|| {
                cursor
                    .get_frames()
                    .iter()
                    .map(|frame| {
                        MemoryRenderBuffer::from_slice(
                            &frame.pixels_rgba,
                            Fourcc::Argb8888,
                            (frame.width as i32, frame.height as i32),
                            scale,
                            Transform::Normal,
                            None,
                        )
                    })
                    .collect()
            })[idx]
            .clone()
    }
}
