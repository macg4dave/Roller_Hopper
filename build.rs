//! Compiles the Slint UI at build time so UI errors fail the build.

fn main() -> Result<(), slint_build::CompileError> {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/app-window.slint", config)
}
