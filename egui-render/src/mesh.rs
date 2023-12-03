use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use core::{
    backends::wgpu_backend::WGPUResource,
    context::ResourceRef,
    mesh::{
        builder::{MeshBuilder, MeshPropertiesBuilder, MeshPropertyType},
        Mesh,
    },
    types::{Rectu, Size, Vec2f},
    util::any_as_u8_slice_array,
};

use crate::EguiRenderFrame;

#[derive(Default)]
pub struct UITextures {
    textures: HashMap<egui::TextureId, (ResourceRef, Size)>,
    user_textures: HashMap<egui::TextureId, ResourceRef>,
}

impl UITextures {
    #[profiling::function]
    fn update_texture(
        &mut self,
        gpu: &WGPUResource,
        id: egui::TextureId,
        data: egui::epaint::ImageDelta,
    ) -> bool {
        log::info!("update texture {:?}", id);
        let size = data.image.size();

        let vsize = Size::new(size[0] as u32, size[1] as u32);
        let mut rect = Rectu::new(0, 0, vsize.x, vsize.y);
        let mut rebuild = false;
        if let Some(pos) = data.pos {
            rect.x = pos[0] as u32;
            rect.y = pos[1] as u32;
        } else {
            if let Some(v) = self.textures.get(&id) {
                gpu.context().deregister(v.0.clone());
            }
            self.textures.remove(&id);
            rebuild = true;
        }

        let texture = {
            self.textures.entry(id).or_insert_with(|| {
                let texture = gpu.new_srgba_2d_texture(Some("ui texture"), vsize);
                let res = gpu.context().register_texture(texture);
                (res, vsize)
            });
            self.textures.get(&id).unwrap()
        };

        match &data.image {
            egui::epaint::ImageData::Color(c) => {
                gpu.copy_texture(
                    texture.0.texture_ref(),
                    4,
                    rect,
                    any_as_u8_slice_array(&c.pixels),
                );
            }
            egui::epaint::ImageData::Font(f) => {
                let data: Vec<egui::Color32> = f.srgba_pixels(None).collect();
                gpu.copy_texture(
                    texture.0.texture_ref(),
                    4,
                    rect,
                    any_as_u8_slice_array(&data),
                );
            }
        }
        rebuild
    }

    pub fn get(&self, texture_id: egui::TextureId) -> ResourceRef {
        self.textures.get(&texture_id).unwrap().0.clone()
    }
}

#[derive(Debug)]
pub struct UIMesh {
    ctx: egui::Context,
}

impl UIMesh {
    pub fn new(ctx: egui::Context) -> Self {
        Self { ctx }
    }

    #[profiling::function]
    pub(crate) fn generate_mesh(
        &self,
        frame: EguiRenderFrame,
        gpu: Arc<WGPUResource>,
        view_size: Size,
        ui_textures: &mut UITextures,
    ) -> (Vec<(Mesh, egui::TextureId)>, HashSet<egui::TextureId>) {
        let ctx = self.ctx.clone();
        let ppi = ctx.pixels_per_point();
        let mut rebuild_textures = HashSet::new();

        for textures in frame.textures {
            for (id, data) in textures.set {
                if ui_textures.update_texture(&gpu, id, data) {
                    rebuild_textures.insert(id);
                }
            }

            for id in textures.free {
                ui_textures.textures.remove(&id);
            }
        }

        let meshes = ctx.tessellate(frame.shapes, ctx.pixels_per_point());
        let mut ret = vec![];
        for mesh in meshes {
            let mut clip = if mesh.clip_rect.is_finite() {
                Rectu::new(
                    (mesh.clip_rect.left() * ppi) as u32,
                    (mesh.clip_rect.top() * ppi) as u32,
                    (mesh.clip_rect.width() * ppi) as u32,
                    (mesh.clip_rect.height() * ppi) as u32,
                )
            } else {
                Rectu::new(0, 0, view_size.x, view_size.y)
            };

            clip.x = clip.x.max(0);
            clip.y = clip.y.max(0);
            // log::info!("view {:?}, c {:?}", view_size, clip);
            clip.z = clip.z.min(view_size.x - clip.x);
            clip.w = clip.w.min(view_size.y - clip.y);

            let mut mesh_builder = MeshBuilder::default();
            let mut properties_builder = MeshPropertiesBuilder::default();
            let pos2 = MeshPropertyType::new::<Vec2f>("pos");
            let tex_coord = MeshPropertyType::new::<Vec2f>("texture_coord");
            let color_uint = MeshPropertyType::new::<u32>("color_uint");

            properties_builder.add_property(pos2);
            properties_builder.add_property(tex_coord);
            properties_builder.add_property(color_uint);

            mesh_builder.set_clip(clip);
            let texture_id = match mesh.primitive {
                egui::epaint::Primitive::Mesh(m) => {
                    if m.vertices.is_empty() {
                        continue;
                    }
                    properties_builder.add_raw_data(any_as_u8_slice_array(&m.vertices));
                    mesh_builder.add_indices32(&m.indices);
                    m.texture_id
                }
                egui::epaint::Primitive::Callback(_) => panic!("unsupported primitive"),
            };
            mesh_builder.add_position_vertices_none();
            mesh_builder.set_properties(properties_builder.build());

            ret.push((mesh_builder.build().unwrap(), texture_id));
        }

        (ret, rebuild_textures)
    }
}
