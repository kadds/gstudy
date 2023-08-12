

struct Epoch {
    last_id: u64,
}

pub struct EpochObjectPool {
    current: Epoch,
    used: Epoch,
}

impl EpochObjectPool {
    pub fn gc(&self) {

    }
}