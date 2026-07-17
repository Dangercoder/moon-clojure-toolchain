# moon-clojure-toolchain

A **Clojure ([tools.deps](https://clojure.org/reference/deps_edn)) toolchain plugin** for the
[moon](https://moonrepo.dev) build system (v2), compiled to WASM.

It makes moon natively understand `deps.edn` / `bb.edn`, unlocking full incremental
builds for Clojure monorepos:

- **Implicit project graph** — `:local/root` coordinates become moon project
  dependencies automatically. No manual `dependsOn`, no hand-maintained task wiring:
  `moon run app:build` builds upstream libs first, `moon ci` only rebuilds what a
  change actually affects.
- **Correct cache invalidation** — every task hash includes a content hash of the
  project's *transitive* `:local/root` manifest closure (even manifests **outside**
  the moon workspace), so dependency changes anywhere in the closure bust the cache.
- **Dependency preparation** — moon's `InstallDependencies` action runs `clojure -P`
  (or `bb prepare` for babashka projects) before tasks, keyed to manifest changes.
- **Manifest intelligence** — `deps.edn`/`bb.edn` are parsed into moon's dependency
  model (`:mvn/version`, `:local/root`, `:git/url`+`:git/sha` coordinates, alias
  `:extra-deps`), which also feeds task hashing.
- **Project & language detection** — projects with `deps.edn`, `bb.edn`,
  `shadow-cljs.edn`, or `build.clj` are detected as `clojure`, and tasks running
  `clojure`/`clj`/`bb` are attached to the toolchain.

## Requirements

- moon **v2.x** (tested against v2.3.5)
- The `clojure` CLI (and a JDK) on `PATH` — and/or `bb` (babashka) for `bb.edn` projects.
  The plugin does not (yet) install these for you; see [Tier 3](#roadmap).

## Installation

Enable the toolchain in `.moon/toolchains.yml` — the plugin is downloaded
from this repo's GitHub releases:

```yaml
clojure:
  plugin: 'github://Dangercoder/moon-clojure-toolchain'         # latest release
  # plugin: 'github://Dangercoder/moon-clojure-toolchain@v0.1.0' # or pin a version
```

That's it. Projects with a `deps.edn`/`bb.edn` manifest are picked up automatically.
(In CI, set `GITHUB_TOKEN` to avoid GitHub API rate limits when the plugin is
first downloaded.)

Alternatively, build from source and point at the file:

```shell
rustup target add wasm32-wasip1
cargo build --target wasm32-wasip1 --release
```

```yaml
clojure:
  plugin: 'file:///path/to/target/wasm32-wasip1/release/clojure_toolchain.wasm'
```

## Configuration

All settings live under the `clojure` block in `.moon/toolchains.yml`
(and can be overridden per-project in `moon.yml` under `toolchain.clojure`):

```yaml
clojure:
  plugin: 'github://Dangercoder/moon-clojure-toolchain'

  # Turn :local/root coordinates into implicit moon project dependencies.
  inferRelationships: true

  # Also scan alias maps (:extra-deps, :replace-deps, :deps, :override-deps,
  # :default-deps) for :local/root coordinates. Alias-scoped deps get the
  # `development` scope; top-level :deps get `production`.
  includeAliasDeps: true

  # Fold every source file of the transitive :local/root dependency closure
  # into task hashes (each dependency's top-level :paths, tools.deps default
  # ["src"]) — not just the manifests. A dependency source edit then
  # invalidates consumer task caches even when the consumer's task `inputs`
  # don't list the dependency's sources.
  hashLocalSources: true

  # Aliases activated when preparing dependencies:
  # ["test", "build"] => `clojure -P -A:test:build`
  prepareAliases: []
```

moon's shared toolchain settings also apply, e.g. `installDependencies: false`
to disable the `clojure -P` step.

## How incremental builds work

Given a monorepo like:

```
libs/greeter/deps.edn
apps/cli/deps.edn      ; :deps {example/greeter {:local/root "../../libs/greeter"}}
```

1. `extend_project_graph` parses every project's manifests and emits
   `cli -> greeter` as an implicit production dependency. `moon run cli:build`
   with a `deps: ['^:build']` task runs `greeter:build` first; affected
   detection (`moon ci`, `moon query affected`) follows the same edge.
2. `hash_task_contents` walks the transitive `:local/root` closure from each
   project and folds `{manifest path: sha256}` into every task hash. Editing
   any `deps.edn` in the closure — including ones outside the moon workspace —
   invalidates the cache. (Out-of-workspace manifests that cannot be read from
   the WASM sandbox are tracked by path with an `unavailable` marker.)
3. With `hashLocalSources` (default on), the same walk also folds
   `{dependency source file: sha256}` into the hash — every file under each
   dependency's top-level `:paths` (tools.deps default `["src"]`; resources
   and `data_readers.clj` count, exactly like classpath contents). This is
   the Bazel action-input model at project granularity: moon has no execution
   sandbox, so a dependency source missing from a consumer's task `inputs`
   would otherwise fail STALE — a silently replayed cached green — rather
   than loud. Dot-prefixed entries plus `target/` and `node_modules/` are
   skipped; only the ORIGIN project's own sources are left to its `inputs`
   (which moon defaults to `**/*`). Per-file hash entries also mean
   `moon query hash-diff` names the exact dependency file behind a cache miss.
4. Task `inputs`/`outputs` handle source-level caching as usual; `.cpcache`
   is registered as the vendor dir and stays out of hashes and Docker scaffolds.

Recommended task shape (e.g. in `.moon/tasks/clojure.yml` or per-project `moon.yml`):

```yaml
tasks:
  build:
    command: 'clojure'
    args: ['-T:build', 'jar']
    deps: ['^:build']            # build upstream :local/root deps first
    inputs: ['deps.edn', 'build.clj', 'src/**/*', 'resources/**/*']
    outputs: ['target/**/*.jar']

  test:
    command: 'clojure'
    args: ['-M:test']
    inputs: ['deps.edn', 'src/**/*', 'test/**/*']
```

## Example workspace

[`example/`](./example) is a runnable two-project workspace (`libs/greeter`,
`apps/cli` with a `:local/root` dependency). After building the plugin:

```shell
cd example
moon run cli:build     # builds greeter first (implicit dep), then cli
moon run cli:build     # instant — everything cached
moon run cli:run       # Hello, moon!
```

Change `libs/greeter/src/greeter/core.clj` and re-run: both projects rebuild.
Check `moon query affected --downstream deep` to see the edge in action, and
`moon hash <task-hash>` to inspect the `clojure` toolchain's hash contribution.

## What's implemented (plugin API surface)

| Tier | Function | Behavior |
|---|---|---|
| 1 | `register_toolchain` | Detection via `deps.edn`, `bb.edn` manifests; `shadow-cljs.edn`, `build.clj`, `tests.edn` config globs; `clojure`/`clj`/`bb` executables; `.cpcache` vendor dir |
| 1 | `define_toolchain_config` | Config schema (validated, powers `moon toolchain info clojure`) |
| 1 | `initialize_toolchain` | `moon toolchain add clojure` prompts |
| 1 | `define_docker_metadata` | `clojure:temurin-21-tools-deps` default image + scaffold globs |
| 2 | `locate_dependencies_root` | Nearest `deps.edn`/`bb.edn` (each project is its own deps root) |
| 2 | `install_dependencies` | `clojure -P [-A:...]` / `bb prepare` |
| 2 | `extend_project_graph` | `:local/root` → implicit project dependencies |
| 2 | `parse_manifest` | `:deps` + alias deps → moon's dependency model |
| 2 | `hash_task_contents` | Transitive manifest + dependency source content hashes |

## Roadmap

- **Tier 3 (tool provisioning)**: embed a proto tool plugin so moon can install the
  Clojure CLI itself (blocked on cross-tool JDK dependency wiring — moonrepo/moon#2076).
  Until then, pin versions with [proto](https://moonrepo.dev/proto) (`openjdk` plugin +
  `asdf:clojure` backend) or bake them into CI images.
- `bb.edn` `:tasks` → inferred moon tasks.
- `shadow-cljs.edn` awareness beyond detection (source paths, `:deps true` delegation).

## Development

```shell
cargo build --target wasm32-wasip1 --release   # build the plugin
cargo test --no-default-features               # unit + WASM sandbox tests
```

The `wasm` cargo feature gates the `#[plugin_fn]` exports; parsers
(`deps_edn`, `closure`) compile natively for fast unit testing. Integration
tests load the built `.wasm` through `moon_pdk_test_utils` sandboxes.

## License

MIT
