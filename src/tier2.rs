use crate::closure::{collect_closure, collect_dep_source_dirs, normalize_rel_source, MANIFEST_NAMES};
use crate::config::ClojureToolchainConfig;
use crate::deps_edn::{DepCoord, DepsEdn};
use crate::sync::{has_unmanaged_file_groups, render_local_deps_block, splice_managed_block, BLOCK_START};
use extism_pdk::*;
use moon_config::DependencyScope;
use moon_pdk::{locate_root, parse_toolchain_config_schema};
use moon_pdk_api::*;
use std::collections::BTreeMap;
use std::fs;

/// Every project with a `deps.edn` (or `bb.edn`) is its own dependencies
/// root — tools.deps has no workspace-level lockfile.
#[plugin_fn]
pub fn locate_dependencies_root(
    Json(input): Json<LocateDependenciesRootInput>,
) -> FnResult<Json<LocateDependenciesRootOutput>> {
    let mut output = LocateDependenciesRootOutput::default();

    for name in MANIFEST_NAMES {
        if let Some(root) = locate_root(&input.starting_dir, name) {
            output.root = root.virtual_path();
            break;
        }
    }

    Ok(Json(output))
}

/// Warm the dependency caches (`~/.m2`, `~/.gitlibs`, `.cpcache`) ahead of
/// task execution: `clojure -P` for deps.edn roots, `bb prepare` for
/// bb.edn-only roots.
#[plugin_fn]
pub fn install_dependencies(
    Json(input): Json<InstallDependenciesInput>,
) -> FnResult<Json<InstallDependenciesOutput>> {
    let config = parse_toolchain_config_schema::<ClojureToolchainConfig>(input.toolchain_config)?;
    let mut output = InstallDependenciesOutput::default();

    if input.root.join("deps.edn").exists() {
        let mut args = vec!["-P".to_string()];

        if !config.prepare_aliases.is_empty() {
            args.push(format!("-A:{}", config.prepare_aliases.join(":")));
        }

        output.install_command = Some(
            ExecCommandInput::new("clojure", args)
                .cwd(input.root.clone())
                .into(),
        );
    } else if input.root.join("bb.edn").exists() {
        output.install_command = Some(
            ExecCommandInput::new("bb", ["prepare"])
                .cwd(input.root.clone())
                .into(),
        );
    }

    Ok(Json(output))
}

/// Turn `:local/root` coordinates into implicit moon project relationships,
/// so that `^:build`-style task deps, affected detection, and `moon ci`
/// understand the Clojure dependency graph without manual `dependsOn`.
#[plugin_fn]
pub fn extend_project_graph(
    Json(input): Json<ExtendProjectGraphInput>,
) -> FnResult<Json<ExtendProjectGraphOutput>> {
    let config = parse_toolchain_config_schema::<ClojureToolchainConfig>(input.toolchain_config)?;
    let mut output = ExtendProjectGraphOutput::default();

    if !config.infer_relationships {
        return Ok(Json(output));
    }

    // First pass: normalized source dir -> project id.
    let mut by_source: BTreeMap<String, Id> = BTreeMap::new();

    for (id, source) in &input.project_sources {
        let key = normalize_rel_source(source, ".").unwrap_or_else(|| ".".into());
        by_source.insert(key, id.to_owned());
    }

    // Second pass: parse each project's manifests and link local deps.
    for (id, source) in &input.project_sources {
        let project_root = input.context.workspace_root.join(source);
        let mut project_output = ExtendProjectOutput::default();

        for name in MANIFEST_NAMES {
            let manifest_path = project_root.join(name);

            if !manifest_path.exists() {
                continue;
            }

            let content = fs::read_to_string(manifest_path.any_path())?;
            let manifest = DepsEdn::parse(&content)
                .map_err(|e| anyhow!("failed to parse {}: {e}", manifest_path.any_path().display()))?;

            if let Some(file) = manifest_path.virtual_path() {
                output.input_files.push(file);
            }

            for (lib, local_root, from_alias) in manifest.local_deps(config.include_alias_deps) {
                let Some(dep_source) = normalize_rel_source(source, local_root) else {
                    // Outside the workspace — cannot be a project relationship.
                    continue;
                };

                let Some(dep_id) = by_source.get(&dep_source) else {
                    continue;
                };

                if dep_id == id
                    || project_output
                        .dependencies
                        .iter()
                        .any(|dep| &dep.id == dep_id)
                {
                    continue;
                }

                project_output.dependencies.push(ProjectDependency {
                    id: dep_id.to_owned(),
                    scope: if from_alias {
                        DependencyScope::Development
                    } else {
                        DependencyScope::Production
                    },
                    via: Some(format!("{name} {lib}")),
                });
            }
        }

        if !project_output.dependencies.is_empty() {
            output.extended_projects.insert(id.to_owned(), project_output);
        }
    }

    Ok(Json(output))
}

