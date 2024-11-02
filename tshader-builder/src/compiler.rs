use std::{collections::{HashMap, HashSet}, path::PathBuf, str::FromStr};

use serde_derive::Deserialize;
use strum::{Display, EnumIter, EnumString};

use crate::preprocessor::{Preprocessor, PreprocessorConfig, Variable};

#[derive(Debug, Deserialize)]
pub struct Tech {
    pub author: String,
}

#[derive(Debug, Deserialize)]
pub enum CameraType {
    D2,
    D2ScreenSize,
    D3,
}

#[derive(Debug, Deserialize)]
pub struct Variants {
    pub unit: Vec<String>,
    pub excludes: Vec<String>,
    pub exclusives: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Pass {
    pub name: String,
    pub index: i32,
    pub source: String,
    pub binding: Vec<String>,
    pub variants: Option<Variants>,
    pub camera: CameraType,
    pub shaders: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub tech: Tech,
    pub pass: Vec<Pass>,
}

#[derive(Debug, Clone, Copy, Display, EnumIter, EnumString, PartialEq, Eq, Hash)]
#[repr(u8)]
#[strum(serialize_all = "snake_case", use_phf)]
pub enum Shader {
    #[strum(serialize = "vs")]
    Vertex,
    #[strum(serialize = "fs")]
    Fragment,
    #[strum(serialize = "cs")]
    Compute,
}

pub struct PassShaderSourceDescriptor {
    pub name: String,
    pub source: String,
    pub entries: Vec<Shader>,
    pub struct_variables: HashMap<String, HashMap<String, Variable>>,
    pub global_variables: HashMap<String, HashMap<String, Variable>>,
}

#[derive(Debug)]
pub struct ShaderTechCompiler {
    config: Config,
    base_path: PathBuf,
    include_base_path: PathBuf,
    source: String,
}

impl ShaderTechCompiler {
    pub fn new<P: Into<PathBuf>>(source: &str, base_path: P) -> anyhow::Result<Self> {
        let p: PathBuf = base_path.into();
        let mut source_path = p.join(source);
        source_path.set_extension("toml");

        let source =
            std::fs::read_to_string(&source_path).map_err(|e| anyhow::anyhow!("{} {:?}", e, source_path))?;

        let config = {
            let mut config: Config = toml::from_str(&source)?;
            config.pass.sort_by_key(|k| k.index);
            config
        };

        Ok(Self {
            config,
            source: source_path.to_str().unwrap().to_owned(),
            include_base_path: p,
            base_path: source_path.canonicalize()?.parent().unwrap().to_path_buf(),
        })
    }

    pub fn total_pass(&self) -> usize {
        self.config.pass.len()
    }

    pub fn compile_pass<S: AsRef<str>>(
        &self,
        pass_index: usize,
        variants: &[S],
    ) -> anyhow::Result<PassShaderSourceDescriptor> {
        profiling::scope!("compile pass");
        let mut cfg = PreprocessorConfig::default();

        let mut set = HashSet::new();

        if let Some(variants) = &self.config.pass[pass_index].variants {
            for unit in &variants.unit {
                set.insert(unit.to_owned());
            }
        }

        for variant in variants {
            if !set.contains(variant.as_ref()) {
                return Err(anyhow::anyhow!(
                    "variant {} not exists in pass {}",
                    variant.as_ref(),
                    pass_index
                ));
            }
            cfg = cfg.with_define(variant.as_ref().to_string(), "True");
        }
        cfg = cfg.with_include(self.base_path.to_str().unwrap());
        cfg = cfg.with_include(self.include_base_path.to_str().unwrap());

        let preprocessor = Preprocessor::new(cfg);
        let real_path = self.base_path.join(&self.config.pass[pass_index].source);

        let res = preprocessor.process(real_path.as_os_str().to_str().unwrap())?;

        log::debug!(
            "compile {} pass {} {:?} success: \n{}",
            self.source,
            pass_index,
            set,
            res.data
        );
        let shaders = self.config.pass[pass_index]
            .shaders
            .iter()
            .map(|v| Shader::from_str(v))
            .collect::<Result<Vec<_>, strum::ParseError>>()?;

        Ok(PassShaderSourceDescriptor {
            source: res.data,
            struct_variables: res.loc_struct_map,
            global_variables: res.loc_global_map,
            entries: shaders,
            name: self.config.pass[pass_index].name.clone(),
        })
    }

    pub fn npass(&self) -> usize {
        self.config.pass.len()
    }
}
