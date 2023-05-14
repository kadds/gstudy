use std::{path::{Path, PathBuf}, collections::{HashMap, HashSet}};

use serde_derive::{Serialize, Deserialize};
// use shaderc::CompilationArtifact;

#[derive(Debug, Deserialize)]
struct Option {

}

#[derive(Debug, Deserialize)]
enum CameraType {
    D2,
    D2ScreenSize,
    D3,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum VariantUnit {
    Define(String),
    DefineKv(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct Variants {
    unit: HashMap<String, VariantUnit>,
    excludes: Vec<String>,
    exclusives: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Pass {
    name: String,
    path: String,
    binding: Vec<String>,
    variants: Variants,
    camera: CameraType,
}

#[derive(Debug, Deserialize)]
struct Config {
    option: Option,
    pass: Vec<Pass>,
}

struct Compiler {
    // c: shaderc::Compiler,
}

// fn artifact_to_shader(artifact: CompilationArtifact) -> tshader::Shader {
//     tshader::Shader {
//         entry: "main".into(),
//         bytes: artifact.as_binary_u8().into(),
//     }
// }

impl Compiler {
    pub fn new() -> Self {
        // let compiler = shaderc::Compiler::new().unwrap();
        // Self {
        //     c: compiler,
        // }
        Self {}
    }

    fn validate(&self, config: &mut Config, name: &str) -> anyhow::Result<()> {
        for pass in config.pass.iter_mut() {
            if pass.name.is_empty() {
                pass.name = name.to_owned()
            }
            for unit in &pass.variants.unit {
                if unit.0.len() != 1 {
                    anyhow::bail!("unit name should be single char");
                }
            }
        }
        Ok(())
    }


    pub fn compile<P: Into<PathBuf>, S: AsRef<str>>(&mut self, path: P, file: S, to: P) -> anyhow::Result<()>{
        // let p: PathBuf = path.into();
        // let file = file.as_ref();
        // let mut source_config = p.join(file);
        // source_config.set_extension("toml");

        // let source = std::fs::read_to_string(source_config)?;
        // let mut config: Config = toml::from_str(&source)?;
        // self.validate(&mut config, file)?;

        // for pass in &config.pass {
        //     let mut opt = shaderc::CompileOptions::new().ok_or(anyhow::anyhow!("init compile options"))?;
        //     opt.set_target_env(
        //         shaderc::TargetEnv::Vulkan,
        //         shaderc::EnvVersion::Vulkan1_2 as u32,
        //     );
        //     let mut pass_variants = HashSet::new();
        //     for exclude in &pass.variants.excludes {
        //         pass_variants.insert(exclude.to_owned());
        //     }

        //     // opt.add_macro_definition

        //     if !pass.vs.is_empty() {
        //         let source_text = std::fs::read_to_string(&pass.vs)?;
        //         let res = self.c.compile_into_spirv(&source_text, shaderc::ShaderKind::Vertex, &pass.vs, "main",Some(&opt))?;
        //         let vs = artifact_to_shader(res);
        //     }
        // }

        Ok(())
    }
}