pub mod abi;
pub mod cranelift;
pub mod layout;
pub mod object;
mod validate;

pub use object::{link_executable, write_object};

pub fn compile_bodies(
    module: &mut cranelift_object::ObjectModule,
    bodies: &[crate::mir::body::Body],
    cx: &crate::types::intern::TyCtxt,
    defs: &crate::resolve::defs::Defs,
) -> Result<std::collections::HashMap<String, cranelift_module::FuncId>, String> {
    validate::validate_codegen_bodies(bodies)?;
    cranelift::compile_bodies(module, bodies, cx, defs)
}
