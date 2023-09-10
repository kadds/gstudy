use std::{collections::HashMap, hash::Hash};

use indexmap::{IndexMap, IndexSet};

use crate::{
    types::{Rectu, Vec3f},
    util::{any_as_u8_slice, any_as_u8_slice_array},
};

use super::{Indices, Mesh, PositionVertices};

#[derive(Debug, Clone, Copy)]
pub struct FieldOffset {
    offset: u32,
    len: u32,
}

pub trait Property: Eq + PartialEq + Hash + Clone + Copy + std::fmt::Debug {
    fn size_alignment(&self) -> (u32, u32);
}

#[derive(Debug)]
pub struct PropertiesFrame<P> {
    pub properties: IndexSet<P>,
    pub data: Vec<u8>,
    pub properties_offset: IndexMap<P, FieldOffset>,
    pub row_strip_size: u32,
    pub row_size: u32,
    pub count: u64,
}

impl<P> PropertiesFrame<P> {
    pub fn view(&self) -> &[u8] {
        &self.data
    }
}

impl<P> Default for PropertiesFrame<P> {
    fn default() -> Self {
        Self {
            properties: IndexSet::default(),
            data: vec![],
            properties_offset: IndexMap::default(),
            row_strip_size: 0,
            row_size: 0,
            count: 0,
        }
    }
}

pub struct PropertiesBuilder<P> {
    properties: IndexSet<P>,
    properties_written: HashMap<P, u64>,
    data: Vec<u8>,
    properties_offset: IndexMap<P, FieldOffset>,
    row_strip_size: u32,
    row_size: u32,

    enable_alignment: bool,
    count: u64,
}

impl<P> Default for PropertiesBuilder<P> {
    fn default() -> Self {
        Self {
            properties: IndexSet::default(),
            properties_written: HashMap::new(),
            data: vec![],
            properties_offset: IndexMap::default(),
            row_strip_size: 0,
            row_size: 0,
            enable_alignment: false,
            count: 0,
        }
    }
}

impl<P> PropertiesBuilder<P>
where
    P: Property,
{
    pub fn has_property(&mut self, property: &P) -> bool {
        self.properties.contains(property)
    }

    pub fn add_property(&mut self, property: P) {
        self.properties.insert(property);
        self.properties_written.insert(property, 0);
    }

    fn finish(&mut self) {
        if self.properties_offset.is_empty() {
            let mut offset = 0;
            let mut max_alignment = 0;

            for prop in &self.properties {
                let (size, alignment) = prop.size_alignment();
                let rest = offset % alignment;
                if rest < size {
                    if rest != 0 {
                        if self.enable_alignment {
                            offset += alignment - rest;
                        }
                    }
                }
                max_alignment = max_alignment.max(alignment);
                self.properties_offset
                    .insert(prop.clone(), FieldOffset { offset, len: size });
                offset += size;
            }
            if max_alignment > 0 {
                self.row_size = offset;

                let rest = offset % max_alignment;
                if rest != 0 {
                    // offset += max_alignment - rest;
                }
                self.row_strip_size = offset;
            }
        }
    }

    pub fn add_property_data<T>(&mut self, property: P, data: &[T]) {
        self.finish();

        let written = self.properties_written.get_mut(&property).unwrap();
        let o = self.properties_offset.get(&property).unwrap().clone();
        if std::mem::size_of::<T>() as u32 != o.len {
            panic!("invalid property size, {:?}", property);
        }
        let row_strip = self.row_strip_size as u64;
        let final_size = row_strip * (data.len() as u64 + *written);
        if (self.data.len() as u64) < final_size {
            self.data.resize(final_size as usize, 0);
        }

        let mut cur_offset = o.offset as u64 + *written * row_strip;
        for t in data {
            unsafe {
                let src_slice = any_as_u8_slice(t);
                let src = src_slice.as_ptr();
                let dst = self.data.as_mut_ptr().add(cur_offset as usize);
                std::ptr::copy_nonoverlapping(src, dst, o.len as usize);
            }
            cur_offset += row_strip;
        }
        *written += data.len() as u64;
        self.count = self.count.max(*written);
    }

    pub fn add_raw_data(&mut self, data: &[u8]) {
        self.finish();

        let count = (data.len() as u64 / self.row_strip_size as u64) as u32;
        if count as u64 * self.row_strip_size as u64 != data.len() as u64 {
            panic!("unexpected raw data");
        }

        self.data.extend_from_slice(data);

        let mut max = 0;
        for (_, w) in &mut self.properties_written {
            *w += count as u64;
            max = max.max(*w);
        }

        self.count = self.count.max(max);
    }

    pub fn properties_vertices_builder(&mut self) -> PropertiesVerticesBuilder<P> {
        self.finish();
        PropertiesVerticesBuilder {
            properties_offset: self.properties_offset.clone(),
            result: vec![],
            row_strip: self.row_strip_size,
        }
    }

    pub fn build(self) -> PropertiesFrame<P> {
        for (prop, len) in &self.properties_written {
            if *len != self.count {
                log::warn!(
                    "build column frame property {:?} expect size {}, get {}",
                    prop,
                    self.count,
                    *len as u32
                );
            }
        }
        PropertiesFrame {
            data: self.data,
            properties: self.properties,
            properties_offset: self.properties_offset,
            row_strip_size: self.row_strip_size,
            row_size: self.row_size,
            count: self.count,
        }
    }
}

