# Changelog

## 0.3.0

- **Generated dependency inputs (gazelle-style) — hand-maintained
  cross-project input globs are gone.** During project sync (part of every
  moon run, or `moon sync projects`), the toolchain now generates and owns a
  marker-delimited `fileGroups.localDeps` block in each Clojure project's
  `moon.yml`, listing the source-dir globs of its transitive `:local/root`
  closure (each dep's top-level `:paths`, tools.deps default `["src"]`).
  Tasks reference it once as `'@group(localDeps)'`; adding a `:local/root`
  dep to deps.edn regenerates the block on the next run. Mirrors
  rules_clojure's `gen_srcs`: build config is derived from the manifests,
  never hand-written, and rewritten only when its content changes.
- The block is a pure function of manifest contents (no filesystem existence
  checks), so regeneration is deterministic; pair it with a
  `git diff --exit-code -- '*moon.yml'` CI step to force regenerated blocks
  to be committed.
- Projects with no local deps are left untouched; files with a hand-written
  `fileGroups:` outside the managed block are skipped (add the markers inside
  your own arrangement to opt in).
- New toolchain setting `syncDependencyInputs` (default `true`) to opt out.

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
