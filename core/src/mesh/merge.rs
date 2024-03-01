use std::sync::Arc;

use super::{builder::MeshBuilder, Mesh};

#[derive(Debug)]
pub struct MeshMerger {
    mesh: Mesh,
}

impl MeshMerger {
    pub fn merge_all<I: Iterator<Item = Mesh>>(mut mesh_list: I) -> Option<Mesh> {
        if let Some(mesh) = mesh_list.next() {
            let mut s = Self { mesh };
            for m in mesh_list {
                s.merge(&m)?;
            }
            return Some(s.mesh);
        }
        None
    }

    pub fn merge(&mut self, mesh: &Mesh) -> Option<()> {
        if self.mesh.properties.properties != mesh.properties.properties {
            return None;
        }

        let total_vertices = self.mesh.vertex_count + mesh.vertex_count;
        match &mut self.mesh.position_vertices {
            super::PositionVertices::Unknown => {
                if let super::PositionVertices::Unknown = &mesh.position_vertices {
                } else {
                    return None;
                }
            }
            super::PositionVertices::None => {
                if let super::PositionVertices::None = &mesh.position_vertices {
                } else {
                    return None;
                }
            }
            super::PositionVertices::F2(res) => {
                if let super::PositionVertices::F2(v) = &mesh.position_vertices {
                    res.extend_from_slice(v);
                } else {
                    return None;
                }
            }
            super::PositionVertices::F3(res) => {
                if let super::PositionVertices::F3(v) = &mesh.position_vertices {
                    res.extend_from_slice(v);
                } else {
                    return None;
                }
            }
            super::PositionVertices::F4(res) => {
                if let super::PositionVertices::F4(v) = &mesh.position_vertices {
                    res.extend_from_slice(v);
                } else {
                    return None;
                }
            }
        };

        self.mesh
            .properties
            .data
            .extend_from_slice(&mesh.properties.data);
        self.mesh.properties.count += mesh.properties.count;

        let mut rebuild_indices = None;
        let base = self.mesh.vertex_count as u32;

        match &mut self.mesh.indices {
            super::Indices::Unknown => {
                if let super::Indices::Unknown = &mesh.indices {
                } else {
                    return None;
                }
            }
            super::Indices::None => {
                if let super::Indices::None = &mesh.indices {
                } else {
                    return None;
                }
            }
            super::Indices::U32(res) => {
                match &mesh.indices {
                    super::Indices::U16(v) => {
                        for i in v {
                            res.push(*i as u32 + base);
                        }
                    }
                    super::Indices::U32(v) => {
                        for i in v {
                            res.push(*i + base);
                        }
                    }
                    _ => return None,
                };
            }
            super::Indices::U16(res) => {
                match &mesh.indices {
                    super::Indices::U16(v) => {
                        if total_vertices <= 65535 {
                            for i in v {
                                res.push(*i as u16 + base as u16);
                            }
                        } else {
                            let mut new_indices = vec![];
                            for i in res {
                                new_indices.push(*i as u32);
                            }
                            for i in v {
                                new_indices.push(*i as u32 + base);
                            }
                            rebuild_indices = Some(new_indices);
                        }
                    }
                    super::Indices::U32(v) => {
                        if total_vertices <= 65535 {
                            for i in v {
                                res.push(*i as u16 + base as u16);
                            }
                        } else {
                            let mut new_indices = vec![];
                            for i in res {
                                new_indices.push(*i as u32);
                            }
                            for i in v {
                                new_indices.push(*i as u32 + base);
                            }
                            rebuild_indices = Some(new_indices);
                        }
                    }
                    _ => return None,
                };
            }
        };

        if let Some(v) = rebuild_indices {
            self.mesh.indices = super::Indices::U32(v);
        }

        self.mesh.vertex_count = total_vertices;

        Some(())
    }
}
