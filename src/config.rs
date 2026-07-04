use moon_pdk_api::{UnresolvedVersionSpec, config_struct};
use schematic::Config;

config_struct!(
    /// Configures and enables the Clojure toolchain.
    #[derive(Config)]
    pub struct ClojureToolchainConfig {
        /// Infer project relationships from `:local/root` coordinates
        /// found in `deps.edn` and `bb.edn` manifests.
        #[setting(default = true)]
        pub infer_relationships: bool,

        /// Also scan alias maps (`:extra-deps`, `:replace-deps`, `:deps`,
        /// `:override-deps`, `:default-deps`) for `:local/root` coordinates.
        #[setting(default = true)]
        pub include_alias_deps: bool,

        /// Aliases to activate when preparing dependencies, e.g. `["test", "build"]`
        /// results in `clojure -P -A:test:build`.
        pub prepare_aliases: Vec<String>,

        /// Reserved for future proto-based version management of the
        /// Clojure CLI. Currently the `clojure`/`bb` binaries are
        /// resolved from `PATH`.
        pub version: Option<UnresolvedVersionSpec>,
    }
);
