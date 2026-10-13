//! The GPU: one device, and the window's surface. Adapted from Flint's
//! `gpu.rs` (Flint `eb23547`), itself from Fenix's: a non-sRGB surface so
//! the palette's values reach the screen as written; resizes applied at
//! most once per frame; low power, since a list of text needs no discrete
//! GPU; and the software fallback adapter (WARP, llvmpipe) when no
//! hardware one works, because VMs and remote desktops without a usable
//! GPU are normal on corporate machines.

use std::sync::Arc;

use winit::window::Window;

/// Which adapter is in use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterSummary {
    /// The driver's name for it.
    pub name: String,
    /// `Dx12`, `Vulkan`, `Gl`...
    pub backend: String,
    /// Whether it is the software fallback.
    pub fallback: bool,
}

/// The device and everything needed to make surfaces for it.
pub struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    /// The logical device.
    pub device: wgpu::Device,
    /// Its queue.
    pub queue: wgpu::Queue,
    /// Which adapter, for the log.
    pub summary: AdapterSummary,
}

impl Gpu {
    /// Finds an adapter that can draw to `surface` (any adapter when there
    /// is no surface yet), trying hardware first and the software fallback
    /// second, and opens a device on it.
    pub fn new(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Gpu, String> {
        let mut failures = Vec::new();
        for fallback in [false, true] {
            let options = wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: surface,
                force_fallback_adapter: fallback,
                apply_limit_buckets: false,
            };
            let adapter = match pollster::block_on(instance.request_adapter(&options)) {
                Ok(adapter) => adapter,
                Err(e) => {
                    failures.push(format!("{}: {e}", kind(fallback)));
                    continue;
                }
            };
            let descriptor = wgpu::DeviceDescriptor {
                label: Some("kiln-device"),
                required_features: wgpu::Features::empty(),
                // Kiln draws little; the lowest limits run everywhere,
                // the fallback adapters included.
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                experimental_features: wgpu::ExperimentalFeatures::default(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            };
            match pollster::block_on(adapter.request_device(&descriptor)) {
                Ok((device, queue)) => {
                    let info = adapter.get_info();
                    let summary = AdapterSummary {
                        name: info.name,
                        backend: format!("{:?}", info.backend),
                        fallback: fallback || info.device_type == wgpu::DeviceType::Cpu,
                    };
                    return Ok(Gpu {
                        instance,
                        adapter,
                        device,
                        queue,
                        summary,
                    });
                }
                Err(e) => failures.push(format!("{}: {e}", kind(fallback))),
            }
        }
        Err(format!("no usable GPU adapter ({})", failures.join("; ")))
    }
}

fn kind(fallback: bool) -> &'static str {
    if fallback {
        "software fallback"
    } else {
        "hardware"
    }
}

/// A window's swapchain.
pub struct Surface {
    surface: wgpu::Surface<'static>,
    /// The current configuration; `format` never changes after creation.
    pub config: wgpu::SurfaceConfiguration,
    pending: Option<(u32, u32)>,
}

impl Surface {
    /// Makes and configures a surface for `window`. `format` is the one
    /// every surface must use once the pipelines exist (they are built for
    /// one format); `None` picks the best for the first surface.
    pub fn new(
        gpu: &Gpu,
        window: Arc<Window>,
        surface: Option<wgpu::Surface<'static>>,
        format: Option<wgpu::TextureFormat>,
    ) -> Result<Surface, String> {
        let size = window.inner_size();
        let surface = match surface {
            Some(surface) => surface,
            None => gpu
                .instance
                .create_surface(window)
                .map_err(|e| format!("couldn't create a surface: {e}"))?,
        };
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = format
            .filter(|f| caps.formats.contains(f))
            .or_else(|| caps.formats.iter().copied().find(|f| !f.is_srgb()))
            .or_else(|| caps.formats.first().copied())
            .ok_or("the GPU can't draw to this window")?;
        // Mailbox shows a frame without waiting for the display's refresh;
        // Fifo is always there.
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        Ok(Surface {
            surface,
            config,
            pending: None,
        })
    }

    /// Notes a new size; applied by the next [`Surface::frame`].
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.pending = Some((width, height));
        }
    }

    /// The next texture to draw into, or `None` when there is nothing to
    /// draw on right now (minimised, occluded, briefly unavailable); the
    /// caller tries again on the next redraw.
    pub fn frame(&mut self, gpu: &Gpu) -> Option<wgpu::SurfaceTexture> {
        if let Some((width, height)) = self.pending.take() {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&gpu.device, &self.config);
        }
        for _ in 0..2 {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame) => return Some(frame),
                wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                    // Usable now; reconfigure for the next one.
                    self.pending = Some((self.config.width, self.config.height));
                    return Some(frame);
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.surface.configure(&gpu.device, &self.config);
                }
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => return None,
            }
        }
        None
    }
}
