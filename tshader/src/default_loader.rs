use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc};

use crate::{tech::ShaderTech, VariantFlags};

use crate::reflection::ShaderPassReflection;
use tshader_builder::compiler::ShaderTechCompiler;
use crate::{Pass, ShaderTechLoader};
use serde_derive::Deserialize;


#[derive(Debug, Deserialize)]
struct Desc {
    map: HashMap<String, String>,
}

pub struct DefaultShaderTechLoader {
    desc_config: Desc,
    desc_path: PathBuf,
}

impl ShaderTechLoader for DefaultShaderTechLoader {
    #[profiling::function]
    fn load(&self, device: &wgpu::Device, name: &str, variant: &VariantFlags) -> anyhow::Result<Arc<ShaderTech>> {
        let path_component = self
            .desc_config
            .map
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("shader tech {} not found", name))?;

        let base_path = PathBuf::from(&self.desc_path)
            .canonicalize()?
            .to_path_buf();

        let compiler = ShaderTechCompiler::new(path_component, base_path)?;

        let reflection = ShaderPassReflection::default();

        let total = compiler.total_pass();

        let mut pass_list = vec![];
        let mut pass_name_map = HashMap::new();

        for pass_index in 0..total {
            let pass: Arc<Pass> = reflection.reflect(device, variant, name, pass_index, &compiler)?;
            pass_name_map.insert(pass.name.clone(), pass_list.len());
            pass_list.push(pass);
        }

        let tech = ShaderTech {
            pass: pass_list,
            pass_name_map,
        };

        Ok(Arc::new(tech))
    }
}

impl DefaultShaderTechLoader {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let source_file = path.join("./desc.toml");
        let source = std::fs::read_to_string(source_file)?;
        Ok(Self {
            desc_config: toml::from_str(&source)?,
            desc_path: path,
        })
    }
}