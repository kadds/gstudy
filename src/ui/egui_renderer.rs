use std::{collections::HashMap, fs::File, io::Read};

use egui::{FontFamily, TextureId};
use font_kit::{family_name::FamilyName, properties::Properties};

use crate::{
    backends::{
        wgpu_backend::{Renderer, WGPURenderTarget, WGPUResource},
        WGPUBackend,
    },
    modules::hardware_renderer::common::{FsTarget, PipelinePass, PipelineReflector},
    types::{Color, Rectu, Size},
    util::any_as_u8_slice_array,
};
use std::num::NonZeroU32;

use super::UIContext;

struct ShaderConstantSize {
    size: [f32; 2],
}

struct EguiInner {
    mat_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline_pass: PipelinePass,
    textures: HashMap<TextureId, (wgpu::Texture, wgpu::BindGroup)>,
    buffer_cache: BufferCache,
    render_target: WGPURenderTarget,
}

pub struct EguiRenderer {
    ctx: egui::Context,
    inner: Option<EguiInner>,
    constant_size: Option<ShaderConstantSize>,
}

pub struct EguiRenderFrame {
    pub textures: egui::epaint::textures::TexturesDelta,
    pub shapes: Vec<egui::epaint::ClippedShape>,
}

struct BufferCache {
    vertex_buffers: Vec<(wgpu::Buffer, usize)>,
    vertex_offset: usize,
    index_buffers: Vec<(wgpu::Buffer, usize)>,
    index_offset: usize,
}

const DEFAULT_VERTEX_BUFFER_SIZE: usize = 1 << 20;
const DEFAULT_INDEX_BUFFER_SIZE: usize = 1 << 19;

pub struct BufferItem {
    pub index_buffer: usize,
    pub vertex_buffer: usize,
    pub base_index: u32,
    pub base_vertex: u32,
    pub index_count: u32,
}

impl BufferCache {
    pub fn new() -> Self {
        Self {
            vertex_buffers: Vec::new(),
            vertex_offset: 0,
            index_buffers: Vec::new(),
            index_offset: 0,
        }
    }
    pub fn reset(&mut self) {
        self.vertex_offset = 0;
        self.index_offset = 0;
        for (_, used) in &mut self.vertex_buffers {
            *used = 0;
        }
        for (_, used) in &mut self.index_buffers {
            *used = 0;
        }
    }
    pub fn add<F: FnMut(BufferItem)>(
        &mut self,
        gpu: &WGPUResource,
        indices: &mut [u32],
        vertices: &mut [egui::epaint::Vertex],
        mut f: F,
    ) {
        let mut index_cursor = 0;

        while index_cursor < indices.len() {
            self.prepare_index_buffer(gpu, self.index_offset);
            self.prepare_vertex_buffer(gpu, self.vertex_offset);

            let (index_buffer, iused) = self.index_buffers.get_mut(self.index_offset).unwrap();
            let (vertex_buffer, vused) = self.vertex_buffers.get_mut(self.vertex_offset).unwrap();

            let iremain = DEFAULT_INDEX_BUFFER_SIZE - *iused;
            let vremain = DEFAULT_VERTEX_BUFFER_SIZE - *vused;

            let span_start = index_cursor;
            let mut min_vindex = indices[index_cursor];
            let mut max_vindex = indices[index_cursor];

            while index_cursor < indices.len() {
                let (mut new_min, mut new_max) = (min_vindex, max_vindex);
                for i in 0..3 {
                    let idx = indices[index_cursor + i];
                    new_min = new_min.min(idx);
                    new_max = new_max.max(idx);
                }

                if new_max - new_min + 1 < vremain as u32 && index_cursor - span_start + 4 < iremain
                {
                    // Triangle fits
                    min_vindex = new_min;
                    max_vindex = new_max;
                    index_cursor += 3;
                } else {
                    break;
                }
            }

            assert!(
                index_cursor > span_start,
                "One triangle spanned more than {} vertices",
                DEFAULT_VERTEX_BUFFER_SIZE
            );
            let vertex_count = (max_vindex - min_vindex + 1) as usize;
            let index_count = index_cursor - span_start;

            let vertex_used = (vertex_count + wgpu::COPY_BUFFER_ALIGNMENT as usize)
                & !(wgpu::COPY_BUFFER_ALIGNMENT as usize - 1);
            let index_used = (index_count + wgpu::COPY_BUFFER_ALIGNMENT as usize)
                & !(wgpu::COPY_BUFFER_ALIGNMENT as usize - 1);

            let indices_new = &mut indices[span_start..index_cursor];
            if min_vindex != 0 {
                for v in indices_new.iter_mut() {
                    *v -= min_vindex as u32;
                }
            }
            let vertices_new = &vertices[(min_vindex as usize)..=(max_vindex as usize)];

            let queue = gpu.queue();

            queue.write_buffer(
                index_buffer,
                (*iused as u64) * std::mem::size_of::<u32>() as u64,
                any_as_u8_slice_array(indices_new),
            );
            queue.write_buffer(
                vertex_buffer,
                (*vused as u64) * std::mem::size_of::<egui::epaint::Vertex>() as u64,
                any_as_u8_slice_array(vertices_new),
            );
            f(BufferItem {
                index_buffer: self.index_offset,
                vertex_buffer: self.vertex_offset,
                base_index: *iused as u32,
                base_vertex: (*vused as u32) * std::mem::size_of::<egui::epaint::Vertex>() as u32,
                index_count: index_count as u32,
            });
            *iused += index_used;
            *vused += vertex_used;
            if iremain < index_used + 3 {
                self.index_offset += 1;
            }
            if vremain < vertex_used + 4 {
                self.vertex_offset += 1;
            }
        }
    }
    fn prepare_vertex_buffer(&mut self, gpu: &WGPUResource, vertex_offset: usize) {
        if vertex_offset >= self.vertex_buffers.len() {
            let buf = self.new_buffer(
                gpu,
                wgpu::BufferUsages::VERTEX,
                (DEFAULT_VERTEX_BUFFER_SIZE * std::mem::size_of::<egui::epaint::Vertex>()) as u64,
            );
            self.vertex_buffers.push((buf, 0));
        }
    }

