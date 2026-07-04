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

fn sha256_hex(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut out = String::with_capacity(64);

    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }

    out
}

/// Walk the transitive `:local/root` closure starting from a project root,
/// producing a deterministic map of `manifest path -> sha256(content)`.
///
/// Dependency roots that cannot be read (missing, or outside the WASM
/// sandbox) are recorded with an `unavailable` marker so that at least
/// path changes still invalidate the hash.
pub fn collect_manifest_closure(
    project_root: &VirtualPath,
    include_alias_deps: bool,
) -> AnyResult<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut visited: BTreeSet<PathBuf> = BTreeSet::new();
    let mut queue: Vec<VirtualPath> = vec![project_root.clone()];

    while let Some(root) = queue.pop() {
        let root = normalize_virtual(&root);

        if !visited.insert(root.any_path().to_path_buf()) {
            continue;
        }

        let mut found_manifest = false;

        for name in MANIFEST_NAMES {
            let file = root.join(name);

            if !file.exists() {
                continue;
            }

            found_manifest = true;

            let key = file.to_string();

            let Ok(content) = fs::read_to_string(file.any_path()) else {
                out.insert(key, "unreadable".to_string());
                continue;
            };

            out.insert(key, sha256_hex(&content));

            let manifest = DepsEdn::parse(&content)
                .map_err(|e| anyhow!("failed to parse {}: {e}", file.any_path().display()))?;

            for (_, local_root, _) in manifest.local_deps(include_alias_deps) {
                queue.push(root.join(local_root));
            }
        }

        if !found_manifest {
            // Missing or outside the sandbox — record its presence so the
            // hash still changes if the path itself changes.
            out.insert(root.to_string(), "unavailable".to_string());
        }
    }

    Ok(out)
}
