use std::{collections::{HashMap, HashSet}, sync::Arc};

#[derive(Debug)]
pub struct Shader {
    pub device_module: Arc<wgpu::ShaderModule>,
    pub global_reference: HashSet<String>,
}

#[derive(Debug)]
pub enum Builtin {
    Position,
}

#[derive(Debug)]
pub struct InputBinding {
    pub binding: u32,
    pub builtin: Option<Builtin>,
    pub size: u32,
    pub alignment: u32,
    pub format: wgpu::VertexFormat,
}


#[derive(Debug, Clone)]
pub struct UniformSubVariable {
    pub size: u32,
    pub offset: u32,
}

#[derive(Debug, Clone)]
pub struct UniformStruct {
    pub group: u32,
    pub binding: u32,
    pub size: u32,
    pub group_name: String,
    pub sub_variables: HashMap<String, UniformSubVariable>,
}

#[derive(Debug, Clone)]
pub struct UniformTexture {
    pub group: u32,
    pub binding: u32,
    pub group_name: String,
    pub dimension: wgpu::TextureViewDimension,
    pub multisampled: bool,
    pub sample_type: wgpu::TextureSampleType,
}

#[derive(Debug, Clone)]
pub struct UniformSampler {
    pub group: u32,
    pub binding: u32,
    pub group_name: String,
    pub comparison: bool,
}

#[derive(Debug, Clone)]
pub struct PushConstant {
    pub size: u32,
    pub group_name: String,
}

#[derive(Debug, Clone)]
pub enum GlobalVariable {
    Struct(UniformStruct),
    Texture(UniformTexture),
    Sampler(UniformSampler),
    PushConstant(PushConstant),
}

#[derive(Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ResourcePosition {
    pub group: u32,
    pub binding: u32,
}

impl ResourcePosition {
    pub fn new(group: u32, binding: u32) -> Self {
        Self { group, binding }
    }
}

#[derive(Debug, Default)]
pub struct Pass {
    pub name: String,
    pub vs: Option<Shader>,
    pub fs: Option<Shader>,
    pub cs: Option<Shader>,

    // pub constants: Vec<wgpu::PushConstantRange>,
    // pub input_layout: BTreeMap<ResourcePosition, (bool, wgpu::VertexFormat)>,
    // pub bind_layout: BTreeMap<ResourcePosition, wgpu::BindGroupLayoutEntry>,

    pub local_variables: HashMap<String, InputBinding>, 
    pub global_variables: HashMap<String, GlobalVariable>,
}

pub struct ShaderTech {
    pub pass: Vec<Arc<Pass>>,
    pub pass_name_map: HashMap<String, usize>,
}

impl ShaderTech {
    pub fn get_pass(&self, name: &str) -> Option<Arc<Pass>> {
        self.pass_name_map.get(name).map(|idx| self.pass[*idx].clone())
    }
}