    fn prepare_index_buffer(&mut self, gpu: &WGPUResource, index_offset: usize) {
        if index_offset >= self.index_buffers.len() {
            let buf = self.new_buffer(
                gpu,
                wgpu::BufferUsages::INDEX,
                (DEFAULT_INDEX_BUFFER_SIZE * std::mem::size_of::<u32>()) as u64,
            );
            self.index_buffers.push((buf, 0));
        }
    }

    pub fn get_vertex_buffer(&self, idx: usize) -> &wgpu::Buffer {
        &self.vertex_buffers[idx].0
    }
    pub fn get_index_buffer(&self, idx: usize) -> &wgpu::Buffer {
        &self.index_buffers[idx].0
    }

    fn new_buffer(
        &self,
        gpu: &WGPUResource,
        buffer_type: wgpu::BufferUsages,
        size: u64,
    ) -> wgpu::Buffer {
        let mat_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("egui mat buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | buffer_type,
            mapped_at_creation: false,
        });
        mat_buffer
    }
}

fn load_font(
    fd: &mut egui::FontDefinitions,
    source: &mut impl font_kit::source::Source,
    name: &str,
    family: FontFamily,
) -> anyhow::Result<()> {
    let font =
        source.select_best_match(&[FamilyName::Title(name.to_string())], &Properties::new())?;
    let data = font.load()?;

    fd.font_data.insert(
        name.to_string(),
        egui::FontData::from_owned(
            data.copy_font_data()
                .ok_or(anyhow::Error::msg("load font data fail"))?
                .to_vec(),
        ),
    );
    fd.families
        .entry(family)
        .and_modify(|v| v.insert(0, name.to_string()))
        .or_default();
    Ok(())
}

fn load_fonts(fd: &mut egui::FontDefinitions) {
    let mut s = font_kit::source::SystemSource::new();
    // for f in s.all_families().unwrap() {
    //     log::info!("{}", f);
    // }

    let fonts = vec![
        ("Microsoft YaHei UI", FontFamily::Proportional),
        ("Segoe UI", FontFamily::Proportional),
        ("Consolas", FontFamily::Monospace),
    ];
    for (name, family) in fonts.into_iter() {
        if let Err(e) = load_font(fd, &mut s, name, family) {
            log::warn!("load font {} fail with {}", name, e);
        }
    }
}

