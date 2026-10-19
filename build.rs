//! Compiles the Slint UI at build time so UI errors fail the build, and bundles the UI text in
//! `translations/` (English included) into the binary.

use slint_build::DefaultTranslationContext;

fn main() -> Result<(), slint_build::CompileError> {
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".into())
        .with_bundled_translations("translations")
        // The .slint files use unique message keys, so no per-component context is needed.
        .with_default_translation_context(DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app-window.slint", config)
}
