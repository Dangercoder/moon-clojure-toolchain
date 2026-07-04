use clojure_toolchain::closure::{collect_manifest_closure, normalize_rel_source};
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
        let manifests = collect_manifest_closure(&root, true).unwrap();

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
        let manifests = collect_manifest_closure(&root, false).unwrap();

        assert!(!manifests.keys().any(|k| k.contains("testkit")));
        assert!(manifests.keys().any(|k| k.ends_with("libs/greeter/deps.edn")));
    }
}
