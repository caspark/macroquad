//! Minimal wgpu example using macroquad for windowing.
//!
//! Draws a coloured triangle to the center of the screen using wgpu,
//! with macroquad handling the window creation and event loop.
//!
//! Usage:
//!   cargo run --example wgpu_triangle             # interactive mode
//!   cargo run --example wgpu_triangle -- --frames 10 --screenshot out.png  # capture mode

use miniquad::conf::Conf;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

struct WgpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    // For screenshot capture
    screenshot_buffer: Option<wgpu::Buffer>,
    screenshot_texture: Option<wgpu::Texture>,
}

impl WgpuState {
    fn new() -> Self {
        let mq_window = miniquad::MiniquadWindow::new();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::DX12,
            ..Default::default()
        });

        // SAFETY: The miniquad window outlives the surface because miniquad owns
        // the event loop and window. The surface is dropped before the window.
        let surface = unsafe {
            let raw_display = mq_window
                .display_handle()
                .expect("display handle")
                .as_raw();
            let raw_window = mq_window.window_handle().expect("window handle").as_raw();
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: raw_display,
                    raw_window_handle: raw_window,
                })
                .expect("failed to create surface")
        };

        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            }))
            .expect("failed to find adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("device"),
                ..Default::default()
            },
        ))
        .expect("failed to create device");

        let (width, height) = miniquad::window::screen_size();
        let width = width as u32;
        let height = height as u32;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("triangle shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        WgpuState {
            device,
            queue,
            surface,
            surface_config,
            render_pipeline,
            screenshot_buffer: None,
            screenshot_texture: None,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            // Invalidate screenshot resources on resize
            self.screenshot_buffer = None;
            self.screenshot_texture = None;
        }
    }

    fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface
                    .configure(&self.device, &self.surface_config);
                self.surface
                    .get_current_texture()
                    .expect("failed to get surface texture after reconfigure")
            }
            Err(e) => panic!("failed to get surface texture: {e}"),
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }

    fn capture_screenshot(&mut self, path: &str) {
        let width = self.surface_config.width;
        let height = self.surface_config.height;

        // We need to render to a texture we can read back
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("screenshot texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("screenshot encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screenshot render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.draw(0..3, 0..1);
        }

        // Bytes per row must be aligned to 256
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let padded_bytes_per_row =
            (unpadded_bytes_per_row + 255) & !255;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map and read
        let buffer_slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + (width * bytes_per_pixel) as usize;
            let row_data = &data[start..end];
            // Convert from surface format (likely Bgra8) to Rgba8
            for pixel in row_data.chunks_exact(4) {
                pixels.push(pixel[2]); // R (from B)
                pixels.push(pixel[1]); // G
                pixels.push(pixel[0]); // B (from R)
                pixels.push(pixel[3]); // A
            }
        }
        drop(data);
        buffer.unmap();

        // Save as PNG
        image::save_buffer(
            path,
            &pixels,
            width,
            height,
            image::ColorType::Rgba8,
        )
        .expect("failed to save screenshot");
        println!("Screenshot saved to {path}");
    }
}

const SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Triangle vertices centered on screen
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.5),    // top
        vec2<f32>(-0.5, -0.5),  // bottom-left
        vec2<f32>(0.5, -0.5),   // bottom-right
    );
    var colors = array<vec3<f32>, 3>(
        vec3<f32>(1.0, 0.0, 0.0),  // red
        vec3<f32>(0.0, 1.0, 0.0),  // green
        vec3<f32>(0.0, 0.0, 1.0),  // blue
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.color = colors[vertex_index];
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
"#;

struct Stage {
    wgpu_state: Option<WgpuState>,
    frame_count: u32,
    max_frames: Option<u32>,
    screenshot_path: Option<String>,
    screenshot_taken: bool,
}

impl miniquad::EventHandler for Stage {
    fn update(&mut self) {}

    fn draw(&mut self) {
        if self.wgpu_state.is_none() {
            self.wgpu_state = Some(WgpuState::new());
        }

        let state = self.wgpu_state.as_mut().unwrap();
        state.render();
        self.frame_count += 1;

        if let (Some(max), Some(path), false) =
            (self.max_frames, &self.screenshot_path, self.screenshot_taken)
        {
            if self.frame_count >= max {
                state.capture_screenshot(path);
                self.screenshot_taken = true;
                miniquad::window::order_quit();
            }
        }
    }

    fn resize_event(&mut self, width: f32, height: f32) {
        if let Some(state) = &mut self.wgpu_state {
            state.resize(width as u32, height as u32);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut max_frames: Option<u32> = None;
    let mut screenshot_path: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--frames" => {
                i += 1;
                max_frames = Some(args[i].parse().expect("--frames needs a number"));
            }
            "--screenshot" => {
                i += 1;
                screenshot_path = Some(args[i].clone());
            }
            _ => {
                eprintln!("Unknown arg: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let conf = Conf {
        window_title: "wgpu triangle".to_string(),
        window_width: 800,
        window_height: 600,
        platform: miniquad::conf::Platform {
            skip_graphics_context: true,
            ..Default::default()
        },
        ..Default::default()
    };

    miniquad::start(conf, move || {
        Box::new(Stage {
            wgpu_state: None,
            frame_count: 0,
            max_frames,
            screenshot_path,
            screenshot_taken: false,
        })
    });
}
