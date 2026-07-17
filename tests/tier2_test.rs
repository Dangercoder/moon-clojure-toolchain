use moon_config::DependencyScope;
use moon_pdk_api::*;
use moon_pdk_test_utils::create_moon_sandbox;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn project_sources() -> BTreeMap<Id, String> {
    BTreeMap::from([
        (Id::raw("cli"), "apps/cli".to_string()),
        (Id::raw("greeter"), "libs/greeter".to_string()),
        (Id::raw("testkit"), "libs/testkit".to_string()),
    ])
}

mod clojure_toolchain_tier2 {
    use super::*;

    mod extend_project_graph {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn links_local_root_deps_as_project_deps() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .extend_project_graph(ExtendProjectGraphInput {
                    project_sources: project_sources(),
                    toolchain_config: json!({}),
                    ..Default::default()
                })
                .await;

            let cli = &output.extended_projects[&Id::raw("cli")];

            assert_eq!(cli.dependencies.len(), 2);

            let greeter = cli
                .dependencies
                .iter()
                .find(|d| d.id == Id::raw("greeter"))
                .unwrap();
            assert_eq!(greeter.scope, DependencyScope::Production);
            assert_eq!(greeter.via.as_deref(), Some("deps.edn example/greeter"));

            let testkit = cli
                .dependencies
                .iter()
                .find(|d| d.id == Id::raw("testkit"))
                .unwrap();
            assert_eq!(testkit.scope, DependencyScope::Development);

            // Only projects with local deps appear.
            assert!(!output.extended_projects.contains_key(&Id::raw("greeter")));

            // All parsed manifests are project-graph cache inputs.
            assert!(
                output
                    .input_files
                    .contains(&PathBuf::from("/workspace/apps/cli/deps.edn"))
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn ignores_alias_deps_when_disabled() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .extend_project_graph(ExtendProjectGraphInput {
                    project_sources: project_sources(),
                    toolchain_config: json!({ "includeAliasDeps": false }),
                    ..Default::default()
                })
                .await;

            let cli = &output.extended_projects[&Id::raw("cli")];

            assert_eq!(cli.dependencies.len(), 1);
            assert_eq!(cli.dependencies[0].id, Id::raw("greeter"));
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn does_nothing_when_disabled() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .extend_project_graph(ExtendProjectGraphInput {
                    project_sources: project_sources(),
                    toolchain_config: json!({ "inferRelationships": false }),
                    ..Default::default()
                })
                .await;

            assert!(output.extended_projects.is_empty());
        }
    }

    mod locate_dependencies_root {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn finds_nearest_deps_edn() {
            let sandbox = create_moon_sandbox("locate");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .locate_dependencies_root(LocateDependenciesRootInput {
                    starting_dir: VirtualPath::Real(sandbox.path().join("app/src/nested")),
                    toolchain_config: json!({}),
                    ..Default::default()
                })
                .await;

            assert_eq!(output.root.unwrap(), PathBuf::from("/workspace/app"));
            assert!(output.members.is_none());
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn finds_bb_edn_roots() {
            let sandbox = create_moon_sandbox("locate");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .locate_dependencies_root(LocateDependenciesRootInput {
                    starting_dir: VirtualPath::Real(sandbox.path().join("scripts")),
                    toolchain_config: json!({}),
                    ..Default::default()
                })
                .await;

            assert_eq!(output.root.unwrap(), PathBuf::from("/workspace/scripts"));
        }
    }

    mod install_dependencies {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn preps_deps_edn_roots() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .install_dependencies(InstallDependenciesInput {
                    root: VirtualPath::Real(sandbox.path().join("apps/cli")),
                    toolchain_config: json!({}),
                    ..Default::default()
                })
                .await;

            let command = output.install_command.unwrap().command;
            assert_eq!(command.command, "clojure");
            assert_eq!(command.args, vec!["-P"]);
            assert!(output.dedupe_command.is_none());
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn passes_prepare_aliases() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .install_dependencies(InstallDependenciesInput {
                    root: VirtualPath::Real(sandbox.path().join("apps/cli")),
                    toolchain_config: json!({ "prepareAliases": ["test", "build"] }),
                    ..Default::default()
                })
                .await;

