use hassle_rs::{Dxc, DxcCompiler, DxcLibrary, HassleError};
use std::{env, fs, path};

fn compile_hlsl(
    compiler: &DxcCompiler,
    library: &DxcLibrary,
    path: &path::Path,
    entry: &str,
    profile: &str,
) -> Result<Vec<u8>, HassleError> {
    let source = fs::read_to_string(path).unwrap();
    let blob = library.create_blob_with_encoding_from_str(&source)?;

    let result = compiler.compile(
        &blob,
        path.file_name().unwrap().to_str().unwrap(),
        entry,
        profile,
        &["-spirv", "-fspv-target-env=vulkan1.3"],
        None,
        &[],
    );

    match result {
        Ok(r) => {
            let code = r.get_result()?.to_vec();
            Ok(code)
        }
        Err(r) => {
            let error_blob = r.0.get_error_buffer()?;
            let error_string = library.get_blob_as_string(&error_blob.into())?;
            panic!(
                "Shader compilation failed for {}:\n{}",
                path.display(),
                error_string
            );
        }
    }
}

fn main() {
    let dxc = Dxc::new(None).expect("failed to load DXC");

    let compiler = dxc.create_compiler().unwrap();
    let library = dxc.create_library().unwrap();

    let shader_dir = path::Path::new("shaders");
    let out_dir = env::var("OUT_DIR").unwrap();

    let spv = compile_hlsl(
        &compiler,
        &library,
        &shader_dir.join("clear.hlsl"),
        "main",
        "cs_6_0",
    )
    .expect("failed to compile clear.hlsl");

    fs::write(path::Path::new(&out_dir).join("clear.cs.spv"), &spv).unwrap();

    println!("cargo::rerun-if-changed=shaders/");
}
