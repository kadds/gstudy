pub mod backend;
pub mod pass;
pub mod present;
pub mod resource;
pub use pass::RenderPass;
pub use pass::RenderPassBuilder;
use petgraph::dot::Config;
use petgraph::dot::Dot;
use resource::{BufferInfo, ResourceId, ResourceType, ResourceUsage, TextureInfo};

use std::collections::LinkedList;
use std::fmt::Debug;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use petgraph::stable_graph::NodeIndex;

use crate::context::ResourceRef;
use crate::graph::rdg::pass::PreferAttachment;
use crate::types::{Color, Size, Vec3f, Vec3u};

use self::pass::DynPass;
use self::resource::ResourceNode;
use self::{
    backend::GraphBackend,
    present::PresentNode,
    resource::{ClearValue, ImportTextureInfo, RT_COLOR_RESOURCE_ID, RT_DEPTH_RESOURCE_ID},
};

type Graph = petgraph::graph::DiGraph<Node, ResourceUsage, u32>;

pub type NodeId = u32;

pub trait GraphContext {}

pub struct ResourceRegistry {
    underlying_map: HashMap<ResourceId, ResourceRef>,
    desc_map: HashMap<ResourceId, Arc<ResourceNode>>,
}

pub struct ResourceStateMap {
    has_clear_map: HashSet<ResourceId>,
}

impl ResourceRegistry {
    pub fn get(&self, id: ResourceId) -> ResourceRef {
        self.underlying_map.get(&id).cloned().unwrap()
    }

    pub fn get_underlying(&self, mut id: ResourceId) -> (ResourceRef, Arc<ResourceNode>) {
        let underlying = self.underlying_map.get(&id).cloned().unwrap();
        let desc = self.desc_map.get(&id).cloned().unwrap();
        (underlying, desc)
    }

    pub fn import(&mut self, id: ResourceId, res: ResourceRef) {
        self.underlying_map.insert(id, res);
    }
}

impl ResourceStateMap {
    pub fn has_clear(&mut self, id: ResourceId) -> bool {
        let res = self.has_clear_map.contains(&id);
        self.has_clear_map.insert(id);
        res
    }
}

#[derive(Debug)]
enum ResourceLifetimeOperation {
    Create(ResourceId),
    Destroy(ResourceId),
}

#[derive(Debug)]
struct ResourceLifetime {
    pub beg: u32,
    pub end: u32,
}

impl ResourceLifetime {
    pub fn add(&mut self, r: Self) {
        self.beg = self.beg.min(r.beg);
        self.end = self.end.max(r.end);
    }
}

pub struct RenderGraph {
    name: String,
    g: Graph,
    passes: Vec<(NodeIndex, Vec<ResourceLifetimeOperation>)>,
    registry: ResourceRegistry,
}

impl RenderGraph {
    pub fn execute<F: FnMut(&Self, &Node), G: FnMut(&Node)>(
        &mut self,
        mut pre_f: F,
        mut post_f: G,
        backend: GraphBackend,
    ) {
        let mut state = ResourceStateMap {
            has_clear_map: HashSet::new(),
        };
        {
            for (pass_node, resource_ops) in self.passes.iter() {
                // create texture/buffer resource
                for resource_lifetime in resource_ops.iter() {
                    match resource_lifetime {
                        ResourceLifetimeOperation::Create(id) => {
                            let res_desc = self.registry.desc_map.get(&id).unwrap();
                            let underlying = backend.create_resource(&res_desc.inner);
                            self.registry.underlying_map.insert(*id, underlying);
                        }
                        ResourceLifetimeOperation::Destroy(id) => {
                            self.registry.underlying_map.remove(id);
                        }
                    }
                }
                let node = self.g.node_weight(*pass_node).unwrap();
                pre_f(self, node);

                if let Node::Pass(pass) = node {
                    pass.execute(&self.registry, &backend, &mut state);
                }
                post_f(node);
            }
        }
        for (id, res) in &self.registry.underlying_map {
            backend.remove_resource(res.clone())
        }
        drop(backend);

        self.registry.underlying_map.clear();
    }

    pub fn registry(&mut self) -> &mut ResourceRegistry {
        &mut self.registry
    }
}

pub struct RenderGraphBuilder {
    name: String,
    resource_map: HashMap<ResourceId, Arc<ResourceNode>>,
    last_id: u32,
    pass_nodes: Vec<Node>,
    present_node: Option<PresentNode>,
}

