pub mod backend;
pub mod pass;
pub mod present;
pub mod resource;
pub use pass::RenderPass;
pub use pass::RenderPassBuilder;
use petgraph::dot::Config;
use petgraph::dot::Dot;
use petgraph::unionfind::UnionFind;
use resource::{BufferInfo, ResourceId, ResourceType, ResourceUsage, TextureInfo};

use std::any::Any;
use std::fmt::Debug;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use petgraph::stable_graph::NodeIndex;

use crate::backends::wgpu_backend::ClearValue;
use crate::backends::wgpu_backend::ResourceOps;
use crate::context::ResourceRef;
use crate::graph::rdg::pass::ClearPassBuilder;
use crate::graph::rdg::pass::ColorRenderTargetDescriptor;
use crate::graph::rdg::pass::PreferAttachment;
use crate::graph::rdg::pass::RenderTargetDescriptor;
use crate::types::{Color, Size, Vec3u};

use self::pass::DynPass;
use self::pass::PassConstraint;
use self::pass::RenderTargetState;
use self::resource::ResourceNode;
use self::{
    backend::GraphBackend,
    present::PresentNode,
    resource::{ImportTextureInfo, RT_COLOR_RESOURCE_ID, RT_DEPTH_RESOURCE_ID},
};

type Graph = petgraph::graph::DiGraph<Node, ResourceUsage, u32>;
type SubGraph = petgraph::graphmap::DiGraphMap<NodeIndex, ()>;

pub type NodeId = u32;

pub trait GraphContext {}

pub struct ResourceRegistry {
    underlying_map: HashMap<ResourceId, ResourceRef>,
    desc_map: HashMap<ResourceId, Arc<ResourceNode>>,
}

impl ResourceRegistry {
    pub fn get(&self, id: ResourceId) -> ResourceRef {
        self.underlying_map.get(&id).cloned().unwrap()
    }

    pub fn get_underlying(&self, id: ResourceId) -> (ResourceRef, Arc<ResourceNode>) {
        let underlying = self.underlying_map.get(&id).cloned().unwrap();
        let desc = self.desc_map.get(&id).cloned().unwrap();
        (underlying, desc)
    }

    pub fn import(&mut self, id: ResourceId, res: ResourceRef) {
        self.underlying_map.insert(id, res);
    }
}

#[derive(Debug, Clone, Copy)]
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

impl Default for ResourceLifetime {
    fn default() -> Self {
        Self {
            beg: u32::MAX,
            end: u32::MIN,
        }
    }
}

pub struct RenderGraph {
    name: String,
    g: Graph,
    union_find: UnionFind<NodeIndex>,
    render_jobs_list: Vec<DependencyRenderJobs>,

    registry: ResourceRegistry,
}

impl RenderGraph {
    pub fn execute(&mut self, backend: GraphBackend, context: &dyn Any) {
        let main_jobs = &self.render_jobs_list[0];
        {
            for job in main_jobs.jobs.iter() {
                match job {
                    RenderJob::ResourceOperation(op) => match op {
                        ResourceLifetimeOperation::Create(id) => {
                            let res_desc = self.registry.desc_map.get(&id).unwrap();
                            let underlying = backend.create_resource(&res_desc.inner);
                            self.registry.underlying_map.insert(*id, underlying);
                        }
                        ResourceLifetimeOperation::Destroy(id) => {
                            let res = self.registry.underlying_map.get(id).unwrap();
                            backend.remove_resource(res.clone());
                            self.registry.underlying_map.remove(id);
                        }
                    },
                    RenderJob::PassCall((node_index, render_target_state)) => {
                        let node = self.g.node_weight(*node_index).unwrap();

                        if let Node::Pass(pass) = node {
                            pass.execute(
                                &self.registry,
                                &backend,
                                render_target_state.as_ref().unwrap(),
                                context,
                            );
                        }
                    }
                }
            }
        }

        for res in self.registry.underlying_map.values() {
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
    pass_name_nodes: HashMap<String, usize>,
    present_node: Option<PresentNode>,
    constraints: HashMap<String, Vec<PassConstraint>>,
}

impl RenderGraphBuilder {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            resource_map: HashMap::new(),
            last_id: RT_DEPTH_RESOURCE_ID + 1,
            pass_nodes: Vec::new(),
            pass_name_nodes: HashMap::new(),
            present_node: None,
            constraints: HashMap::new(),
        }
    }

