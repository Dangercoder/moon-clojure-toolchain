use crate::deps_edn::DepsEdn;
use moon_pdk_api::{AnyResult, VirtualPath, anyhow};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST_NAMES: [&str; 2] = ["deps.edn", "bb.edn"];

/// Lexically normalize a path: resolve `.` and `..` components without
/// touching the filesystem. `..` past the root is preserved.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }

    out
}

fn normalize_virtual(path: &VirtualPath) -> VirtualPath {
    match path {
        VirtualPath::Real(p) => VirtualPath::Real(normalize_path(p)),
        VirtualPath::Virtual {
            path,
            virtual_prefix,
            real_prefix,
        } => VirtualPath::Virtual {
            path: normalize_path(path),
            virtual_prefix: virtual_prefix.clone(),
            real_prefix: real_prefix.clone(),
        },
    }
}

/// Resolve a `:local/root` value against a workspace-relative project
/// source dir, returning the dependency's workspace-relative source dir.
/// Returns `None` when the path is absolute or escapes the workspace root.
pub fn normalize_rel_source(base_source: &str, local_root: &str) -> Option<String> {
    if local_root.starts_with('/') || local_root.contains(':') {
        // Absolute unix path or windows drive/URL-ish — treat as external.
        return None;
    }

    let mut stack: Vec<&str> = base_source
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .collect();

    for component in local_root.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if stack.pop().is_none() {
                    // Escapes above the workspace root.
                    return None;
                }
            }
            other => stack.push(other),
        }
    }

    if stack.is_empty() {
        Some(".".to_string())
    } else {
        Some(stack.join("/"))
    }
}

fn sha256_hex_bytes(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    let mut out = String::with_capacity(64);

    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }

    out
}

fn sha256_hex(content: &str) -> String {
    sha256_hex_bytes(content.as_bytes())
}

/// Content-addressable view of a project's transitive `:local/root` closure.
#[derive(Debug, Default)]
pub struct ClosureContents {
    /// `manifest path -> sha256(content)`, with `unavailable`/`unreadable`
    /// markers for roots and files that cannot be read.
    pub manifests: BTreeMap<String, String>,
    /// `dependency source file -> sha256(content)`: every file under each
    /// DEPENDENCY project's top-level `:paths` (tools.deps default `["src"]`
    /// when undeclared). The origin project's own sources are never
    /// collected — the task's `inputs` own those; this map is the backstop
    /// for sources the consumer's `inputs` cannot be trusted to list.
    pub sources: BTreeMap<String, String>,
}

/// Build-output directories that may sit inside a `:paths` dir (bb.edn
/// projects with `:paths ["."]`, generated-source setups). Their contents
/// are machine-produced churn, never classpath source-of-truth.
const SKIPPED_DIR_NAMES: [&str; 2] = ["target", "node_modules"];

/// Walk the transitive `:local/root` closure starting from a project root,
/// producing deterministic `path -> sha256(content)` maps of every manifest
/// and (when `include_dep_sources`) every dependency source file.
///
/// Dependency roots that cannot be read (missing, or outside the WASM
/// sandbox) are recorded with an `unavailable` marker so that at least
/// path changes still invalidate the hash. Same rationale as Bazel action
/// inputs: content digests, never mtimes, and file-level granularity —
/// resources, `data_readers.clj`, and `.cljc` under `:paths` all count.
pub fn collect_closure(
    project_root: &VirtualPath,
    include_alias_deps: bool,
    include_dep_sources: bool,
) -> AnyResult<ClosureContents> {
    let mut out = ClosureContents::default();
    let mut visited: BTreeSet<PathBuf> = BTreeSet::new();
    let origin = normalize_virtual(project_root);
    let mut queue: Vec<VirtualPath> = vec![origin.clone()];

    while let Some(root) = queue.pop() {
        let root = normalize_virtual(&root);

        if !visited.insert(root.any_path().to_path_buf()) {
            continue;
        }

        let is_origin = root.any_path() == origin.any_path();
        let mut found_manifest = false;
        let mut has_deps_edn = false;
        let mut source_paths: Vec<String> = vec![];

        for name in MANIFEST_NAMES {
            let file = root.join(name);

            if !file.exists() {
                continue;
            }

            found_manifest = true;

            let key = file.to_string();

            let Ok(content) = fs::read_to_string(file.any_path()) else {
                out.manifests.insert(key, "unreadable".to_string());
                continue;
            };

            out.manifests.insert(key, sha256_hex(&content));

            let manifest = DepsEdn::parse(&content)
                .map_err(|e| anyhow!("failed to parse {}: {e}", file.any_path().display()))?;

            for (_, local_root, _) in manifest.local_deps(include_alias_deps) {
                queue.push(root.join(local_root));
            }

            if name == "deps.edn" {
                has_deps_edn = true;
            }

            for path in &manifest.paths {
                if !source_paths.contains(path) {
                    source_paths.push(path.clone());
                }
            }
        }

        if !found_manifest {
            // Missing or outside the sandbox — record its presence so the
            // hash still changes if the path itself changes.
            out.manifests.insert(root.to_string(), "unavailable".to_string());
            continue;
        }

        if include_dep_sources && !is_origin {
            // Consumers see a dependency's top-level `:paths` only (aliases
            // never activate transitively in tools.deps). deps.edn defaults
            // `:paths` to ["src"]; bb.edn-only roots hash declared paths only
            // (bb's implicit "." would swallow the whole project dir).
            if source_paths.is_empty() && has_deps_edn {
                source_paths.push("src".to_string());
            }

            for path in source_paths {
                hash_dir_contents(&root.join(&path), &mut out.sources);
            }
        }
    }

    Ok(out)
}

/// Recursively hash every file under `dir` into `out`. Dot-prefixed entries
/// (editor droppings, `.cpcache`, `.nrepl-port`) and build-output dirs are
/// skipped; unreadable files keep a marker so their existence still perturbs
/// the hash. Directory symlinks are skipped (cycle guard); file symlinks are
/// followed. A `:paths` dir that does not exist contributes nothing.
fn hash_dir_contents(dir: &VirtualPath, out: &mut BTreeMap<String, String>) {
    let Ok(entries) = fs::read_dir(dir.any_path()) else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();

        if name.starts_with('.') {
            continue;
        }

        let child = dir.join(&name);

        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            if !SKIPPED_DIR_NAMES.contains(&name.as_str()) {
                hash_dir_contents(&child, out);
            }
        } else if file_type.is_symlink() {
            match fs::metadata(entry.path()) {
                Ok(meta) if meta.is_file() => hash_file(&child, out),
                _ => {}
            }
        } else {
            hash_file(&child, out);
        }
    }
}

fn hash_file(file: &VirtualPath, out: &mut BTreeMap<String, String>) {
    let key = file.to_string();

    match fs::read(file.any_path()) {
        Ok(bytes) => out.insert(key, sha256_hex_bytes(&bytes)),
        Err(_) => out.insert(key, "unreadable".to_string()),
    };
}
