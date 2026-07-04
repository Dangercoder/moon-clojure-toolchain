use clojure_toolchain::deps_edn::DepsEdn;

#[test]
fn parses_empty_content() {
    assert_eq!(DepsEdn::parse("").unwrap(), DepsEdn::default());
    assert_eq!(DepsEdn::parse("  \n").unwrap(), DepsEdn::default());
    assert_eq!(DepsEdn::parse("{}").unwrap(), DepsEdn::default());
}

#[test]
fn parses_paths_and_deps() {
    let doc = DepsEdn::parse(
        r#"
        ;; a comment
        {:paths ["src" "resources"]
         :deps {org.clojure/clojure {:mvn/version "1.12.0"}
                example/lib {:local/root "../lib"}
                io.github.foo/bar {:git/url "https://github.com/foo/bar"
                                   :git/sha "abc123"}}}
        "#,
    )
    .unwrap();

    assert_eq!(doc.paths, vec!["src", "resources"]);
    assert_eq!(
        doc.deps["org.clojure/clojure"].mvn_version.as_deref(),
        Some("1.12.0")
    );
    assert_eq!(doc.deps["example/lib"].local_root.as_deref(), Some("../lib"));
    assert_eq!(
        doc.deps["io.github.foo/bar"].git_url.as_deref(),
        Some("https://github.com/foo/bar")
    );
    assert_eq!(doc.deps["io.github.foo/bar"].git_sha.as_deref(), Some("abc123"));
}

#[test]
fn parses_alias_deps_and_paths() {
    let doc = DepsEdn::parse(
        r#"
        {:deps {}
         :aliases
         {:test {:extra-paths ["test"]
                 :extra-deps {example/testkit {:local/root "../testkit"}}}
          :build {:deps {io.github.clojure/tools.build {:mvn/version "0.10.5"}}
                  :ns-default build}}}
        "#,
    )
    .unwrap();

    assert_eq!(
        doc.alias_deps["example/testkit"].local_root.as_deref(),
        Some("../testkit")
    );
    assert_eq!(
        doc.alias_deps["io.github.clojure/tools.build"]
            .mvn_version
            .as_deref(),
        Some("0.10.5")
    );
    assert_eq!(doc.alias_paths, vec!["test"]);
}

#[test]
fn resolves_keyword_path_aliases() {
    let doc = DepsEdn::parse(
        r#"
        {:paths [:clj-paths "resources"]
         :aliases {:clj-paths ["src/clj" "src/cljc"]}}
        "#,
    )
    .unwrap();

    assert_eq!(doc.paths, vec!["src/clj", "src/cljc", "resources"]);
}

#[test]
fn tolerates_tagged_literals_and_discards() {
    let doc = DepsEdn::parse(
        r#"
        {:paths ["src"]
         #_:ignored-key
         :deps {example/lib {:local/root #_"old" "../lib"}}
         :other #shadow/env "PORT"}
        "#,
    )
    .unwrap();

    assert_eq!(doc.paths, vec!["src"]);
    assert_eq!(doc.deps["example/lib"].local_root.as_deref(), Some("../lib"));
}

#[test]
fn local_deps_dedups_alias_overrides() {
    let doc = DepsEdn::parse(
        r#"
        {:deps {example/a {:local/root "../a"}}
         :aliases {:dev {:extra-deps {example/a {:local/root "../a-dev"}
                                      example/b {:local/root "../b"}}}}}
        "#,
    )
    .unwrap();

    let with_aliases = doc.local_deps(true);
    assert_eq!(
        with_aliases,
        vec![("example/a", "../a", false), ("example/b", "../b", true)]
    );

    let without_aliases = doc.local_deps(false);
    assert_eq!(without_aliases, vec![("example/a", "../a", false)]);
}

#[test]
fn parses_git_tag_coordinates() {
    let doc = DepsEdn::parse(
        r#"
        {:deps {io.github.cognitect-labs/test-runner
                {:git/tag "v0.5.1" :git/sha "dfb30dd"}}}
        "#,
    )
    .unwrap();

    let coord = &doc.deps["io.github.cognitect-labs/test-runner"];
    assert_eq!(coord.git_tag.as_deref(), Some("v0.5.1"));
    assert_eq!(coord.git_sha.as_deref(), Some("dfb30dd"));
}
