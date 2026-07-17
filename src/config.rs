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

        /// Fold every source file of the transitive `:local/root` dependency
        /// closure into task hashes (each dependency's top-level `:paths`,
        /// tools.deps default `["src"]`) — not just the manifests. Closes the
        /// stale-cache gap left when a consumer task's `inputs` miss a
        /// dependency's sources: moon has no execution sandbox, so an
        /// undeclared input fails STALE (silent cached green), never loud.
        #[setting(default = true)]
        pub hash_local_sources: bool,

        /// Aliases to activate when preparing dependencies, e.g. `["test", "build"]`
        /// results in `clojure -P -A:test:build`.
        pub prepare_aliases: Vec<String>,

        /// During project sync, generate and maintain a marker-delimited
        /// `fileGroups.localDeps` block in each Clojure project's `moon.yml`,
        /// listing the source-dir globs of its transitive `:local/root`
        /// closure. Tasks reference it once as `'@group(localDeps)'` and the
        /// dependency list is never hand-maintained again (rules_clojure
        /// `gen_srcs` style). The block is a pure function of the manifests
        /// and only rewritten when its content changes.
        #[setting(default = true)]
        pub sync_dependency_inputs: bool,

        /// Reserved for future proto-based version management of the
        /// Clojure CLI. Currently the `clojure`/`bb` binaries are
        /// resolved from `PATH`.
        pub version: Option<UnresolvedVersionSpec>,
    }
);