impl EguiRenderer {
    pub fn new() -> Self {
        let ctx = egui::Context::default();
        let mut fd = egui::FontDefinitions::default();
        load_fonts(&mut fd);

        ctx.set_fonts(fd);
        Self {
            ctx,
            inner: None,
            constant_size: None,
        }
    }
    pub fn ctx(&self) -> egui::Context {
        self.ctx.clone()
    }
    fn new_texture(
        gpu: &WGPUResource,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        size: Size,
    ) -> (wgpu::Texture, wgpu::BindGroup) {
        let device = gpu.device();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("egui_texture"),
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui_texture"),
            layout: texture_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            }],
        });
        (texture, bind_group)
    }

    fn update_texture(
        gpu: &WGPUResource,
        id: egui::TextureId,
        inner: &mut EguiInner,
        data: egui::epaint::ImageDelta,
    ) {
        let size = data.image.size();
        let mut rect = Rectu::new(0, 0, size[0] as u32, size[1] as u32);
        if let Some(pos) = data.pos {
            rect.x = pos[0] as u32;
            rect.y = pos[1] as u32;
            log::info!("{:?} {:?}", pos, rect);
        } else {
            log::info!("{:?}", rect);
        }

        let size = data.image.size();

        if !inner.textures.contains_key(&id) {
            inner.textures.insert(
                id,
                Self::new_texture(
                    gpu,
                    &inner.pipeline_pass.bind_group_layouts[1],
                    Size::new(size[0] as u32, size[1] as u32),
                ),
            );
        }

        let texture = &inner.textures.get(&id).unwrap().0;

        match &data.image {
            egui::epaint::ImageData::Color(c) => {
                Self::copy_texture(gpu, texture, 4, rect, any_as_u8_slice_array(&c.pixels));
            }
            egui::epaint::ImageData::Font(f) => {
                let data: Vec<egui::Color32> = f.srgba_pixels(1.0f32).collect();
                Self::copy_texture(gpu, texture, 4, rect, any_as_u8_slice_array(&data));
            }
        }
    }
    fn copy_texture(
        gpu: &WGPUResource,
        texture: &wgpu::Texture,
        bytes_per_pixel: u32,
        rectangle: Rectu,
        data: &[u8],
    ) {
        let dst = wgpu::ImageCopyTexture {
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: rectangle.x,
                y: rectangle.y,
                z: 0,
            },
            texture,
            aspect: wgpu::TextureAspect::All,
        };
        let row_bytes = rectangle.z * bytes_per_pixel;
        let data_layout = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(row_bytes),
            rows_per_image: None,
        };

        gpu.queue().write_texture(
            dst,
            data,
            data_layout,
            wgpu::Extent3d {
                width: rectangle.z,
                height: rectangle.w,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn resize(&mut self, size: Size) {
        self.constant_size = Some(ShaderConstantSize {
            size: [size.x as f32, size.y as f32],
        });
    }

    fn init(&mut self, gpu_resource: &WGPUResource) {
        let device = gpu_resource.device();
        let pipeline_pass = PipelineReflector::new(Some("egui"), device)
            .add_vs(wgpu::include_spirv!("../compile_shaders/ui.vert"))
            .add_fs(
                wgpu::include_spirv!("../compile_shaders/ui.frag"),
                FsTarget::new_blend_alpha_add_mix(wgpu::TextureFormat::Rgba8UnormSrgb),
            )
            .build_default();

        let mat_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("egui"),
            size: 8,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("egui"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0f32,
            lod_max_clamp: f32::MAX,
            compare: None,
            anisotropy_clamp: None,
            border_color: None,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui"),
            layout: &pipeline_pass.bind_group_layouts[0],
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &mat_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        self.inner = Some(EguiInner {
            pipeline_pass,
            mat_buffer,
            bind_group,
            textures: HashMap::new(),
            buffer_cache: BufferCache::new(),
            render_target: WGPURenderTarget::new("egui renderer"),
        })
    }

    pub fn render(
        &mut self,
        backend: &WGPUBackend,
        frame: EguiRenderFrame,
        color: Color,
        scale_factor: f32,
        ui_context: &mut UIContext,
    ) {
        struct RenderPrimitive<'a> {
            item: BufferItem,
            clip: egui::epaint::Rect,
            texture_bind_group: &'a wgpu::BindGroup,
        }

        let mut render_primitive = Vec::new();
        let mut renderer = backend.renderer();
        let gpu_resource = renderer.resource();

        ui_context.executor.render(gpu_resource.clone());

        if self.inner.is_none() {
            self.init(&gpu_resource);
        }
        let inner = self.inner.as_mut().unwrap();

        if let Some(s) = &self.constant_size {
            gpu_resource
                .queue()
                .write_buffer(&inner.mat_buffer, 0, any_as_u8_slice_array(&s.size));
            self.constant_size = None;
        }

        let meshes = self.ctx.tessellate(frame.shapes);
        let textures = frame.textures;

        for texture in textures.set {
            Self::update_texture(&gpu_resource, texture.0, inner, texture.1);
        }
        inner.buffer_cache.reset();

        {
            let mut pass_encoder =
                match renderer.begin_surface(&mut inner.render_target, Some(color)) {
                    Some(v) => v,
                    None => return,
                };

            for mut mesh in meshes {
                let mut skip = false;
                let clip = mesh.clip_rect;
                let (mut indices, mut vertices, texture) = match &mut mesh.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => (
                        &mut mesh.indices,
                        &mut mesh.vertices,
                        match inner.textures.get(&mesh.texture_id) {
                            Some(v) => &v.1,
                            None => match mesh.texture_id {
                                TextureId::User(u) => {
                                    if let Some(c) = ui_context.canvas_map.get(&u) {
                                        c.make_sure(&gpu_resource, pass_encoder.encoder_mut());
                                        match c.display_frame(&gpu_resource) {
                                            Some(v) => v,
                                            None => {
                                                skip = true;
                                                continue;
                                            }
                                        }
                                    } else {
                                        panic!("canvas texture id not found");
                                    }
                                }
                                _ => {
                                    panic!("invalid texture id");
                                }
                            },
                        },
                    ),
                    egui::epaint::Primitive::Callback(callback) => {
                        todo!("3d callback");
                    }
                };
                if skip {
                    continue;
                }

                inner
                    .buffer_cache
                    .add(&gpu_resource, indices, vertices, |item| {
                        render_primitive.push(RenderPrimitive {
                            item,
                            clip,
                            texture_bind_group: texture,
                        });
                    });
            }
            let mut pass = pass_encoder.new_pass();

            pass.set_pipeline(&inner.pipeline_pass.pipeline);
            pass.set_bind_group(0, &inner.bind_group, &[]);
            for primitive in render_primitive {
                let clip = primitive.clip;
                let mut x = (clip.left() * scale_factor) as u32;
                let mut y = (clip.top() * scale_factor) as u32;
                let mut width = (clip.width() * scale_factor) as u32;
                let mut height = (clip.height() * scale_factor) as u32;
                if !clip.is_finite() {
                    x = 0;
                    y = 0;
                    width = gpu_resource.width();
                    height = gpu_resource.height();
                }
                if width > 0 && height > 0 {
                    pass.set_scissor_rect(x, y, width, height);
                } else {
                    continue;
                }
                let item = primitive.item;
                let index_buffer = inner.buffer_cache.get_index_buffer(item.index_buffer);
                let vertex_buffer = inner.buffer_cache.get_vertex_buffer(item.vertex_buffer);

                pass.set_bind_group(1, primitive.texture_bind_group, &[]);
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.set_vertex_buffer(0, vertex_buffer.slice((item.base_vertex as u64)..));
                pass.draw_indexed(
                    item.base_index..(item.index_count + item.base_index),
                    0,
                    0..1,
                )
            }
        }

        for free_id in textures.free {
            inner.textures.remove(&free_id);
        }
    }
}