pub struct PropertiesRow {
    data: Vec<u8>,
}

pub struct PropertiesVerticesBuilder<P> {
    properties_offset: IndexMap<P, FieldOffset>,
    row_strip: u32,
    result: Vec<PropertiesRow>,
}

impl<P> PropertiesVerticesBuilder<P>
where
    P: Property,
{
    pub fn add_row<T>(&mut self) -> PropertiesRowBuilder<P> {
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

pub struct PropertiesRowBuilder<'a, P> {
    properties_offset: &'a IndexMap<P, FieldOffset>,
    vertex: PropertiesRow,
    result: &'a mut Vec<PropertiesRow>,
}

impl<'a, P> PropertiesRowBuilder<'a, P>
where
    P: Property,
{
    pub fn fill<T>(&mut self, property: P, vertex: &T) {
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
    pub fn finish(self) {
        self.result.push(self.vertex)
    }
}

pub struct MeshBuilder {
    mesh: Mesh,
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self {
            mesh: Mesh {
                position_vertices: PositionVertices::Unknown,
                indices: Indices::Unknown,
                clip: None,
                vertex_count: 0,
                properties: PropertiesFrame::default(),
            },
        }
    }
}

impl MeshBuilder {
    pub fn set_clip(&mut self, clip: Rectu) {
        self.mesh.clip = Some(clip);
    }

    pub fn add_indices32(&mut self, indices: &[u32]) {
        match &mut self.mesh.indices {
            Indices::Unknown => {
                self.mesh.indices = Indices::U32(indices.iter().cloned().collect());
            }
            Indices::U32(d) => d.extend_from_slice(indices),
            _ => panic!("different index type"),
        }
    }

    pub fn add_indices_none(&mut self) {
        match &mut self.mesh.indices {
            Indices::Unknown => {
                self.mesh.indices = Indices::None;
            }
            Indices::None => {}
            _ => panic!("different index type"),
        }
    }

    pub fn add_position_vertices3(&mut self, position: &[Vec3f]) {
        match &mut self.mesh.position_vertices {
            PositionVertices::Unknown => {
                self.mesh.position_vertices = PositionVertices::F3(position.to_vec());
                self.mesh.vertex_count += position.len() as u64;
            }
            PositionVertices::F3(d) => {
                d.extend_from_slice(position);
                self.mesh.vertex_count += d.len() as u64;
            }
            _ => panic!("different position vertex type"),
        }
    }

    pub fn add_position_vertices_none(&mut self) {
        match &mut self.mesh.position_vertices {
            PositionVertices::Unknown => {
                self.mesh.position_vertices = PositionVertices::None;
            }
            PositionVertices::None => {}
            _ => panic!("different position vertex type"),
        }
    }

    pub fn set_properties(&mut self, frame: PropertiesFrame<MeshPropertyType>) {
        self.mesh.properties = frame;
    }
    pub fn build(mut self) -> anyhow::Result<Mesh> {
        // check properties vertex count
        match self.mesh.position_vertices {
            PositionVertices::Unknown => {
                anyhow::bail!("set position vertices first");
            }
            PositionVertices::None => {
                self.mesh.vertex_count = self.mesh.properties.count;
            }
            _ => {
                if self.mesh.properties.count != 0
                    && self.mesh.vertex_count != self.mesh.properties.count
                {
                    anyhow::bail!("position vertex count is not equal to property vertex count");
                }
            }
        }
        if self.mesh.vertex_count == 0 {
            log::info!("empty mesh");
        }

        // check index count
        match self.mesh.indices {
            Indices::Unknown => {
                anyhow::bail!("set indices first");
            }
            _ => (),
        }

        Ok(self.mesh)
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct MeshPropertyType {
    pub name: &'static str,
    pub size: u32,
    pub alignment: u32,
}

impl MeshPropertyType {
    pub fn new<T>(name: &'static str) -> Self {
        let size = std::mem::size_of::<T>();
        let alignment = if size <= 4 {
            4
        } else if size <= 8 {
            8
        } else if size <= 16 {
            16
        } else {
            panic!()
        };
        Self {
            name,
            size: size as u32,
            alignment,
        }
    }
}

impl Property for MeshPropertyType {
    fn size_alignment(&self) -> (u32, u32) {
        (self.size, self.alignment)
    }
}

pub type MeshPropertiesBuilder = PropertiesBuilder<MeshPropertyType>;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct InstancePropertyType {
    pub name: &'static str,
    pub size: u32,
    pub alignment: u32,
}

impl InstancePropertyType {
    pub fn new<T>(name: &'static str) -> Self {
        let size = std::mem::size_of::<T>();
        let alignment = if size <= 4 {
            4
        } else if size <= 8 {
            8
        } else if size <= 16 {
            16
        } else if size <= 32 {
            32
        } else if size <= 64 {
            64
        } else {
            panic!()
        };
        Self {
            name,
            size: size as u32,
            alignment,
        }
    }
}

impl Property for InstancePropertyType {
    fn size_alignment(&self) -> (u32, u32) {
        (self.size, self.alignment)
    }
}

pub const INSTANCE_TRANSFORM: InstancePropertyType = InstancePropertyType {
    name: "transform",
    size: 64,
    alignment: 64,
};

pub type InstancePropertiesBuilder = PropertiesBuilder<InstancePropertyType>;
pub type InstancePropertiesUpdater<'a> = PropertiesUpdater<'a, InstancePropertyType>;

pub struct PropertiesUpdater<'a, P> {
    p: &'a mut PropertiesFrame<P>,
}

impl<'a, P> PropertiesUpdater<'a, P>
where
    P: Property,
{
    pub fn new(p: &'a mut PropertiesFrame<P>) -> Self {
        Self { p }
    }

    pub fn set_property<T>(&mut self, property: P, index: u64, data: &[T]) {
        let o = self.p.properties_offset.get(&property).unwrap().clone();
        if std::mem::size_of::<T>() as u32 != o.len {
            panic!("invalid property size, {:?}", property);
        }
        if index + data.len() as u64 > self.p.count {
            panic!(
                "invalid property data size, {:?} {}",
                property, self.p.count
            );
        }

        let row_strip = self.p.row_strip_size as u64;

        let mut cur_offset = o.offset as u64 + index * row_strip;
        for t in data {
            unsafe {
                let src_slice = any_as_u8_slice(t);
                let src = src_slice.as_ptr();
                let dst = self.p.data.as_mut_ptr().add(cur_offset as usize);
                std::ptr::copy_nonoverlapping(src, dst, o.len as usize);
            }
            cur_offset += row_strip;
        }
    }

    pub fn get_property1<T: Copy>(&mut self, property: P, index: u64) -> &T {
        let o = self.p.properties_offset.get(&property).unwrap().clone();
        if std::mem::size_of::<T>() as u32 != o.len {
            panic!("invalid property size, {:?}", property);
        }
        if index + 1 > self.p.count {
            panic!(
                "invalid property data size, {:?} {}",
                property, self.p.count
            );
        }

        let row_strip = self.p.row_strip_size as u64;

        let cur_offset = o.offset as u64 + index * row_strip;
        unsafe {
            let src = self.p.data.as_ptr().add(cur_offset as usize) as *const T;
            let src = src.as_ref().unwrap();
            src
        }
    }

    pub fn get_property<T: Copy>(&mut self, property: P, index: u64, count: u64) -> Vec<T> {
        let o = self.p.properties_offset.get(&property).unwrap().clone();
        if std::mem::size_of::<T>() as u32 != o.len {
            panic!("invalid property size, {:?}", property);
        }
        if index + count > self.p.count {
            panic!(
                "invalid property data size, {:?} {}",
                property, self.p.count
            );
        }

        let row_strip = self.p.row_strip_size as u64;

        let mut cur_offset = o.offset as u64 + index * row_strip;
        let mut res = vec![];
        res.reserve(count as usize);
        for _ in 0..count {
            unsafe {
                let src = self.p.data.as_ptr().add(cur_offset as usize) as *const T;
                let src = src.as_ref().unwrap();
                res.push(*src);
            }
            cur_offset += row_strip;
        }
        res
    }
}
