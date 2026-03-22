use crate::RenderError;
use std::sync::Arc;
use wgpu::{Adapter, Device, Instance, Queue, Surface, SurfaceConfiguration, TextureFormat};
use winit::window::Window;

pub struct GpuContext<'window> {
    pub surface: Surface<'window>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub adapter: Adapter,
    pub format: TextureFormat,
}

impl<'window> GpuContext<'window> {
    pub async fn new(window: Arc<Window>) -> Result<GpuContext<'window>, RenderError> {
        let size = window.inner_size();

        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        tracing::info!(adapter = %adapter.get_info().name, "GPU adapter selected");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("wmux_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        // Prefer non-sRGB format so that our sRGB hex colors (#0d1117 etc.)
        // are passed directly to the display without double-gamma correction.
        // With an sRGB format, the GPU applies linear→sRGB on output, but our
        // colors are already in sRGB space — making everything appear ~2x lighter.
        let format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .or(surface_caps.formats.first())
            .copied()
            .ok_or(RenderError::NoSupportedFormats)?;

        let alpha_mode = surface_caps
            .alpha_modes
            .iter()
            .find(|m| **m == wgpu::CompositeAlphaMode::PreMultiplied)
            .or(surface_caps.alpha_modes.first())
            .copied()
            .ok_or(RenderError::NoSupportedAlphaModes)?;

        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(GpuContext {
            surface,
            device,
            queue,
            config,
            adapter,
            format,
        })
    }

    #[inline]
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.config.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.config.height
    }
}
