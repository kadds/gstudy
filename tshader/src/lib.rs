use std::{path::{Path, PathBuf}, collections::HashMap};
pub mod cache;

use bincode::{Encode, Decode};

#[derive(Encode, Decode, Debug)]
pub struct Shader {
    pub entry: String,
    pub bytes: Vec<u8>,
}

#[derive(Encode, Decode, Debug)]
pub enum ShaderBindingType {
    Buffer(u32),
    Sampler,
    Texture,
    Storage(),
}

#[derive(Encode, Decode, Debug)]
pub struct InputLayout {

}

#[derive(Encode, Decode, Debug)]
pub struct InputAttribute {
    pub offset: u32,
    pub format: u64,
}

#[derive(Encode, Decode, Debug)]
pub struct Pass {
    pub name: String,

    pub vs: Option<Shader>,
    pub fs: Option<Shader>,
    pub cs: Option<Shader>,
}

#[derive(Encode, Decode, Debug)]
pub struct TShader {
    pub name: String,
    pub desc: String,
    pub author: String,
    pub variants: HashMap<String, Vec<Pass>>,
}

pub struct ShaderTechIdentity {
    pub name: String,
}

pub struct Loader {
    path: PathBuf,
}

impl Loader {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path
        }
    }

    fn load<S: AsRef<str>>(&self, name: S) -> anyhow::Result<TShader> {
        let config = bincode::config::standard();
        let file = std::fs::File::open(self.path.join(name.as_ref()))?;
        let mut reader = std::io::BufReader::new(file);
        let tshader = bincode::decode_from_reader(&mut reader, config)?;
        Ok(tshader)
    }
}