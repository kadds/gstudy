fn main() {
    let mut compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();

    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_2 as u32,
    );

    let dirs = std::fs::read_dir("src/shaders/").unwrap();
    for f in dirs {
        let f = f.unwrap().path();
        let path = f.as_path();
        let path_str = path.to_str().unwrap();
        let file_content = std::fs::read_to_string(path).unwrap();
        let ext = path.extension().unwrap().to_str().unwrap();
        let filename = path.file_stem().unwrap().to_str().unwrap();
        let shader_type = match ext {
            "vert" => shaderc::ShaderKind::Vertex,
            "frag" => shaderc::ShaderKind::Fragment,
            _ => {
                panic!("unknown type");
            }
        };
        let result = compiler
            .compile_into_spirv(&file_content, shader_type, path_str, "main", Some(&options))
            .unwrap();

        let bytes_u8 = result.as_binary_u8();
        let bytes = bytes_u8;

        let name = format!("src/compile_shaders/{}.{}", filename, ext);
        std::fs::write(&name, &bytes).unwrap();
        println!("cargo:rerun-if-changed={}", path_str);
    }
}
