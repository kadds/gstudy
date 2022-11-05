use std::{any::{TypeId, Any}, fmt::Debug, hash::Hash, sync::atomic::{AtomicU64, Ordering}};

pub trait Material: Any + Sync + Send + Debug{
    fn material_id(&self) -> Option<u64>;
    fn reset_material_id(&self, id: Option<u64>) -> Option<u64>;
}

pub fn downcast<T: Material>(obj: &dyn Material) -> &T {
    unsafe { & *(obj as *const dyn Material as *const T) }
}

pub trait MaterialParameter : Sync + Send + Debug + 'static{
}

#[derive(Debug)]
pub struct BMaterial<P> {
    p: P,
    id: AtomicU64,
}

impl<P> BMaterial<P> where P: MaterialParameter{
    pub fn new(p: P) -> Self {
        Self {
            p,
            id: AtomicU64::new(0),
        }
    }

    pub fn static_type_id() -> TypeId {
        TypeId::of::<Self>()
    }

    pub fn inner(&self) -> &P {
        &self.p
    }
}

impl<P> Material for BMaterial<P> where P: MaterialParameter {
    fn material_id(&self) -> Option<u64> {
        let val = self.id.load(Ordering::Acquire);
        if val == 0 {
            None
        } else {
            Some(val)
        }
    }

    fn reset_material_id(&self, id: Option<u64>) -> Option<u64> {
        let old = self.material_id();
        if let Some(id) = id {
            self.id.store(id, Ordering::Release);
        } else {
            self.id.store(0, Ordering::Release);
        }
        old
    }
}

#[derive(Debug, Default)]
pub struct BasicMaterialParameter {
    pub has_color: bool,
    pub has_texture: bool,
    pub has_normal: bool,
    pub line: bool,
}

impl BasicMaterialParameter {
    pub fn new() -> Self {
        Self {
            has_color: false,
            has_texture: false,
            has_normal: false,
            line: false,
        }
    }
}

impl MaterialParameter for BasicMaterialParameter {}

pub type BasicMaterial = BMaterial<BasicMaterialParameter>;


#[derive(Debug)]
pub struct DepthMaterialParameter {
    pub line: bool,
}

impl DepthMaterialParameter {
    pub fn new() -> Self {
        Self {
            line: false,
        }
    }
}

impl MaterialParameter for DepthMaterialParameter {}

pub type DepthMaterial = BMaterial<DepthMaterialParameter>;


#[derive(Debug)]
pub struct ConstantMaterialParameter {
    pub line: bool,
}

impl ConstantMaterialParameter {
    pub fn new() -> Self {
        Self {
            line: false,
        }
    }
}

impl MaterialParameter for ConstantMaterialParameter {}

pub type ConstantMaterial =  BMaterial<ConstantMaterialParameter>;
