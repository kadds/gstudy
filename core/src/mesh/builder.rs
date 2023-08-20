use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::{
    types::{Rectu, Vec3f},
    util::{any_as_u8_slice, any_as_u8_slice_array},
};

use super::{FieldOffset, Indices, Mesh, MeshPropertyType, PositionVertices};

#[derive(Default)]
pub struct MeshBuilder {
    properties: BTreeSet<MeshPropertyType>,
    mesh: Option<Mesh>,
    properties_written: HashMap<MeshPropertyType, usize>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self {
            mesh: None,
            ..Default::default()
        }
    }

    pub fn add_property(&mut self, property: MeshPropertyType) {
        if !self.mesh.is_none() {
            panic!("please modify property before add vertex data")
        }

        self.properties.insert(property);
        self.properties_written.insert(property, 0);
    }

    fn finish_property(&mut self) -> &mut Mesh {
        if self.mesh.is_none() {
            self.mesh = Some(Mesh::new(self.properties.iter()));
        }
        let mesh = self.mesh.as_mut().unwrap();
        mesh
    }

    pub fn add_indices32(&mut self, indices: &[u32]) {
        let mesh = self.finish_property();
        match &mut mesh.indices {
            Indices::Unknown => {
                mesh.indices = Indices::U32(indices.into_iter().cloned().collect());
            }
            Indices::U32(d) => d.extend_from_slice(indices),
            _ => panic!("different index type"),
        }
    }

    pub fn add_indices_none(&mut self) {
        let mesh = self.finish_property();
        match &mut mesh.indices {
            Indices::Unknown => {
                mesh.indices = Indices::None;
            }
            Indices::None => {}
            _ => panic!("different index type"),
        }
    }

    pub fn add_position_vertices3(&mut self, position: &[Vec3f]) {
        let mesh = self.finish_property();
        match &mut mesh.position_vertices {
            PositionVertices::Unknown => {
                mesh.position_vertices =
                    PositionVertices::F3(position.into_iter().cloned().collect());
                mesh.vertex_count += position.len();
            }
            PositionVertices::F3(d) => {
                d.extend_from_slice(position);
                mesh.vertex_count += d.len();
            }
            _ => panic!("different position vertex type"),
        }
    }

    pub fn add_position_vertices_none(&mut self) {
        let mesh = self.finish_property();
        match &mut mesh.position_vertices {
            PositionVertices::Unknown => {
                mesh.position_vertices = PositionVertices::None;
            }
            PositionVertices::None => {}
            _ => panic!("different position vertex type"),
        }
    }

    pub fn add_properties_vertices(&mut self, vertices: &[PropertiesRow]) {
        let mesh = self.finish_property();
        for vertex in vertices {
            mesh.properties
                .extend_from_slice(any_as_u8_slice_array(&vertex.data))
        }
    }

    pub unsafe fn add_raw_properties_vertices(&mut self, data: &[u8], count: usize) {
        let mesh = self.finish_property();
        mesh.properties.extend_from_slice(data);
        let rows = data.len() / mesh.row_strip_size as usize;
        if rows != count {
            panic!("invalid data size");
        }

        for (_, value) in &mut self.properties_written {
            *value += count;
        }
    }

    pub fn set_clip(&mut self, clip: Rectu) {
        let mesh = self.finish_property();
        mesh.clip = Some(clip);
    }

    pub fn add_property_vertices<T>(&mut self, property: MeshPropertyType, vertices: &[T]) {
        let written = *self.properties_written.get(&property).unwrap();
        let mesh = self.finish_property();
        let o = mesh.properties_offset.get(&property).unwrap();
        if std::mem::size_of::<T>() as u32 != o.len {
            panic!("invalid property size, {:?}", property);
        }
        let row_strip = mesh.row_strip_size as usize;
        let final_size = row_strip * (vertices.len() + written);
        if mesh.properties.len() < final_size {
            mesh.properties.resize(final_size, 0);
        }

        let mut cur_offset = o.offset as usize;
        for vertex in vertices {
            unsafe {
                let src_slice = any_as_u8_slice(vertex);
                let src = src_slice.as_ptr();
                let dst = mesh.properties.as_mut_ptr().add(cur_offset as usize);
                std::ptr::copy_nonoverlapping(src, dst, o.len as usize);
            }
            cur_offset += row_strip;
        }
        *self.properties_written.get_mut(&property).unwrap() += vertices.len();
    }

    pub fn properties_vertices_builder(&mut self) -> PropertiesVerticesBuilder {
        let mesh = self.finish_property();
        PropertiesVerticesBuilder {
            properties_offset: mesh.properties_offset.clone(),
            result: vec![],
            row_strip: mesh.row_strip_size,
        }
    }

    pub fn build(mut self) -> anyhow::Result<Mesh> {
        self.finish_property();

        // check properties vertex count
        let mut property_vertex_write_count = usize::MAX;
        for (property, count) in &self.properties_written {
            if property_vertex_write_count == usize::MAX {
                property_vertex_write_count = *count;
            } else {
                if property_vertex_write_count != *count {
                    anyhow::bail!("some properties has no vertex data");
                }
            }
        }

        let mesh = self.finish_property();

        match mesh.position_vertices {
            PositionVertices::Unknown => {
                anyhow::bail!("set position vertices first");
            }
            PositionVertices::None => {
                mesh.vertex_count = property_vertex_write_count;
            }
            _ => {
                if property_vertex_write_count != usize::MAX
                    && mesh.vertex_count != property_vertex_write_count
                {
                    anyhow::bail!("position vertex count is not equal to property vertex count");
                }
            }
        }
        if mesh.vertex_count == usize::MAX {
            log::info!("empty mesh");
            mesh.vertex_count = 0;
        }

        // check index count
        match mesh.indices {
            Indices::Unknown => {
                anyhow::bail!("set indices first");
            }
            _ => (),
        }

        drop(mesh);

        Ok(self.mesh.take().unwrap())
    }
}

pub struct PropertiesRow {
    data: Vec<u8>,
}

pub struct PropertiesVerticesBuilder {
    properties_offset: BTreeMap<MeshPropertyType, FieldOffset>,
    row_strip: u32,
    result: Vec<PropertiesRow>,
}

impl PropertiesVerticesBuilder {
    pub fn add_row<T>(&mut self) -> PropertiesRowBuilder {
        let mut data = Vec::with_capacity(self.row_strip as usize);
        data.resize(self.row_strip as usize, 0);
        PropertiesRowBuilder {
            properties_offset: &self.properties_offset,
            vertex: PropertiesRow { data },
            result: &mut self.result,
        }
    }

    pub fn build(self) -> Vec<PropertiesRow> {
        self.result
    }
}

pub struct PropertiesRowBuilder<'a> {
    properties_offset: &'a BTreeMap<MeshPropertyType, FieldOffset>,
    vertex: PropertiesRow,
    result: &'a mut Vec<PropertiesRow>,
}

impl<'a> PropertiesRowBuilder<'a> {
    pub fn fill<T>(&mut self, property: MeshPropertyType, vertex: &T) {
        let o = self.properties_offset.get(&property).unwrap();
        if std::mem::size_of::<T>() as u32 != o.len {
            panic!("invalid property size, {:?}", property);
        }
        unsafe {
            let src_slice = any_as_u8_slice(vertex);
            let src = src_slice.as_ptr();
            let dst = self.vertex.data.as_mut_ptr().add(o.offset as usize);
            std::ptr::copy_nonoverlapping(src, dst, o.len as usize);
        }
    }
    pub fn finish(mut self) {
        self.result.push(self.vertex)
    }
}
