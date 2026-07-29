use cranelift_module::default_libcall_names;

pub fn make_object_module() -> Result<cranelift_object::ObjectModule, String> {
    let isa_builder = cranelift_native::builder().map_err(|e| e.to_string())?;
    let flags = cranelift_codegen::settings::Flags::new(cranelift_codegen::settings::builder());
    let isa = isa_builder.finish(flags).map_err(|e| e.to_string())?;
    let builder =
        cranelift_object::ObjectBuilder::new(isa, "avera_module", default_libcall_names())
            .map_err(|e| e.to_string())?;
    Ok(cranelift_object::ObjectModule::new(builder))
}

pub fn write_object(
    module: cranelift_object::ObjectModule,
    out_dir: &std::path::Path,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let product = module.finish();
    let obj_bytes = product.emit().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let path = out_dir.join(format!("{}.o", name));
    std::fs::write(&path, obj_bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn link_executable_with_rt(
    obj: &std::path::Path,
    rt_obj: &std::path::Path,
    out: &std::path::Path,
) -> Result<(), String> {
    link_internal(obj, &[rt_obj], out, &[])
}

pub fn link_executable(
    obj: &std::path::Path,
    out: &std::path::Path,
    extra_libs: &[String],
) -> Result<(), String> {
    link_internal(obj, &[], out, extra_libs)
}

fn link_internal(
    obj: &std::path::Path,
    extra_objs: &[&std::path::Path],
    out: &std::path::Path,
    libs: &[String],
) -> Result<(), String> {
    let linker = detect_linker().ok_or_else(|| "no system linker found".to_string())?;
    let mut cmd = std::process::Command::new(&linker);
    cmd.arg(obj);
    for o in extra_objs {
        cmd.arg(o);
    }
    cmd.arg("-o").arg(out);
    // Link the C standard library for FFI support.
    cmd.arg("-lc");
    for lib in libs {
        cmd.arg("-l").arg(lib);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("failed to invoke linker `{}`: {}", linker, e))?;
    if !output.status.success() {
        return Err(format!(
            "linker failed: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    Ok(())
}

fn detect_linker() -> Option<String> {
    for c in ["cc", "clang"] {
        if which(c).is_some() {
            return Some(c.to_string());
        }
    }
    None
}

fn which(cmd: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let full = dir.join(cmd);
            if full.is_file() {
                Some(full)
            } else {
                None
            }
        })
    })
}
