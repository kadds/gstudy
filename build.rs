use std::path::Path;
use vergen::*;

fn compile_shaders(
    path: &Path,
    compiler: &mut shaderc::Compiler,
    opt: &mut shaderc::CompileOptions,
) {
    let dirs = std::fs::read_dir(path).unwrap();
    for f in dirs {
        let f = f.unwrap().path();
        let path = f.as_path();
        if path.is_dir() {
            compile_shaders(path, compiler, opt);
        } else {
            println!("{:?}", path);
            let path_str = path.to_str().unwrap();
            let ext = path.extension().unwrap().to_str().unwrap();
            let shader_type = match ext {
                "vert" => shaderc::ShaderKind::Vertex,
                "frag" => shaderc::ShaderKind::Fragment,
                _ => shaderc::ShaderKind::InferFromSource,
            };
            let file_content = std::fs::read_to_string(path).unwrap();
            let result = compiler
                .compile_into_spirv(&file_content, shader_type, path_str, "main", Some(opt))
                .unwrap();

            let bytes_u8 = result.as_binary_u8();
            let bytes = bytes_u8;

            let compile_filename = path_str.replace("src/shaders/", "src/compile_shaders/");
            let path = Path::new(&compile_filename).parent().unwrap();
            if !path.exists() {
                std::fs::create_dir_all(path).unwrap();
            }
            std::fs::write(&compile_filename, &bytes).unwrap();
        }
    }
}

fn main() {
    let mut config = Config::default();
    *config.git_mut().sha_kind_mut() = ShaKind::Short;
    *config.build_mut().kind_mut() = TimestampKind::DateOnly;

    vergen(config).unwrap();

    let mut compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();

    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_2 as u32,
    );

    compile_shaders(Path::new("src/shaders/"), &mut compiler, &mut options);

    println!("cargo:rerun-if-changed=src/shaders/");
}
