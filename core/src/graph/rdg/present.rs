use std::fmt::Debug;

use super::{pass::PassRenderTargets, resource::ResourceId};

pub struct PresentNode {
    render_target: PassRenderTargets,
    inputs: smallvec::SmallVec<[ResourceId; 2]>,
}

impl Debug for PresentNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresentNode")
            .field("inputs", &self.inputs)
            .finish()
    }
}

impl PresentNode {
    pub fn new(target: PassRenderTargets) -> Self {
        Self {
            render_target: target,
            inputs: smallvec::SmallVec::new(),
        }
    }

    pub(crate) fn target(&self) -> PassRenderTargets {
        self.render_target.clone()
    }

    pub(crate) fn inputs(&self) -> &[ResourceId] {
        &self.inputs
    }

    pub(crate) fn associate(&mut self, resources: &[ResourceId]) {
        self.inputs.extend_from_slice(resources)
    }
}
