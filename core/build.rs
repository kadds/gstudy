use shaderc::ShaderKind;
use std::path::{Path, PathBuf};

struct State {}

impl State {
    pub fn new() -> Self {
        Self {}
    }

    fn parse_combines(&self, content: &str) -> anyhow::Result<Vec<(String, Vec<String>)>> {
        let content = content.trim();
        let mut result = Vec::new();
        if content.starts_with("/// compile flags") {
            for line in content.lines().skip(1) {
                if !line.starts_with("///") {
                    break;
                }

                let mut kv = line[3..].splitn(2, ':');
                let key = kv
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("invalid compile flags, key not exist"))?
                    .trim();
                let value = kv
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("invalid compile flags, value not exist"))?
                    .trim();
                let value: Vec<String> = value
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                result.push((key.to_owned(), value));
            }
        }
        if result.is_empty() {
            result.push(("".to_owned(), Vec::new()));
        }
        Ok(result)
    }

    pub fn compile(
        &self,
        compiler: &mut shaderc::Compiler,
        shader_type: ShaderKind,
        filepath: &Path,
    ) -> anyhow::Result<()> {
        let file_content = std::fs::read_to_string(filepath)?;
        let path_str = filepath
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("filepath"))?;
        let compile_filename = path_str.replace("src/shaders/", "src/compile_shaders/");
        let stem = filepath.file_stem().unwrap().to_str().unwrap();
        let extension = filepath.extension().unwrap().to_str().unwrap();

        for (key, flags) in self.parse_combines(&file_content)? {
            let mut opt = shaderc::CompileOptions::new().unwrap();
            opt.set_target_env(
                shaderc::TargetEnv::Vulkan,
                shaderc::EnvVersion::Vulkan1_2 as u32,
            );
            for f in &flags {
                opt.add_macro_definition(f, None);
            }

            let result = compiler
                .compile_into_spirv(
                    &file_content,
                    shader_type,
                    filepath.to_str().unwrap(),
                    "main",
                    Some(&opt),
                )
                .map_err(|e| anyhow::anyhow!("with {:?} flags {:?} {}", key, flags, e))?;

            let bytes = result.as_binary_u8();
            let mut target_path = PathBuf::from(&compile_filename);
            if key.is_empty() {
                target_path.set_file_name(format!("{}.{}", stem, extension));
            } else {
                target_path.set_file_name(format!("{}_{}.{}", stem, key, extension));
            }

            let path = target_path.parent().unwrap();
            if !path.exists() {
                std::fs::create_dir_all(path)?;
            }

            std::fs::write(target_path, bytes)?;
        }
        Ok(())
    }
}

fn compile_shaders(path: &Path, compiler: &mut shaderc::Compiler, state: &mut State, deep: usize) {
    if deep > 8 {
        return;
    }

    let dirs = std::fs::read_dir(path).unwrap();
    for f in dirs {
        let f = f.unwrap().path();
        let path = f.as_path();
        if path.is_dir() {
            compile_shaders(path, compiler, state, deep + 1);
        } else {
            let ext = path.extension().unwrap().to_str().unwrap();
            let shader_type = match ext {
                "vert" => shaderc::ShaderKind::Vertex,
                "frag" => shaderc::ShaderKind::Fragment,
                _ => shaderc::ShaderKind::InferFromSource,
            };
            if let Err(e) = state.compile(compiler, shader_type, path) {
                panic!("{} {}", path.to_str().unwrap(), e);
            }
        }
    }
}

fn main() {
    let mut compiler = shaderc::Compiler::new().unwrap();
    let mut state = State::new();

    compile_shaders(Path::new("src/shaders/"), &mut compiler, &mut state, 0);
    println!("cargo:rerun-if-changed=src/shaders/");
}
