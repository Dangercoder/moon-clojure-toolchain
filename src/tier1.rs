use crate::config::ClojureToolchainConfig;
use extism_pdk::*;
use moon_config::LanguageType;
use moon_pdk_api::*;
use schematic::SchemaBuilder;

#[plugin_fn]
pub fn register_toolchain(
    Json(_): Json<RegisterToolchainInput>,
) -> FnResult<Json<RegisterToolchainOutput>> {
    Ok(Json(RegisterToolchainOutput {
        name: "Clojure".into(),
        plugin_version: env!("CARGO_PKG_VERSION").into(),
        description: Some(
            "Clojure (tools.deps) support: deps.edn-aware project relationships, \
             dependency hashing, and dependency preparation"
                .into(),
        ),
        language: Some(LanguageType::Other(Id::raw("clojure"))),
        config_file_globs: vec![
            "shadow-cljs.edn".into(),
            "build.clj".into(),
            "tests.edn".into(),
        ],
        exe_names: vec!["clojure".into(), "clj".into(), "bb".into()],
        lock_file_names: vec![],
        manifest_file_names: vec!["deps.edn".into(), "bb.edn".into()],
        vendor_dir_name: Some(".cpcache".into()),
        ..Default::default()
    }))
}

#[plugin_fn]
pub fn define_toolchain_config() -> FnResult<Json<DefineToolchainConfigOutput>> {
    Ok(Json(DefineToolchainConfigOutput {
        schema: SchemaBuilder::build_root::<ClojureToolchainConfig>(),
    }))
}

#[plugin_fn]
pub fn initialize_toolchain(
    Json(_): Json<InitializeToolchainInput>,
) -> FnResult<Json<InitializeToolchainOutput>> {
    Ok(Json(InitializeToolchainOutput {
        config_url: Some("https://github.com/Dangercoder/moonrepo-clojure#configuration".into()),
        docs_url: Some("https://github.com/Dangercoder/moonrepo-clojure".into()),
        prompts: vec![SettingPrompt::new(
            "includeAliasDeps",
            "Scan alias maps (<file>:extra-deps</file>, etc) for local dependencies?",
            PromptType::Confirm { default: true },
        )],
        ..Default::default()
    }))
}

#[plugin_fn]
pub fn define_docker_metadata(
    Json(_): Json<DefineDockerMetadataInput>,
) -> FnResult<Json<DefineDockerMetadataOutput>> {
    Ok(Json(DefineDockerMetadataOutput {
        default_image: Some("clojure:temurin-21-tools-deps".into()),
        scaffold_globs: vec!["build.clj".into(), "shadow-cljs.edn".into()],
        ..Default::default()
    }))
}
