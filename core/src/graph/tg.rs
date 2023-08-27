use petgraph::stable_graph::NodeIndex;

pub enum Dependency {
    Noset,
    BeforeNode(String),
    AfterNode(String),
    Resource(String),
    Begin,
    End,
    And(Vec<Dependency>),
    Or(Vec<Dependency>),
}

pub struct TaskNode {}

pub struct TaskResource {}

type Graph = petgraph::graph::DiGraph<TaskNode, TaskResource, u32>;

pub struct TaskGraph {
    graph: Graph,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
        }
    }
}
