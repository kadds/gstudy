use std::collections::HashMap;

use crate::scene::Transform;

mod sg;

pub type NodeId = u64;
type Graph<P> = petgraph::graph::DiGraph<Node<P>, (), u32>;

pub struct SceneGraph<P> {
    root: NodeId,
    last_node_id: NodeId,

    nodes: HashMap<NodeId, Box<Node<P>>>,
}

pub struct Node<P> {
    object: P,
    local_transform: Transform,
    word_transform: Transform,
    children: Vec<u64>,
}

impl<P> Node<P> {
    pub fn new(object: P) -> Self {
        Self {
            object,
            local_transform: Transform::default(),
            word_transform: Transform::default(),
            children: Vec::new(),
        }
    }
}

impl<P> SceneGraph<P> {
    pub fn root_node(&self) -> &Node<P> {
        self.nodes.get(&self.root).unwrap()
    }
    pub fn root_node_mut(&mut self) -> &mut Node<P> {
        self.nodes.get_mut(&self.root).unwrap()
    }

    pub fn add_to_root(&mut self, object: P) -> NodeId {
        let node_id = self.last_node_id;
        self.last_node_id += 1;
        self.nodes.insert(node_id, Box::new(Node::new(object)));

        let root_node = self.root_node_mut();
        root_node.children.push(node_id);

        node_id
    }

    pub fn add_to_node(&mut self, parent_node_id: NodeId, object: P) -> NodeId {
        let node_id = self.last_node_id;
        self.last_node_id += 1;
        self.nodes.insert(node_id, Box::new(Node::new(object)));

        let parent_node = self.nodes.get_mut(&parent_node_id).unwrap();
        parent_node.children.push(node_id);

        node_id
    }

    pub fn update(&mut self) {}
}
