pub mod backend;
pub mod pass;
pub mod present;
pub mod resource;
pub use pass::RenderPass;
pub use pass::RenderPassBuilder;
use petgraph::dot::Config;
use petgraph::dot::Dot;
use resource::{BufferInfo, Resource, ResourceId, ResourceOp, ResourceType, TextureInfo};

use std::any::Provider;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::{
    any::Any,
    borrow::BorrowMut,
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use petgraph::stable_graph::NodeIndex;

use crate::context::ResourceRef;
use crate::graph::rdg::pass::ClearPassBuilder;
use crate::{
    backends::wgpu_backend::WGPUResource,
    types::{Color, Size, Vec3f, Vec3u},
};

use self::pass::DynPass;
use self::{
    backend::GraphBackend,
    pass::PassRenderTargets,
    present::PresentNode,
    resource::{ClearValue, ImportTextureInfo, RT_COLOR_RESOURCE_ID, RT_DEPTH_RESOURCE_ID},
};

type Graph = petgraph::graph::DiGraph<Node, ResourceOp, u32>;

pub type NodeId = u32;

pub trait GraphContext {}

pub struct ResourceRegistry {
    map: HashMap<ResourceId, Resource>,
    single_map: HashMap<ResourceId, ResourceRef>,
}

impl ResourceRegistry {
    pub fn import_underlying(&mut self, id: ResourceId, underlying: ResourceRef) {
        self.single_map.insert(id, underlying);
    }

    pub fn get(&self, id: ResourceId) -> ResourceRef {
        self.single_map.get(&id).cloned().unwrap()
    }
    pub fn get_desc(&self, id: ResourceId) -> &Resource {
        self.map.get(&id).unwrap()
    }
    pub fn get_desc_and_underlying(&self, mut id: ResourceId) -> (&Resource, ResourceRef) {
        let mut desc = self.map.get(&id).unwrap();
        if let ResourceType::AliasResource(from, to) = desc.ty {
            id = to;
            desc = self.map.get(&id).unwrap();
        }
        let underlying = self.single_map.get(&id).cloned().unwrap();
        (desc, underlying)
    }
}

pub struct RenderGraph {
    name: String,
    g: Graph,
    passes: Vec<NodeIndex>,
    registry: ResourceRegistry,
}

impl RenderGraph {
    pub fn execute<F: FnMut(&Self, &Node), G: FnMut(&Node)>(
        &mut self,
        mut pre_f: F,
        mut post_f: G,
        backend: GraphBackend,
    ) {
        // create texture/buffer resource
        for (res_id, res) in &self.registry.map {
            let create = match &res.ty {
                ResourceType::Texture(t) => true,
                ResourceType::Buffer(b) => true,
                _ => false,
            };
            if create {
                let underlying = backend.create_resource(&res.ty);
                self.registry.single_map.insert(*res_id, underlying);
            }
        }
        {
            for index in &self.passes {
                let node = self.g.node_weight(*index).unwrap();
                pre_f(&self, node);

                if let Node::Pass(pass) = node {
                    pass.execute(&self.registry, &backend);
                }
                post_f(node);
            }
        }
        drop(backend);

        self.registry.single_map.clear();
    }

    pub fn registry(&mut self) -> &mut ResourceRegistry {
        &mut self.registry
    }
}

pub struct RenderGraphBuilder {
    name: String,
    resource_map: HashMap<ResourceId, Resource>,
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
        let resource = Resource {
            refs: 0,
            ty: ResourceType::Texture(TextureInfo {
                size: Vec3u::new(size.x, size.y, 1),
                format: wgpu::TextureFormat::Depth32Float,
                id: depth,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                clear: Some(ClearValue::Depth((1f32, 0))),
            }),
        };

        self.resource_map.insert(depth, resource);
        self.resource_map.insert(
            color,
            Resource {
                refs: 0,
                ty: ResourceType::ImportTexture(ImportTextureInfo {
                    id: color,
                    clear: clear.map(|v| ClearValue::Color(v)),
                }),
            },
        );

        let target = PassRenderTargets::new(color, depth);

        self.present_node = Some(PresentNode::new(target));
    }

    pub fn allocate_texture(
        &mut self,
        size: Vec3u,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        clear: Option<ClearValue>,
    ) -> ResourceId {
        let id = self.last_id;
        self.last_id += 1;
        let resource = Resource {
            refs: 0,
            ty: ResourceType::Texture(TextureInfo {
                size,
                format,
                id,
                clear,
                usage,
            }),
        };
        self.resource_map.insert(id, resource);
        id
    }

    pub fn alias_texture(&mut self, mut to: ResourceId) -> ResourceId {
        let id = self.last_id;
        self.last_id += 1;

        if let ResourceType::AliasResource(_, next_to) = self.resource_map.get(&to).unwrap().ty {
            to = next_to;
        }

        let resource = Resource {
            refs: 1,
            ty: ResourceType::AliasResource(id, to),
        };
        self.resource_map.insert(id, resource);
        id
    }

    pub fn allocate_buffer(&mut self, size: u64, usage: wgpu::BufferUsages) -> ResourceId {
        let id = self.last_id;
        self.last_id += 1;
        let resource = Resource {
            refs: 1,
            ty: ResourceType::Buffer(BufferInfo { size, id, usage }),
        };
        self.resource_map.insert(id, resource);
        id
    }

    pub fn import_texture(&mut self) -> ResourceId {
        let id = self.last_id;
        self.last_id += 1;
        let resource = Resource {
            refs: 1,
            ty: ResourceType::ImportBuffer(id),
        };
        self.resource_map.insert(id, resource);
        id
    }

    fn link_pass(
        &mut self,
        pass: Box<dyn DynPass>,
        g: &mut Graph,
        resource_nodes: &mut HashMap<ResourceId, NodeIndex>,
    ) {
        let (inputs, outputs) = pass.inputs_outputs();

        let inputs: Vec<(ResourceId, ResourceOp)> = inputs
            .textures
            .iter()
            .chain(inputs.buffers.iter())
            .cloned()
            .collect();
        let outputs: Vec<(u32, ResourceOp)> = outputs
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
                let node_id = g.add_node(Node::Resource(id));

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
                let node_id = g.add_node(Node::Resource(id));

                resource_nodes.insert(id, node_id);
                g.add_edge(index, node_id, op.clone());
            }
        }
    }

    pub fn compile(mut self) -> RenderGraph {
        let mut present = self.present_node.take().unwrap();
        let present_color = present.target().colors[0];
        let present_depth = present.target().depth.unwrap();

        let mut g = Graph::new();

        let mut back_rt_alias = Vec::new();
        let mut pass_nodes = Vec::new();
        std::mem::swap(&mut pass_nodes, &mut self.pass_nodes);

        loop {
            // pre process
            for node in &mut pass_nodes {
                match node {
                    Node::Pass(pass) => {
                        pass.connect_external(&mut self, present_color, present_depth);
                        let (inputs, outputs) = pass.inputs_outputs();
                        let mut rts = (None, None);
                        for item in outputs.textures.iter() {
                            let res = self.resource_map.get(&item.0).unwrap();
                            match res.ty {
                                ResourceType::AliasResource(from, to) => {
                                    if to == present_color {
                                        rts.0 = Some(from);
                                    } else if to == present_depth {
                                        rts.1 = Some(from);
                                    }
                                }
                                _ => (),
                            };
                        }
                        if rts.0.is_some() || rts.1.is_some() {
                            back_rt_alias.push(rts);
                        }
                    }
                    _ => {}
                }
            }

            if back_rt_alias.is_empty() {
                log::warn!("no present node linked, add default");
                pass_nodes.push(Node::Pass(Box::new(
                    ClearPassBuilder::new("default clear pass").build(),
                )));
                continue;
            }
            break;
        }

        let (last_color, last_depth) = back_rt_alias.last().unwrap();
        if let Some(c) = last_color {
            present.associate(&[*c]);
        }
        if let Some(d) = last_depth {
            present.associate(&[*d]);
        }

        let present_index = g.add_node(Node::Present(present));

        let mut resource_nodes = HashMap::new();

        // build graph
        for node in pass_nodes {
            match node {
                Node::Pass(pass) => {
                    self.link_pass(pass, &mut g, &mut resource_nodes);
                }
                _ => {}
            }
        }
        if let Some(c) = last_color {
            let frame_input_color = resource_nodes.get(c).unwrap();
            g.add_edge(
                *frame_input_color,
                present_index,
                ResourceOp::RenderTargetTextureRead,
            );
        }
        if let Some(d) = last_depth {
            let frame_input_depth = resource_nodes.get(d).unwrap();

            g.add_edge(
                *frame_input_depth,
                present_index,
                ResourceOp::RenderTargetTextureRead,
            );
        }

        if petgraph::algo::is_cyclic_directed(&g) {
            let gz = Dot::with_config(&g, &[Config::EdgeNoLabel]);
            panic!("cyclic detected in render graph {:?}", gz);
        }

        let mut inputs_nodes = VecDeque::new();
        inputs_nodes.push_back(present_index);

        let mut passes = Vec::new();
        let mut tested = HashSet::new();

        while !inputs_nodes.is_empty() {
            let node_index = inputs_nodes.pop_front().unwrap();
            if !tested.insert(node_index) {
                continue;
            }
            {
                let node = g.node_weight(node_index).unwrap();
                match node {
                    Node::Pass(_) => {
                        passes.push(node_index);
                    }
                    _ => (),
                }
            }
            for input in g.neighbors_directed(node_index, petgraph::Direction::Incoming) {
                inputs_nodes.push_back(input);
            }
        }

        passes.reverse();
        let gz = Dot::with_config(&g, &[Config::EdgeNoLabel]);
        log::info!("graph {:?} \n passes {:?}", gz, passes);

        let g = RenderGraph {
            name: self.name,
            g,
            passes,
            registry: ResourceRegistry {
                map: self.resource_map,
                single_map: HashMap::new(),
            },
        };
        g
    }
}

#[derive(Default, Debug)]
pub struct PassParameter {
    pub buffers: Vec<(ResourceId, ResourceOp)>,
    pub textures: Vec<(ResourceId, ResourceOp)>,
}

pub enum Node {
    Pass(Box<dyn DynPass>),
    Present(PresentNode),
    Resource(ResourceId),
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
