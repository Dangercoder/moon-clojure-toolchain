use clojure_reader::edn::{self, Edn};
use moon_pdk_api::{AnyResult, anyhow};
use std::collections::BTreeMap;

/// A single dependency coordinate from a `:deps` map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DepCoord {
    pub mvn_version: Option<String>,
    pub local_root: Option<String>,
    pub git_url: Option<String>,
    pub git_sha: Option<String>,
    pub git_tag: Option<String>,
}

impl DepCoord {
    pub fn is_local(&self) -> bool {
        self.local_root.is_some()
    }
}

/// The subset of a `deps.edn` / `bb.edn` document that the toolchain
/// cares about. Alias-scoped values are unioned across all aliases.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DepsEdn {
    /// Top-level `:paths` (with alias keyword indirections resolved).
    pub paths: Vec<String>,
    /// Top-level `:deps`, keyed by lib symbol (e.g. `org.clojure/clojure`).
    pub deps: BTreeMap<String, DepCoord>,
    /// Union of `:extra-deps`, `:replace-deps`, `:deps`, `:override-deps`,
    /// and `:default-deps` across all aliases.
    pub alias_deps: BTreeMap<String, DepCoord>,
    /// Union of `:extra-paths`, `:replace-paths`, and `:paths` across all aliases.
    pub alias_paths: Vec<String>,
}

impl DepsEdn {
    pub fn parse(content: &str) -> AnyResult<DepsEdn> {
        if content.trim().is_empty() {
            return Ok(DepsEdn::default());
        }

        let root = edn::read_string(content).map_err(|e| anyhow!("invalid EDN: {e:?}"))?;
        let root = untag(&root);

        let mut doc = DepsEdn::default();

        let Edn::Map(root_map) = root else {
            // Not a map (e.g. empty file) — treat as empty config.
            return Ok(doc);
        };

        if let Some(deps) = get_key(root_map, "deps") {
            parse_dep_map(deps, &mut doc.deps);
        }

        let aliases = get_key(root_map, "aliases").and_then(|a| match untag(a) {
            Edn::Map(m) => Some(m),
            _ => None,
        });

        if let Some(paths) = get_key(root_map, "paths") {
            parse_paths(paths, aliases, &mut doc.paths);
        }

        if let Some(aliases) = aliases {
            for alias_value in aliases.values() {
                let Edn::Map(alias_map) = untag(alias_value) else {
                    continue;
                };

                for deps_key in [
                    "extra-deps",
                    "replace-deps",
                    "deps",
                    "override-deps",
                    "default-deps",
                ] {
                    if let Some(deps) = get_key(alias_map, deps_key) {
                        parse_dep_map(deps, &mut doc.alias_deps);
                    }
                }

                for paths_key in ["extra-paths", "replace-paths", "paths"] {
                    if let Some(paths) = get_key(alias_map, paths_key) {
                        parse_paths(paths, None, &mut doc.alias_paths);
                    }
                }
            }
        }

        Ok(doc)
    }

    /// All `:local/root` dependencies as `(lib, path, from_alias)` tuples.
    /// Top-level `:deps` come first, then alias-scoped deps not already seen.
    pub fn local_deps(&self, include_aliases: bool) -> Vec<(&str, &str, bool)> {
        let mut out = vec![];

        for (lib, coord) in &self.deps {
            if let Some(root) = &coord.local_root {
                out.push((lib.as_str(), root.as_str(), false));
            }
        }

        if include_aliases {
            for (lib, coord) in &self.alias_deps {
                if let Some(root) = &coord.local_root {
                    if !self.deps.get(lib).is_some_and(DepCoord::is_local) {
                        out.push((lib.as_str(), root.as_str(), true));
                    }
                }
            }
        }

        out
    }
}

/// Unwrap tagged literals (e.g. `#shadow/env "PORT"`) down to their value.
fn untag<'a, 'e>(edn: &'a Edn<'e>) -> &'a Edn<'e> {
    match edn {
        Edn::Tagged(_, inner) => untag(inner),
        other => other,
    }
}

fn get_key<'a, 'e>(map: &'a BTreeMap<Edn<'e>, Edn<'e>>, key: &'e str) -> Option<&'a Edn<'e>> {
    map.get(&Edn::Key(key))
}

fn parse_dep_map(edn: &Edn, out: &mut BTreeMap<String, DepCoord>) {
    let Edn::Map(map) = untag(edn) else {
        return;
    };

    for (lib, coord) in map {
        let name = match untag(lib) {
            Edn::Symbol(s) => (*s).to_string(),
            Edn::Str(s) => (*s).to_string(),
            _ => continue,
        };

        out.insert(name, parse_coord(coord));
    }
}

fn parse_coord(edn: &Edn) -> DepCoord {
    let mut coord = DepCoord::default();

    let Edn::Map(map) = untag(edn) else {
        return coord;
    };

    let get_str = |key: &str| -> Option<String> {
        match get_key(map, key).map(untag) {
            Some(Edn::Str(s)) => Some((*s).to_string()),
            _ => None,
        }
    };

    coord.mvn_version = get_str("mvn/version");
    coord.local_root = get_str("local/root");
    coord.git_url = get_str("git/url");
    coord.git_sha = get_str("git/sha").or_else(|| get_str("sha"));
    coord.git_tag = get_str("git/tag").or_else(|| get_str("tag"));

    coord
}

/// Parse a `:paths`-style vector. String entries are collected directly;
/// keyword entries name aliases whose value is a vector of paths
/// (`{:paths [:clj-paths] :aliases {:clj-paths ["src/clj"]}}`).
fn parse_paths(edn: &Edn, aliases: Option<&BTreeMap<Edn, Edn>>, out: &mut Vec<String>) {
    let Edn::Vector(items) = untag(edn) else {
        return;
    };

    for item in items {
        match untag(item) {
            Edn::Str(s) => {
                let path = (*s).to_string();
                if !out.contains(&path) {
                    out.push(path);
                }
            }
            Edn::Key(alias_name) => {
                if let Some(aliases) = aliases {
                    if let Some(paths) = get_key(aliases, alias_name) {
                        parse_paths(paths, None, out);
                    }
                }
            }
            _ => {}
        }
    }
}
