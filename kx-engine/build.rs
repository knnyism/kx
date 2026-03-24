use hassle_rs::{Dxc, DxcCompiler, DxcLibrary, HassleError};
use rspirv_reflect::{BindingCount, Reflection};
use std::{env, fs, path};

fn profile_to_stage_flags(profile: &str) -> u32 {
    match &profile[..2] {
        "cs" => 0x0000_0020, // COMPUTE
        "vs" => 0x0000_0001, // VERTEX
        "ps" => 0x0000_0010, // FRAGMENT
        "gs" => 0x0000_0008, // GEOMETRY
        "hs" => 0x0000_0002, // TESSELLATION_CONTROL
        "ds" => 0x0000_0004, // TESSELLATION_EVALUATION
        "ms" => 0x0000_0080, // MESH_EXT
        "as" => 0x0000_0040, // TASK_EXT
        _ => panic!("unknown shader profile: {profile}"),
    }
}

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
                "shader compilation failed for {}:\n{}",
                path.display(),
                error_string
            );
        }
    }
}

fn reflect_spv(spv: &[u8], profile: &str) -> Vec<u8> {
    let refl = Reflection::new_from_spirv(spv).expect("failed to reflect SPIR-V");

    let descriptor_sets = refl
        .get_descriptor_sets()
        .expect("failed to enumerate descriptor bindings");

    let push_constant = refl
        .get_push_constant_range()
        .expect("failed to enumerate push constants");

    let stage = profile_to_stage_flags(profile);

    let mut out = Vec::new();
    let write_u32 = |out: &mut Vec<u8>, val: u32| out.extend_from_slice(&val.to_le_bytes());

    write_u32(&mut out, stage);

    let total_bindings: u32 = descriptor_sets.values().map(|s| s.len() as u32).sum();
    write_u32(&mut out, total_bindings);

    for (&set, bindings) in &descriptor_sets {
        for (&binding, info) in bindings {
            write_u32(&mut out, set);
            write_u32(&mut out, binding);
            write_u32(&mut out, info.ty.0);

            let count = match info.binding_count {
                BindingCount::One => 1,
                BindingCount::StaticSized(n) => n as u32,
                BindingCount::Unbounded => 0, // 0 signals runtime array
            };
            write_u32(&mut out, count);
        }
    }

    match push_constant {
        Some(pc) => {
            write_u32(&mut out, 1);
            write_u32(&mut out, pc.offset);
            write_u32(&mut out, pc.size);
        }
        None => {
            write_u32(&mut out, 0);
        }
    }

    out
}

fn compile_shader(
    compiler: &DxcCompiler,
    library: &DxcLibrary,
    out_dir: &path::Path,
    shader_path: &path::Path,
    entry: &str,
    profile: &str,
    output_stem: &str,
) {
    let spv = compile_hlsl(compiler, library, shader_path, entry, profile)
        .unwrap_or_else(|e| panic!("failed to compile {}: {:?}", shader_path.display(), e));

    let meta = reflect_spv(&spv, profile);

    fs::write(out_dir.join(format!("{output_stem}.spv")), &spv).unwrap();
    fs::write(out_dir.join(format!("{output_stem}.meta")), &meta).unwrap();
}

fn main() {
    let dxc = Dxc::new(None).expect("failed to load DXC");

    let compiler = dxc.create_compiler().unwrap();
    let library = dxc.create_library().unwrap();

    let shader_dir = path::Path::new("shaders");
    let out_dir = path::PathBuf::from(env::var("OUT_DIR").unwrap());

    compile_shader(
        &compiler,
        &library,
        &out_dir,
        &shader_dir.join("clear.hlsl"),
        "main",
        "cs_6_0",
        "clear.cs",
    );

    println!("cargo::rerun-if-changed=shaders/");
}
