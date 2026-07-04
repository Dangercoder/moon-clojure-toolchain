use moon_pdk_api::*;
use moon_pdk_test_utils::create_empty_moon_sandbox;

mod clojure_toolchain_tier1 {
    use super::*;

    mod register_toolchain {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn registers_metadata() {
            let sandbox = create_empty_moon_sandbox();
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .register_toolchain(RegisterToolchainInput {
                    id: Id::raw("clojure"),
                })
                .await;

            assert_eq!(output.name, "Clojure");
            assert_eq!(
                output.manifest_file_names,
                vec!["deps.edn".to_string(), "bb.edn".to_string()]
            );
            assert_eq!(
                output.exe_names,
                vec!["clojure".to_string(), "clj".to_string(), "bb".to_string()]
            );
            assert_eq!(output.vendor_dir_name.as_deref(), Some(".cpcache"));
            assert!(output.lock_file_names.is_empty());
            assert!(!output.plugin_version.is_empty());
        }
    }

    mod initialize_toolchain {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn returns_docs_and_prompts() {
            let sandbox = create_empty_moon_sandbox();
            let plugin = sandbox.create_toolchain("clojure").await;

            let output = plugin
                .initialize_toolchain(InitializeToolchainInput::default())
                .await;

            assert!(output.config_url.is_some());
            assert_eq!(output.prompts.len(), 1);
        }
    }
}