            let command = output.install_command.unwrap().command;
            assert_eq!(command.args, vec!["-P", "-A:test:build"]);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn preps_bb_edn_roots() {
            let sandbox = create_moon_sandbox("locate");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .install_dependencies(InstallDependenciesInput {
                    root: VirtualPath::Real(sandbox.path().join("scripts")),
                    toolchain_config: json!({}),
                    ..Default::default()
                })
                .await;

            let command = output.install_command.unwrap().command;
            assert_eq!(command.command, "bb");
            assert_eq!(command.args, vec!["prepare"]);
        }
    }

    mod parse_manifest {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn parses_deps_and_alias_deps() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .parse_manifest(ParseManifestInput {
                    path: VirtualPath::Real(sandbox.path().join("apps/cli/deps.edn")),
                    ..Default::default()
                })
                .await;

            assert_eq!(
                output.dependencies["org.clojure/clojure"],
                ManifestDependency::Version(UnresolvedVersionSpec::parse("1.12.0").unwrap())
            );

            let ManifestDependency::Config(greeter) = &output.dependencies["example/greeter"]
            else {
                panic!("expected a config dependency");
            };
            assert_eq!(greeter.path.as_deref(), Some(std::path::Path::new("../../libs/greeter")));

            assert!(output.dev_dependencies.contains_key("example/testkit"));
        }
    }

    mod hash_task_contents {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn hashes_transitive_manifest_closure() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .hash_task_contents(HashTaskContentsInput {
                    project: ProjectFragment {
                        id: Id::raw("cli"),
                        source: "apps/cli".into(),
                        ..Default::default()
                    },
                    toolchain_config: json!({}),
                    ..Default::default()
                })
                .await;

            let content = &output.contents[0];
            let manifests = content["manifests"].as_object().unwrap();

            assert!(manifests.contains_key("/workspace/apps/cli/deps.edn"));
            assert!(manifests.contains_key("/workspace/libs/greeter/deps.edn"));
            assert!(manifests.contains_key("/workspace/libs/testkit/deps.edn"));

            // Manifest content changes flow into the hash.
            let hash = manifests["/workspace/libs/greeter/deps.edn"].as_str().unwrap();
            assert_eq!(hash.len(), 64);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn hashes_dependency_sources_by_default() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .hash_task_contents(HashTaskContentsInput {
                    project: ProjectFragment {
                        id: Id::raw("cli"),
                        source: "apps/cli".into(),
                        ..Default::default()
                    },
                    toolchain_config: json!({}),
                    ..Default::default()
                })
                .await;

            let sources = output.contents[0]["localSources"].as_object().unwrap();

            assert!(sources.contains_key("/workspace/libs/greeter/src/greeter/core.clj"));
            assert!(sources.contains_key("/workspace/libs/greeter/resources/greeting.txt"));
            // No :paths declared -> tools.deps default ["src"].
            assert!(sources.contains_key("/workspace/libs/testkit/src/testkit/helpers.clj"));
            // The origin project's own sources belong to its task inputs.
            assert!(!sources.keys().any(|k| k.contains("apps/cli")));
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn omits_sources_when_disabled() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .hash_task_contents(HashTaskContentsInput {
                    project: ProjectFragment {
                        id: Id::raw("cli"),
                        source: "apps/cli".into(),
                        ..Default::default()
                    },
                    toolchain_config: json!({ "hashLocalSources": false }),
                    ..Default::default()
                })
                .await;

            assert!(output.contents[0].get("localSources").is_none());
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn marks_external_deps_unavailable() {
            let sandbox = create_moon_sandbox("projects");
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .hash_task_contents(HashTaskContentsInput {
                    project: ProjectFragment {
                        id: Id::raw("cli"),
                        source: "apps/cli".into(),
                        ..Default::default()
                    },
                    toolchain_config: json!({}),
                    ..Default::default()
                })
                .await;

            let manifests = output.contents[0]["manifests"].as_object().unwrap();

            let external = manifests
                .iter()
                .find(|(k, _)| k.contains("outside-workspace"))
                .expect("external dep should be tracked");
            assert_eq!(external.1.as_str().unwrap(), "unavailable");
        }
    }
}