fn to_manifest_dependency(coord: &DepCoord) -> ManifestDependency {
    if let Some(path) = &coord.local_root {
        return ManifestDependency::Config(ManifestDependencyConfig {
            path: Some(path.into()),
            ..Default::default()
        });
    }

    if let Some(url) = &coord.git_url {
        return ManifestDependency::Config(ManifestDependencyConfig {
            url: Some(url.to_owned()),
            reference: coord.git_sha.clone().or_else(|| coord.git_tag.clone()),
            ..Default::default()
        });
    }

    if let Some(version) = &coord.mvn_version {
        // Maven versions are not always semver ("1.2.3.4", "2.0-alpha1"),
        // so fall back to a reference string when parsing fails.
        return match UnresolvedVersionSpec::parse(version) {
            Ok(spec) => ManifestDependency::Version(spec),
            Err(_) => ManifestDependency::Config(ManifestDependencyConfig {
                reference: Some(version.to_owned()),
                ..Default::default()
            }),
        };
    }

    // Git deps with inferred URLs (io.github.org/project) end up here.
    ManifestDependency::Config(ManifestDependencyConfig {
        reference: coord.git_sha.clone().or_else(|| coord.git_tag.clone()),
        ..Default::default()
    })
}

#[plugin_fn]
pub fn parse_manifest(
    Json(input): Json<ParseManifestInput>,
) -> FnResult<Json<ParseManifestOutput>> {
    let mut output = ParseManifestOutput::default();

    let content = fs::read_to_string(input.path.any_path())?;
    let manifest = DepsEdn::parse(&content)
        .map_err(|e| anyhow!("failed to parse {}: {e}", input.path.any_path().display()))?;

    for (lib, coord) in &manifest.deps {
        output
            .dependencies
            .insert(lib.to_owned(), to_manifest_dependency(coord));
    }

    for (lib, coord) in &manifest.alias_deps {
        if !manifest.deps.contains_key(lib) {
            output
                .dev_dependencies
                .insert(lib.to_owned(), to_manifest_dependency(coord));
        }
    }

    Ok(Json(output))
}

/// Generate and maintain the `fileGroups.localDeps` block in the project's
/// `moon.yml` from its transitive `:local/root` closure — the dependency
/// input list moon's affected detection schedules from, produced instead of
/// hand-maintained (rules_clojure `gen_srcs` style; see `sync.rs`).
#[plugin_fn]
pub fn sync_project(Json(input): Json<SyncProjectInput>) -> FnResult<Json<SyncOutput>> {
    let config = parse_toolchain_config_schema::<ClojureToolchainConfig>(input.toolchain_config)?;
    let mut output = SyncOutput::default();

    if !config.sync_dependency_inputs {
        output.skipped = true;
        return Ok(Json(output));
    }

    let dirs = collect_dep_source_dirs(
        &input.context.workspace_root,
        &input.project.source,
        config.include_alias_deps,
    )?;

    let manifest_path = input
        .context
        .workspace_root
        .join(&input.project.source)
        .join("moon.yml");

    let existing = if manifest_path.exists() {
        fs::read_to_string(manifest_path.any_path())?
    } else {
        String::new()
    };

    let has_markers = existing.contains(BLOCK_START);

    // Nothing to declare and nothing previously declared — leave the
    // project untouched (no surprise moon.yml churn for leaf projects).
    if dirs.is_empty() && !has_markers {
        output.skipped = true;
        return Ok(Json(output));
    }

    // A hand-written fileGroups key outside the managed block would collide
    // (duplicate YAML key). Leave the file alone; the user opts in by adding
    // the marker block inside their own fileGroups arrangement.
    if !has_markers && has_unmanaged_file_groups(&existing) {
        output.skipped = true;
        return Ok(Json(output));
    }

    let block = render_local_deps_block(&dirs);

    if let Some(updated) = splice_managed_block(&existing, &block) {
        fs::write(manifest_path.any_path(), updated)?;
        output
            .changed_files
            .push(manifest_path.any_path().to_path_buf());
    } else {
        output.skipped = true;
    }

    Ok(Json(output))
}

/// Fold the transitive `:local/root` closure into every task hash: the
/// manifests AND (by default) every dependency source file under each dep's
/// top-level `:paths`. Dependency changes anywhere in the closure invalidate
/// task caches even when the consumer's task `inputs` don't list the
/// dependency's sources — per-file entries also make `moon query hash-diff`
/// name the exact file behind a cache miss.
#[plugin_fn]
pub fn hash_task_contents(
    Json(input): Json<HashTaskContentsInput>,
) -> FnResult<Json<HashTaskContentsOutput>> {
    let config = parse_toolchain_config_schema::<ClojureToolchainConfig>(input.toolchain_config)?;
    let project_root = input.context.get_project_root(&input.project);

    let closure = collect_closure(
        &project_root,
        config.include_alias_deps,
        config.hash_local_sources,
    )?;

    let mut content = json::Map::new();
    content.insert("toolchain".into(), json::Value::String("clojure".into()));
    content.insert(
        "manifests".into(),
        json::Value::Object(
            closure
                .manifests
                .into_iter()
                .map(|(path, hash)| (path, json::Value::String(hash)))
                .collect(),
        ),
    );

    // Omitted when empty so leaf projects (no local deps) keep their
    // pre-0.2.0 hashes — upgrading doesn't cold-start their caches.
    if !closure.sources.is_empty() {
        content.insert(
            "localSources".into(),
            json::Value::Object(
                closure
                    .sources
                    .into_iter()
                    .map(|(path, hash)| (path, json::Value::String(hash)))
                    .collect(),
            ),
        );
    }

    if !config.prepare_aliases.is_empty() {
        content.insert(
            "prepareAliases".into(),
            json::Value::Array(
                config
                    .prepare_aliases
                    .iter()
                    .map(|a| json::Value::String(a.to_owned()))
                    .collect(),
            ),
        );
    }

    Ok(Json(HashTaskContentsOutput {
        contents: vec![json::Value::Object(content)],
    }))
}
