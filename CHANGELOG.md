# Changelog

## 0.2.0

- **Task hashes now cover the sources of the transitive `:local/root` closure,
  not just its manifests.** For every dependency project in the closure,
  each file under its top-level `:paths` (tools.deps default `["src"]`) is
  content-hashed into `hash_task_contents` as a `localSources` map. A
  dependency source edit now invalidates consumer task caches even when the
  consumer's task `inputs` don't list the dependency's sources — closing the
  stale-cache false-green gap that hand-maintained cross-project input globs
  drift into. Mirrors Bazel's file-level action-input semantics (studied via
  griffinbank/rules_clojure) at project granularity.
- Dependency `:paths` follow tools.deps consumer semantics: top-level only
  (aliases never activate transitively), `["src"]` when undeclared;
  bb.edn-only roots contribute declared paths only. Dot-prefixed entries,
  `target/`, and `node_modules/` are skipped; unreadable files and
  unavailable roots keep marker entries; directory symlinks are skipped
  (cycle guard). Entries are per-file, so `moon query hash-diff` names the
  exact file behind a cache miss.
- New toolchain setting `hashLocalSources` (default `true`) to opt out.
- Leaf projects (no `:local/root` deps) hash identically to 0.1.0, so
  upgrading does not cold-start their caches.

## 0.1.0

Initial release.

- Tier 1: toolchain registration, project/language detection (`deps.edn`, `bb.edn`,
  `shadow-cljs.edn`, `build.clj`), config schema, `moon toolchain add` prompts,
  Docker metadata.
- Tier 2: implicit project relationships from `:local/root` coordinates (including
  alias `:extra-deps`), dependency-root location, `clojure -P` / `bb prepare`
  dependency preparation, manifest parsing (`:mvn/version`, `:local/root`,
  `:git/url` coordinates), and task hashing over the transitive `:local/root`
  manifest closure.
