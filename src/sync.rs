//! Gazelle-style config generation (after rules_clojure's `gen_srcs`): the
//! toolchain OWNS a marker-delimited `fileGroups` block inside each Clojure
//! project's `moon.yml`, regenerated from the deps.edn `:local/root` closure
//! during project sync. Humans reference the group from tasks — once, as
//! `'@group(localDeps)'` — and never hand-maintain dependency source globs.
//!
//! The block is a pure function of the manifest closure and is rewritten
//! only when its content actually changes (gen_srcs' `changed?` idempotence).

use std::collections::BTreeSet;

pub const BLOCK_START: &str = "# <clojure-toolchain:local-deps>";
pub const BLOCK_END: &str = "# </clojure-toolchain:local-deps>";
pub const GROUP_NAME: &str = "localDeps";

/// Render the managed block for a set of workspace-relative dependency
/// source dirs (from `collect_dep_source_dirs`).
pub fn render_local_deps_block(dirs: &BTreeSet<String>) -> String {
    let mut out = String::new();

    out.push_str(BLOCK_START);
    out.push('\n');
    out.push_str(
        "# Generated from deps.edn's transitive :local/root closure — the source\n\
         # dirs of every local dependency. Do not edit: project sync (any moon run,\n\
         # or `moon sync projects`) refreshes it. Reference it from tasks as an\n\
         # input: '@group(localDeps)'.\n",
    );

    if dirs.is_empty() {
        out.push_str("fileGroups:\n  localDeps: []\n");
    } else {
        out.push_str("fileGroups:\n  localDeps:\n");

        for dir in dirs {
            out.push_str("    - '/");
            out.push_str(dir);
            out.push_str("/**/*'\n");
        }
    }

    out.push_str(BLOCK_END);
    out.push('\n');
    out
}

/// Splice the managed block into `moon.yml` content: replaces an existing
/// marker-delimited block in place, or appends one (blank-line separated)
/// when absent. Returns `None` when the content is already up to date.
pub fn splice_managed_block(content: &str, block: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|l| l.trim() == BLOCK_START);
    let end = lines.iter().position(|l| l.trim() == BLOCK_END);

    let updated = match (start, end) {
        (Some(start), Some(end)) if start <= end => {
            let mut out = String::new();

            for line in &lines[..start] {
                out.push_str(line);
                out.push('\n');
            }

            out.push_str(block);

            for line in &lines[end + 1..] {
                out.push_str(line);
                out.push('\n');
            }

            out
        }
        _ => {
            let mut out = content.to_string();

            if !out.is_empty() {
                while !out.ends_with("\n\n") {
                    out.push('\n');
                }
            }

            out.push_str(block);
            out
        }
    };

    if updated == content {
        None
    } else {
        Some(updated)
    }
}

/// True when the content declares a `fileGroups:` key OUTSIDE the managed
/// block — splicing ours in would produce a duplicate YAML key, so the
/// caller must skip (and the user merges the group by hand).
pub fn has_unmanaged_file_groups(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|l| l.trim() == BLOCK_START);
    let end = lines.iter().position(|l| l.trim() == BLOCK_END);

    lines.iter().enumerate().any(|(idx, line)| {
        let inside = matches!((start, end), (Some(s), Some(e)) if idx >= s && idx <= e);

        !inside && line.trim_start().starts_with("fileGroups:")
    })
}
