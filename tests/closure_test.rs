use clojure_toolchain::closure::{collect_closure, normalize_rel_source};
use moon_pdk_api::VirtualPath;
use std::path::PathBuf;

mod normalize {
    use super::*;

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(
            normalize_rel_source("apps/cli", "../../libs/greeter").as_deref(),
            Some("libs/greeter")
        );
        assert_eq!(
            normalize_rel_source("apps/cli", "./nested").as_deref(),
            Some("apps/cli/nested")
        );
        assert_eq!(normalize_rel_source("apps/cli", "../..").as_deref(), Some("."));
        assert_eq!(normalize_rel_source(".", "libs/a").as_deref(), Some("libs/a"));
    }

    #[test]
    fn rejects_escapes_and_absolutes() {
        assert_eq!(normalize_rel_source("apps/cli", "../../../other"), None);
        assert_eq!(normalize_rel_source(".", ".."), None);
        assert_eq!(normalize_rel_source("apps/cli", "/abs/path"), None);
        assert_eq!(normalize_rel_source("apps/cli", "C:/other"), None);
    }
}

mod manifest_closure {
    use super::*;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/__fixtures__/projects")
    }

    #[test]
    fn walks_transitive_local_deps() {
        let root = VirtualPath::Real(fixture_root().join("apps/cli"));
        let manifests = collect_closure(&root, true, false).unwrap().manifests;

        let keys: Vec<_> = manifests.keys().collect();

        assert!(keys.iter().any(|k| k.ends_with("apps/cli/deps.edn")));
        assert!(keys.iter().any(|k| k.ends_with("libs/greeter/deps.edn")));
        assert!(keys.iter().any(|k| k.ends_with("libs/testkit/deps.edn")));

        // The external dep does not exist: recorded as unavailable.
        let external = manifests
            .iter()
            .find(|(k, _)| k.ends_with("outside-workspace"))
            .expect("external dep should be marked");
        assert_eq!(external.1, "unavailable");

        // Real manifests get sha256 content hashes.
        let cli = manifests
            .iter()
            .find(|(k, _)| k.ends_with("apps/cli/deps.edn"))
            .unwrap();
        assert_eq!(cli.1.len(), 64);
    }

    #[test]
    fn skips_alias_deps_when_disabled() {
        let root = VirtualPath::Real(fixture_root().join("apps/cli"));
        let manifests = collect_closure(&root, false, false).unwrap().manifests;

        assert!(!manifests.keys().any(|k| k.contains("testkit")));
        assert!(manifests.keys().any(|k| k.ends_with("libs/greeter/deps.edn")));
    }
}

mod source_closure {
    use super::*;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/__fixtures__/projects")
    }

    fn cli_root() -> VirtualPath {
        VirtualPath::Real(fixture_root().join("apps/cli"))
    }

    #[test]
    fn hashes_dep_sources_but_never_the_origin() {
        let closure = collect_closure(&cli_root(), true, true).unwrap();
        let keys: Vec<_> = closure.sources.keys().collect();

        // Every declared :paths dir of a dependency counts, resources included.
        assert!(
            keys.iter()
                .any(|k| k.ends_with("libs/greeter/src/greeter/core.clj"))
        );
        assert!(
            keys.iter()
                .any(|k| k.ends_with("libs/greeter/resources/greeting.txt"))
        );

        // The origin project's own sources belong to its task inputs.
        assert!(!keys.iter().any(|k| k.contains("apps/cli/src")));
    }

    #[test]
    fn defaults_paths_to_src_when_undeclared() {
        // testkit's deps.edn declares no :paths — tools.deps defaults to ["src"].
        let closure = collect_closure(&cli_root(), true, true).unwrap();

        assert!(
            closure
                .sources
                .keys()
                .any(|k| k.ends_with("libs/testkit/src/testkit/helpers.clj"))
        );
    }

    #[test]
    fn skips_dotfiles_and_build_output_dirs() {
        let closure = collect_closure(&cli_root(), true, true).unwrap();

        assert!(!closure.sources.keys().any(|k| k.contains(".nrepl-port")));
        assert!(!closure.sources.keys().any(|k| k.contains("node_modules")));
    }

    #[test]
    fn respects_alias_scoping() {
        // testkit reaches cli only through the :test alias.
        let closure = collect_closure(&cli_root(), false, true).unwrap();

        assert!(!closure.sources.keys().any(|k| k.contains("testkit")));
        assert!(closure.sources.keys().any(|k| k.contains("greeter")));
    }

    #[test]
    fn source_hashing_can_be_disabled() {
        let closure = collect_closure(&cli_root(), true, false).unwrap();

        assert!(closure.sources.is_empty());
    }

    #[test]
    fn produces_content_hashes() {
        let closure = collect_closure(&cli_root(), true, true).unwrap();

        let (_, hash) = closure
            .sources
            .iter()
            .find(|(k, _)| k.ends_with("core.clj"))
            .unwrap();

        assert_eq!(hash.len(), 64);
    }
}
