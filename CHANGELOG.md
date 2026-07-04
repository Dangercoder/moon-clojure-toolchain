# Changelog

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
