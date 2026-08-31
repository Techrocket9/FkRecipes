# Implementation notes -- the FkRecipes build report

Working notes for the v1 build, 2026-08-31. This file is a deliverable equal to the code: every design deviation with evidence, every piece of ecosystem friction met while dogfooding FkLua's distribution machinery, every cross-language mirror gap, and the gate results. Unresolved issues are recorded here, never papered over. The charter is FkLua's agents/drafts/recipes-library-design.md (decisions resolved 2026-08-31).

## Ecosystem friction, graded

Each entry says what was tried, what happened verbatim, and the grade: CLEAN (worked as documented), AWKWARD (worked but cost something), MISLED (pointed the wrong way), BLOCKED (needed a workaround).

### The Go v0.1.0 require resolves: CLEAN

`go/go.mod` requires `github.com/Techrocket9/fklua/guest/go v0.1.0` with no replace. `go mod tidy` printed `go: downloading github.com/Techrocket9/fklua/guest/go v0.1.0`, exited 0, and wrote a two-line go.sum. The require survives tidy even though the only import of the module sits behind `//go:build tinygo.wasm` (tidy considers all build tags). The real channel works first try.

### The cargo git dependency finds fkdata: CLEAN

`rust/Cargo.toml` declares `fkdata = { git = "https://github.com/Techrocket9/fklua" }`, wasm-gated. `cargo fetch` updated the git repository and locked fkdata 0.1.0 plus its fk dependency at commit efcc71e8, exit 0. Cargo found the named package with no root Cargo.toml in that repo (the workspace root is guest/rust/) by scanning; fkdata's own `fk = { path = "../fk" }` resolved inside the fetched checkout. Cargo.lock is committed, matching FkLua's guest/rust convention.

### `fklua init --library` for two languages: CLEAN, with the refusal doing the teaching

Running `fklua init --library fkrecipes --lang go,rust` refuses with:

    fklua init: --library scaffolds ONE language per directory; a two-language library is two sibling trees with a mirror test between them (the fkipc arrangement), so run init --library twice, in sibling directories

Running it twice in `go/` and `rust/` scaffolded exactly the documented four Go files and two Rust files, refused nothing else, and printed the composition-contract closing message both times.

### The scaffold is control-stage flavored: MISLED (mildly), for a data-stage library

`init --library` has one flavor, and it is a control-stage library: the example surface is `OnTick`/`OnEvent(id, ptr) bool` routing, the guest half calls `fk.Log`, the Go guest imports `guest/go/fk`, the Rust Cargo.toml declares `fk` as the wasm-gated dependency, and the generated contract comments emphasize event routing and multiplayer join safety. A data-stage library needs `fkdata` in both manifests and has no events at all, so both dependency declarations and both guest halves were rewritten by hand. The parts that transfer anyway (route-never-own, the build-gate split, no_std under cfg, the fk/fkgc feature ban, the [patch] one-source note) transfer verbatim. A `--data` flavor, or one sentence in the closing message saying "a data-stage library swaps fk for fkdata", would have removed the detour.

### The scaffold's go.mod placeholder: AWKWARD

The generated `module fkrecipes` line carries a comment saying to rename it to the publishing path, which is right, but says nothing about the sibling-directory arrangement it itself prescribed one refusal earlier: a module living in the `go/` subdirectory of a repo must carry the `/go` suffix in its module path (`github.com/Techrocket9/fkrecipes/go`) or consumers cannot fetch it. Someone following the refusal message into `go/` and the comment's "rename to the path you will publish under" can write a path `go get` will never resolve. One sentence in the comment would close the gap.

### `; echo exit=$?` is a session convention, not an FkLua script convention

The instruction to never read a gate's exit code through a pipe is recorded here as adopted for operator commands. FkLua's scripts themselves guard differently (`set -euo pipefail`, redirect-to-log with `|| { cat log; return 1; }`, a FAIL accumulator); this repo's scripts follow the FkLua shape, and the `; echo exit=$?` capture is used when a human or agent runs a gate by hand.

## Design deviations

None yet. Every deviation lands here with the evidence that forced it.

- docs-style.md is carried as a copy of FkLua's with one section adapted: the licence-statements paragraph named FkLua's third-party inputs (testdata/spec, third_party/lua-5.2.1), which do not exist here, so it now states FkRecipes' own MIT position. Everything else is verbatim.
- The em-dash and en-dash ban is extended to working notes and every error message (the parents exempt working notes). Recorded in CLAUDE.md's critical rules.

## Repository decisions

- Trunk is `master` (renamed from the freshly-initialized `main` before the first commit; matches the house convention and FkLua's /blob/master/ link scheme).
- The Go example guest will be its own module and the Rust example guest is a workspace member with wasm-gated dependencies, so `go test ./...` and `cargo test` on the host never try to compile fkdata (which cannot build off-target in either language, by design).

## Gate results

Recorded per commit as the gates land. Bootstrap baseline, 2026-08-31:

- `cd go && go mod tidy` exit 0 (network fetch of the v0.1.0 tag)
- `cd go && go test ./...` exit 0 (scaffold test)
- `cd rust && cargo fetch` exit 0 (git resolution)
- `cd rust && cargo test` exit 0 (scaffold test, 1 passed)

## What v1 leaves open

Collected as the build proceeds.