    pub fn add_constraint<S: Into<String>>(&mut self, pass_node: S, c: PassConstraint) {
        let cs = self.constraints.entry(pass_node.into()).or_default();
        cs.push(c);
    }

    pub fn add_render_pass(&mut self, mut builder: RenderPassBuilder) {
        let n = self.pass_nodes.len();
        let mut tmp = vec![];
        std::mem::swap(&mut tmp, &mut builder.constraints.constraints);
        let name = builder.name.clone();
        self.pass_name_nodes.insert(name.clone(), n);
        self.pass_nodes.push(Node::Pass(Box::new(builder.build())));

        if tmp.len() > 0 {
            let cs = self.constraints.entry(name).or_default();
            cs.extend(tmp);
        }
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
                clear: clear.map(|v| ClearValue::Color(v)),
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

    pub fn alias_resource(&mut self, from: ResourceId) -> ResourceId {
        let id = self.last_id;
        self.last_id += 1;
        let resource = ResourceNode {
            id,
            name: "alias".to_owned(),
            inner: ResourceType::AliasResource(id, from),
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
        let mut pass_nodes = Vec::new();
        std::mem::swap(&mut pass_nodes, &mut self.pass_nodes);
        pass_nodes.reverse();

        let mut g = Graph::new();

        let present_index = g.add_node(Node::Present(present));

        // build graph
        let mut resource_nodes = HashMap::new();
        let mut pass_name_map = HashMap::new();

        for node in pass_nodes {
            match node {
                Node::Pass(pass) => {
                    let name = pass.name().to_owned();
                    let index = self.link_pass(pass, &mut g, &mut resource_nodes);
                    pass_name_map.insert(name, index);
                }
                _ => {}
            }
        }

        let mut main_subgraph = SubGraph::new();
        let first_dummy_node = NodeIndex::new(usize::MAX);
        let last_dummy_node = NodeIndex::new(usize::MAX - 1);
        // let first_dummy_node2 = NodeIndex::new(usize::MAX - 2);
        let last_dummy_node2 = NodeIndex::new(usize::MAX - 3);

        main_subgraph.add_node(first_dummy_node);
        main_subgraph.add_node(last_dummy_node);

        // main_subgraph.add_node(first_dummy_node2);
        main_subgraph.add_node(last_dummy_node2);

        // main_subgraph.add_edge(first_dummy_node, first_dummy_node2, ());
        main_subgraph.add_edge(last_dummy_node, last_dummy_node2, ());

        for node_index in g.node_indices() {
            let node = g.node_weight(node_index).unwrap();
            match node {
                Node::Pass(pass) => {
                    if !pass.inputs_outputs().2.has_default() {
                        continue;
                    }
                    if let Some(c) = pass.constraints() {
                        for c in &c.constraints {
                            match c {
                                PassConstraint::Before(name) => {
                                    if let Some(index) = pass_name_map.get(name) {
                                        main_subgraph.add_edge(node_index, *index, ());
                                    }
                                }
                                PassConstraint::After(name) => {
                                    if let Some(index) = pass_name_map.get(name) {
                                        main_subgraph.add_edge(*index, node_index, ());
                                    }
                                }
                                PassConstraint::First => {
                                    main_subgraph.add_edge(first_dummy_node, node_index, ());
                                }
                                PassConstraint::Last => {
                                    main_subgraph.add_edge(node_index, last_dummy_node, ());
                                }
                            }
                        }
                    }
                    main_subgraph.add_edge(node_index, last_dummy_node2, ());
                }
                _ => (),
            }
        }
        if petgraph::algo::is_cyclic_directed(&main_subgraph) {
            let gz = Dot::with_config(&main_subgraph, &[Config::EdgeNoLabel]);
            panic!("cyclic detected in subgraph {:?}", gz);
        }

        let mut main_pass_list = petgraph::algo::toposort(&main_subgraph, None).unwrap();

        let mut prev = None;
        main_pass_list.push(present_index);
        if main_pass_list.len() == 4 {
            // add clear pass
            let b = ClearPassBuilder::new("default clear")
                .render_target(RenderTargetDescriptor {
                    colors: smallvec::smallvec![ColorRenderTargetDescriptor {
                        prefer_attachment: PreferAttachment::Default,
                        ops: ResourceOps {
                            load: None,
                            store: true,
                        },
                    }],
                    depth: None,
                })
                .build();
            let index = g.add_node(Node::Pass(Box::new(b)));
            main_pass_list.insert(0, index);
        }

        // add clear pass
        // for (_, res) in &self.resource_map {
        //     match &res.inner {
        //         ResourceType::Texture(t) => {
        //             if let Some(colors) = &t.clear {
        //                 let mut desc = RenderTargetDescriptor {
        //                     colors: smallvec::smallvec![],
        //                     depth: None,
        //                 };
        //                 match colors {
        //                     ClearValue::Color(c) => desc.colors.push(ColorRenderTargetDescriptor {
        //                         prefer_attachment: pass::PreferAttachment::Resource(res.id),
        //                         ops: resource::ResourceOps {
        //                             load: Some(colors.clone()),
        //                             store: true,
        //                         },
        //                     }),
        //                     ClearValue::Depth(d) => {
        //                         desc.depth = Some(DepthRenderTargetDescriptor {
        //                             prefer_attachment: pass::PreferAttachment::Resource(res.id),
        //                             depth_ops: Some(resource::ResourceOps {
        //                                 load: Some(colors.clone()),
        //                                 store: true,
        //                             }),
        //                             stencil_ops: None,
        //                         })
        //                     }
        //                     ClearValue::Stencil(s) => {
        //                         desc.depth = Some(DepthRenderTargetDescriptor {
        //                             prefer_attachment: pass::PreferAttachment::Resource(res.id),
        //                             depth_ops: None,
        //                             stencil_ops: Some(resource::ResourceOps {
        //                                 load: Some(colors.clone()),
        //                                 store: true,
        //                             }),
        //                         })
        //                     }
        //                     ClearValue::DepthAndStencil((d, s)) => {
        //                         desc.depth = Some(DepthRenderTargetDescriptor {
        //                             prefer_attachment: pass::PreferAttachment::Resource(res.id),
        //                             depth_ops: Some(resource::ResourceOps {
        //                                 load: Some(colors.clone()),
        //                                 store: true,
        //                             }),
        //                             stencil_ops: Some(resource::ResourceOps {
        //                                 load: Some(colors.clone()),
        //                                 store: true,
        //                             }),
        //                         })
        //                     }
        //                 };
        //                 let index = g.add_node(Node::Pass(Box::new(
        //                     ClearPassBuilder::new(format!("clear resource {}", res.id))
        //                         .render_target(desc)
        //                         .build(),
        //                 )));
        //                 main_pass_list.insert(0, index);
        //             }
        //         }
        //         ResourceType::ImportTexture(t) => {}
        //         _ => (),
        //     }
        // }

        let mut connect = |from: NodeIndex, to: NodeIndex| {
            // create resource
            let resource_id = RT_COLOR_RESOURCE_ID;
            let resource_id2 = RT_DEPTH_RESOURCE_ID;

            let resource_color = g.add_node(Node::Resource(
                self.resource_map.get(&resource_id).cloned().unwrap(),
            ));
            let resource_depth = g.add_node(Node::Resource(
                self.resource_map.get(&resource_id2).cloned().unwrap(),
            ));

            g.add_edge(
                from,
                resource_color,
                ResourceUsage::RenderTargetTextureWrite,
            );
            g.add_edge(
                from,
                resource_depth,
                ResourceUsage::RenderTargetTextureWrite,
            );

            g.add_edge(resource_color, to, ResourceUsage::RenderTargetTextureRead);
            g.add_edge(resource_depth, to, ResourceUsage::RenderTargetTextureRead);
        };

        for node_index in &main_pass_list {
            if *node_index == first_dummy_node
                || *node_index == last_dummy_node
                || *node_index == last_dummy_node2
            {
                continue;
            }
            if prev.is_none() {
                prev = Some(*node_index);
                continue;
            }
            connect(prev.unwrap(), *node_index);
            prev = Some(*node_index);
        }

        if petgraph::algo::is_cyclic_directed(&g) {
            let gz = Dot::with_config(&g, &[Config::EdgeNoLabel]);
            panic!("cyclic detected in render graph {:?}", gz);
        }

        let mut union_find = UnionFind::new(g.node_count());
        for node_index in g.node_indices() {
            for n in g.neighbors(node_index) {
                union_find.union(node_index, n);
            }
        }

        let mut render_jobs_list = vec![];

        for node_index in petgraph::algo::toposort(&g, None).unwrap() {
            if !union_find.equiv(node_index, present_index) {
                continue;
            }
            let node = g.node_weight(node_index).unwrap();
            match node {
                Node::Pass(pass) => {
                    if render_jobs_list.len() == 0 {
                        render_jobs_list.push(DependencyRenderJobs::default());
                    }
                    render_jobs_list[0]
                        .jobs
                        .push(RenderJob::PassCall((node_index, None)));
                }
                _ => (),
            }
        }

        // lifetime
        let mut imported = HashSet::new();
        for jobs in &mut render_jobs_list {
            let mut resource_lifetime_map = HashMap::new();
            for (index, job) in jobs.jobs.iter().enumerate() {
                if let RenderJob::PassCall((node_index, _)) = job {
                    let resources = g.neighbors(*node_index);
                    for resource in resources {
                        if let Node::Resource(resource) = g.node_weight(resource).unwrap() {
                            match resource.inner {
                                ResourceType::ImportTexture(_) => {
                                    imported.insert(resource.id);
                                    ()
                                }
                                ResourceType::ImportBuffer(_) => {
                                    imported.insert(resource.id);
                                    ()
                                }
                                _ => (),
                            };
                            let l = resource_lifetime_map
                                .entry(resource.id)
                                .or_insert_with(|| ResourceLifetime::default());
                            l.add(ResourceLifetime {
                                beg: index as u32,
                                end: index as u32,
                            });
                        }
                    }
                }
            }

            let mut jobs_with_resource = vec![];
            let mut clears = vec![];

            for (index, render_job) in jobs.jobs.iter().enumerate() {
                for (res_id, res_lifetime) in &resource_lifetime_map {
                    if index as u32 == res_lifetime.beg {
                        clears.push(*res_id);
                        if imported.contains(res_id) {
                            continue;
                        }

                        jobs_with_resource.push(RenderJob::ResourceOperation(
                            ResourceLifetimeOperation::Create(*res_id),
                        ));
                    }
                }

                let mut job = *render_job;
                let job = match render_job {
                    RenderJob::PassCall((node_index, s)) => {
                        let node = g.node_weight(*node_index).unwrap();
                        let state = if let Node::Pass(pass) = &node {
                            let (_, _, desc) = pass.inputs_outputs();
                            Some(RenderTargetState::new(desc, &clears))
                        } else {
                            None
                        };
                        (*node_index, state)
                    }
                    _ => panic!("unexpected"),
                };

                jobs_with_resource.push(RenderJob::PassCall(job));

                for (res_id, res_lifetime) in &resource_lifetime_map {
                    if index as u32 == res_lifetime.end {
                        if imported.contains(res_id) {
                            continue;
                        }
                        jobs_with_resource.push(RenderJob::ResourceOperation(
                            ResourceLifetimeOperation::Destroy(*res_id),
                        ));
                    }
                }

                clears.clear();
            }

            jobs.jobs = jobs_with_resource;
        }

        let gz = Dot::with_config(&g, &[Config::EdgeNoLabel]);
        log::info!("graph {:?}", gz);
        log::info!("render jobs {:?}", render_jobs_list);

        RenderGraph {
            name: self.name,
            g,
            render_jobs_list,
            union_find,
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

#[derive(Debug, Copy, Clone)]
enum RenderJob {
    ResourceOperation(ResourceLifetimeOperation),
    PassCall((NodeIndex, Option<RenderTargetState>)),
}

#[derive(Debug, Default)]
struct DependencyRenderJobs {
    jobs: Vec<RenderJob>,
    depends_on_jobs: Vec<u32>,
}
