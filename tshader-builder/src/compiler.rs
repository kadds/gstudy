use itertools::Itertools;
use std::{path::PathBuf, str::FromStr};

use serde_derive::Deserialize;
use strum::{Display, EnumIter, EnumString};

use crate::preprocessor::{Preprocessor, PreprocessorConfig};

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
pub enum Variant {
    #[strum(serialize = "TEXTURE_COLOR")]
    TextureColor,
    #[strum(serialize = "NORMAL_TEX")]
    NormalTex,
    #[strum(serialize = "HEIGHT_TEX")]
    HeightTex,
    #[strum(serialize = "EMISSION_TEX")]
    EmissionTex,
    #[strum(serialize = "VERTEX_COLOR")]
    VertexColor,
    #[strum(serialize = "ALPHA_TEST")]
    AlphaTest,
}

impl Variant {
    pub fn need_sampler(&self) -> bool {
        match self {
            Variant::TextureColor => true,
            Variant::NormalTex => true,
            Variant::HeightTex => true,
            Variant::EmissionTex => true,
            Variant::VertexColor => false,
            Variant::AlphaTest => false,
        }
    }
}

pub fn variants_name<S: Into<Vec<Variant>>>(variants: S) -> String {
    let mut variants: Vec<Variant> = variants.into();
    variants.sort_by_key(|v| *v as u8);
    variants.iter().map(|v| v.to_string()).join(",")
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
    pub source: String,
    pub include_shaders: Vec<Shader>,
}

#[derive(Debug)]
pub struct ShaderTechCompiler {
    config: Config,
    base_path: PathBuf,
    include_base_path: PathBuf,
}

impl ShaderTechCompiler {
    pub fn new<P: Into<PathBuf>>(source: &str, base_path: P) -> anyhow::Result<Self> {
        let p: PathBuf = base_path.into();
        let mut source_path = p.join(source);
        source_path.set_extension("toml");

        let source =
            std::fs::read_to_string(&source_path).map_err(|e| anyhow::anyhow!("{} {:?}", e, p))?;
        let mut config: Config = toml::from_str(&source)?;
        config.pass.sort_by_key(|k| k.index);

        Ok(Self {
            config,
            include_base_path: p,
            base_path: source_path.canonicalize()?.parent().unwrap().to_path_buf(),
        })
    }

    pub fn compile_pass(
        &self,
        pass_index: usize,
        variants: &[Variant],
    ) -> anyhow::Result<PassShaderSourceDescriptor> {
        let mut cfg = PreprocessorConfig::default();

        for variant in variants {
            cfg = cfg.with_define(variant.to_string(), "True");
        }
        cfg = cfg.with_include(self.base_path.to_str().unwrap());
        cfg = cfg.with_include(self.include_base_path.to_str().unwrap());

        let preprocessor = Preprocessor::new(cfg);
        let real_path = self.base_path.join(&self.config.pass[pass_index].source);

        let res = preprocessor.process(real_path.as_os_str().to_str().unwrap())?;

        let shaders = self.config.pass[pass_index]
            .shaders
            .iter()
            .map(|v| Shader::from_str(v))
            .collect::<Result<Vec<_>, strum::ParseError>>()?;

        Ok(PassShaderSourceDescriptor {
            source: res,
            include_shaders: shaders,
        })
    }

    pub fn npass(&self) -> usize {
        self.config.pass.len()
    }
}