impl RenderGraphBuilder {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            resource_map: HashMap::new(),
            last_id: RT_DEPTH_RESOURCE_ID + 1,
            pass_nodes: Vec::new(),
            present_node: None,
        }
    }

    pub fn add_render_pass(&mut self, pass: RenderPass) {
        self.pass_nodes.push(Node::Pass(Box::new(pass)));
    }

    pub fn set_present_target(
        &mut self,
        size: Size,
        format: wgpu::TextureFormat,
        clear: Option<Color>,
    ) {
        let color = RT_COLOR_RESOURCE_ID;
        let depth = RT_DEPTH_RESOURCE_ID;
        let resource = ResourceNode {
            id: depth,
            name: "back_depth".to_owned(),
            inner: ResourceType::Texture(TextureInfo {
                size: Vec3u::new(size.x, size.y, 1),
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                clear: Some(ClearValue::Depth(1f32)),
            }),
        };

        self.resource_map.insert(depth, resource.into());

        let color_resource = Arc::new(ResourceNode {
            id: color,
            name: "back_buffer".to_owned(),
            inner: ResourceType::ImportTexture(ImportTextureInfo {
                clear: clear.map(ClearValue::Color),
            }),
        });

        self.resource_map.insert(color, color_resource);

        self.present_node = Some(PresentNode::new(color));
    }

    pub fn allocate_texture(
        &mut self,
        name: String,
        size: Vec3u,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        clear: Option<ClearValue>,
    ) -> ResourceId {
        let id = self.last_id;
        self.last_id += 1;
        let resource = ResourceNode {
            id,
            name,
            inner: ResourceType::Texture(TextureInfo {
                size,
                format,
                clear,
                usage,
            }),
        };
        self.resource_map.insert(id, resource.into());
        id
    }

    // pub fn alias_texture(&mut self, mut to: ResourceId) -> ResourceId {
    //     let id = self.last_id;
    //     self.last_id += 1;

    //     if let ResourceType::AliasResource(_, next_to) = self.resource_map.get(&to).unwrap().ty {
    //         to = next_to;
    //     }

    //     let resource = Resource {
    //         refs: 1,
    //         ty: ResourceType::AliasResource(id, to),
    //     };
    //     self.resource_map.insert(id, resource);
    //     id
    // }

    pub fn allocate_buffer(
        &mut self,
        name: String,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> ResourceId {
        let id = self.last_id;
        self.last_id += 1;
        let resource = ResourceNode {
            id,
            name,
            inner: ResourceType::Buffer(BufferInfo { size, usage }),
        };
        self.resource_map.insert(id, resource.into());
        id
    }

    pub fn import_texture(&mut self, name: &str) -> ResourceId {
        let id = self.last_id;
        self.last_id += 1;
        let resource = ResourceNode {
            id,
            name: name.to_owned(),
            inner: ResourceType::ImportTexture(ImportTextureInfo { clear: None }),
        };
        self.resource_map.insert(id, resource.into());
        id
    }

    fn link_pass(
        &mut self,
        pass: Box<dyn DynPass>,
        g: &mut Graph,
        resource_nodes: &mut HashMap<ResourceId, NodeIndex>,
    ) -> NodeIndex {
        let (inputs, outputs, _) = pass.inputs_outputs();

        let inputs: Vec<(ResourceId, ResourceUsage)> = inputs
            .textures
            .iter()
            .chain(inputs.buffers.iter())
            .cloned()
            .collect();
        let outputs: Vec<(u32, ResourceUsage)> = outputs
            .textures
            .iter()
            .chain(outputs.buffers.iter())
            .cloned()
            .collect();

        // link input nodes

        let index = g.add_node(Node::Pass(pass));

        for (id, op) in inputs {
            let node_id = resource_nodes.get(&id);
            if let Some(node_id) = node_id {
                g.add_edge(*node_id, index, op.clone());
            } else {
                let res = self.resource_map.get(&id).cloned().unwrap();
                let node_id = g.add_node(Node::Resource(res));

                resource_nodes.insert(id, node_id);
                g.add_edge(node_id, index, op.clone());
            }
        }

        // link output nodes
        for (id, op) in outputs {
            let node_id = resource_nodes.get(&id);
            if let Some(node_id) = node_id {
                g.add_edge(index, *node_id, op.clone());
            } else {
                let res = self.resource_map.get(&id).cloned().unwrap();
                let node_id = g.add_node(Node::Resource(res));

                resource_nodes.insert(id, node_id);
                g.add_edge(index, node_id, op.clone());
            }
        }
        index
    }

    pub fn compile(mut self) -> RenderGraph {
        let present = self.present_node.take().unwrap();

        let mut g = Graph::new();

        let mut pass_nodes = Vec::new();
        std::mem::swap(&mut pass_nodes, &mut self.pass_nodes);
        pass_nodes.reverse();

        let present_index = g.add_node(Node::Present(present));
        let back_buffer_index = g.add_node(Node::Resource(
            self.resource_map
                .get(&RT_COLOR_RESOURCE_ID)
                .cloned()
                .unwrap(),
        ));
        g.add_edge(back_buffer_index, present_index, ResourceUsage::TextureRead);

        // build graph

        let mut resource_nodes = HashMap::new();
        let mut has_new = false;
        let mut passes = LinkedList::new();

        for node in pass_nodes {
            match node {
                Node::Pass(pass) => {
                    let index = if !has_new && pass.is_default_render_target(RT_COLOR_RESOURCE_ID) {
                        has_new = true;
                        let index = self.link_pass(pass, &mut g, &mut resource_nodes);
                        g.add_edge(index, back_buffer_index, ResourceUsage::TextureWrite);
                        index
                    } else {
                        self.link_pass(pass, &mut g, &mut resource_nodes)
                    };
                    passes.push_back(index);
                }
                _ => {}
            }
        }

        let mut usage_resources: HashMap<ResourceId, ResourceLifetime> = HashMap::new();
        for (index, node_index) in passes.iter().enumerate() {
            let node = g.node_weight(*node_index).unwrap();
            match node {
                Node::Pass(pass) => {
                    let (inputs, outputs, render_target) = pass.inputs_outputs();
                    for (resource, _) in inputs.textures.iter().chain(outputs.textures.iter()) {
                        usage_resources
                            .entry(*resource)
                            .and_modify(|value| {
                                value.add(ResourceLifetime {
                                    beg: index as u32,
                                    end: index as u32,
                                })
                            })
                            .or_insert_with(|| ResourceLifetime {
                                beg: index as u32,
                                end: index as u32,
                            });
                    }

                    for (resource, _) in inputs.buffers.iter().chain(outputs.buffers.iter()) {
                        usage_resources
                            .entry(*resource)
                            .and_modify(|value| {
                                value.add(ResourceLifetime {
                                    beg: index as u32,
                                    end: index as u32,
                                })
                            })
                            .or_insert_with(|| ResourceLifetime {
                                beg: index as u32,
                                end: index as u32,
                            });
                    }

                    if let Some(depth) = &render_target.depth {
                        if let PreferAttachment::Default = depth.prefer_attachment {
                            usage_resources
                                .entry(RT_DEPTH_RESOURCE_ID)
                                .and_modify(|value| {
                                    value.add(ResourceLifetime {
                                        beg: index as u32,
                                        end: index as u32,
                                    })
                                })
                                .or_insert_with(|| ResourceLifetime {
                                    beg: index as u32,
                                    end: index as u32,
                                });
                        }
                    }
                }
                _ => {}
            }
        }

        if petgraph::algo::is_cyclic_directed(&g) {
            let gz = Dot::with_config(&g, &[Config::EdgeNoLabel]);
            panic!("cyclic detected in render graph {:?}", gz);
        }
        log::info!("{:?}", usage_resources);

        let mut passes_vec = vec![];
        passes_vec.push((present_index, vec![]));

        for (index, node_index) in passes.iter().enumerate() {
            let node = g.node_weight(*node_index).unwrap();
            match node {
                Node::Pass(_) => {
                    let mut lifetime_ops = vec![];
                    let mut next_destroy_resource = vec![];
                    for (res_id, lifetime) in &usage_resources {
                        if lifetime.end == index as u32 {
                            lifetime_ops.push(ResourceLifetimeOperation::Create(*res_id));
                        }
                        if lifetime.beg == index as u32 {
                            next_destroy_resource.push(ResourceLifetimeOperation::Destroy(*res_id));
                        }
                    }
                    passes_vec.last_mut().unwrap().1 = next_destroy_resource;
                    passes_vec.push((*node_index, lifetime_ops));
                }
                _ => (),
            }
        }

        passes_vec.reverse();

        let gz = Dot::with_config(&g, &[Config::EdgeNoLabel]);
        log::info!("graph {:?} \n passes {:?}", gz, passes_vec);

        RenderGraph {
            name: self.name,
            g,
            passes: passes_vec,
            registry: ResourceRegistry {
                desc_map: self.resource_map,
                underlying_map: HashMap::new(),
            },
        }
    }
}

#[derive(Default, Debug)]
pub struct PassParameter {
    pub buffers: Vec<(ResourceId, ResourceUsage)>,
    pub textures: Vec<(ResourceId, ResourceUsage)>,
}

pub enum Node {
    Pass(Box<dyn DynPass>),
    Present(PresentNode),
    Resource(Arc<ResourceNode>),
}

impl Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pass(arg0) => f.debug_tuple("Pass").field(arg0).finish(),
            Self::Present(arg0) => f.debug_tuple("Present").field(arg0).finish(),
            Self::Resource(arg0) => f.debug_tuple("Resource").field(arg0).finish(),
        }
    }
}
