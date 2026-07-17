use clojure_toolchain::closure::collect_dep_source_dirs;
use clojure_toolchain::sync::{
    has_unmanaged_file_groups, render_local_deps_block, splice_managed_block,
};
use moon_pdk_api::VirtualPath;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn dirs(entries: &[&str]) -> BTreeSet<String> {
    entries.iter().map(|s| s.to_string()).collect()
}

mod dep_source_dirs {
    use super::*;

    fn workspace_root() -> VirtualPath {
        VirtualPath::Real(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/__fixtures__/projects"),
        )
    }

    #[test]
    fn collects_declared_and_defaulted_paths() {
        let dirs = collect_dep_source_dirs(&workspace_root(), "apps/cli", true).unwrap();

        assert_eq!(
            dirs,
            super::dirs(&[
                "libs/greeter/src",
                "libs/greeter/resources",
                // No :paths declared -> tools.deps default ["src"].
                "libs/testkit/src",
            ])
        );
    }

    #[test]
    fn excludes_origin_and_alias_deps_when_disabled() {
        let dirs = collect_dep_source_dirs(&workspace_root(), "apps/cli", false).unwrap();

        assert!(!dirs.iter().any(|d| d.starts_with("apps/cli")));
        assert!(!dirs.iter().any(|d| d.contains("testkit")));
        assert!(dirs.iter().any(|d| d.contains("greeter")));
    }
}

mod render {
    use super::*;

    #[test]
    fn renders_globs_sorted() {
        let block = render_local_deps_block(&dirs(&["libs/b/src", "libs/a/src"]));

        let a = block.find("libs/a/src").unwrap();
        let b = block.find("libs/b/src").unwrap();

        assert!(a < b);
        assert!(block.contains("- '/libs/a/src/**/*'"));
        assert!(block.starts_with("# <clojure-toolchain:local-deps>\n"));
        assert!(block.ends_with("# </clojure-toolchain:local-deps>\n"));
    }

    #[test]
    fn renders_empty_group() {
        let block = render_local_deps_block(&dirs(&[]));

        assert!(block.contains("localDeps: []"));
    }
}

mod splice {
    use super::*;

    #[test]
    fn appends_when_absent() {
        let content = "language: 'clojure'\n";
        let block = render_local_deps_block(&dirs(&["libs/a/src"]));

        let updated = splice_managed_block(content, &block).unwrap();

        assert!(updated.starts_with("language: 'clojure'\n\n# <clojure-toolchain"));
        assert!(updated.ends_with("# </clojure-toolchain:local-deps>\n"));
    }

    #[test]
    fn replaces_in_place_preserving_surroundings() {
        let block_v1 = render_local_deps_block(&dirs(&["libs/a/src"]));
        let content = format!("language: 'clojure'\n\n{block_v1}\ntasks: {{}}\n");

        let block_v2 = render_local_deps_block(&dirs(&["libs/a/src", "libs/b/src"]));
        let updated = splice_managed_block(&content, &block_v2).unwrap();

        assert!(updated.contains("- '/libs/b/src/**/*'"));
        assert!(updated.starts_with("language: 'clojure'\n"));
        assert!(updated.ends_with("tasks: {}\n"));
        // Exactly one managed block remains.
        assert_eq!(updated.matches("# <clojure-toolchain:local-deps>").count(), 1);
    }

    #[test]
    fn idempotent_when_unchanged() {
        let block = render_local_deps_block(&dirs(&["libs/a/src"]));
        let content = format!("language: 'clojure'\n\n{block}");

        assert!(splice_managed_block(&content, &block).is_none());
    }
}

mod unmanaged_guard {
    use super::*;

    #[test]
    fn detects_hand_written_file_groups() {
        assert!(has_unmanaged_file_groups("fileGroups:\n  own: []\n"));
    }

    #[test]
    fn ignores_the_managed_block_itself() {
        let block = render_local_deps_block(&dirs(&["libs/a/src"]));
        let content = format!("language: 'clojure'\n\n{block}");

        assert!(!has_unmanaged_file_groups(&content));
    }
}
