use std::fmt::Debug;

use super::resource::ResourceId;

pub struct PresentNode {
    target: [ResourceId; 1],
}

impl Debug for PresentNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresentNode")
            .field("target", &self.target)
            .finish()
    }
}

impl PresentNode {
    pub fn new(target: ResourceId) -> Self {
        Self { target: [target] }
    }

    pub(crate) fn target(&self) -> ResourceId {
        self.target[0]
    }

    pub(crate) fn inputs(&self) -> &[ResourceId] {
        &self.target
    }
}
