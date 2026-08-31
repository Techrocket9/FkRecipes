# Implementation notes -- the FkRecipes build report

Working notes for the v1 build, 2026-08-31. This file is a deliverable equal to the code: every design deviation with evidence, every piece of ecosystem friction met while dogfooding FkLua's distribution machinery, every cross-language mirror gap, and the gate results. Unresolved issues are recorded here, never papered over. The charter is FkLua's agents/drafts/recipes-library-design.md (decisions resolved 2026-08-31).

## Ecosystem friction, graded

Each entry says what was tried, what happened verbatim, and the grade: CLEAN (worked as documented), AWKWARD (worked but cost something), MISLED (pointed the wrong way), BLOCKED (needed a workaround).

### The Go v0.1.0 require resolves: CLEAN

`go/go.mod` requires `github.com/Techrocket9/fklua/guest/go v0.1.0` with no replace. `go mod tidy` printed `go: downloading github.com/Techrocket9/fklua/guest/go v0.1.0`, exited 0, and wrote a two-line go.sum. At that moment the scaffold's guest.go imported the module behind `//go:build tinygo.wasm` and tidy kept the require (tidy considers all build tags). The real channel works first try. Caveat recorded by the Go review: once guest.go became a placeholder with no import, `go mod tidy` would DELETE the require and both go.sum lines; the require becomes load-bearing again when the emit layer imports fkdata. Do not run tidy in the placeholder window.

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

Every deviation lands here with the evidence that forced it.

- docs-style.md is carried as a copy of FkLua's with one section adapted: the licence-statements paragraph named FkLua's third-party inputs (testdata/spec, third_party/lua-5.2.1), which do not exist here, so it now states FkRecipes' own MIT position. Everything else is verbatim.
- The em-dash and en-dash ban is extended to working notes and every error message (the parents exempt working notes). Recorded in CLAUDE.md's critical rules.
- **max_level is not a unit field.** The design says CostOf copies "the whole unit including count_formula/max_level verbatim", but the engine keeps max_level on the technology prototype, beside unit, not inside it. A verbatim unit copy therefore cannot carry it. Resolution: World grew TechMaxLevel (a Value, since the engine's field is a number or the string "infinite"), and CostOf copies it onto the generated technology separately; count_formula rides inside the unit copy as the design expected. The unit copy itself is proven a copy by an unrecognised key surviving byte for byte.
- **The sketch's functional options became spec structs and typed 1-based handles**, so the two languages mirror field for field (Go zero values and Rust Default both mean absent) and a reference to an undeclared or foreign thing is refused rather than representable. The dropdown constructor is named DropdownSettingNeedingLocale; the name is the design's required mark that dropdown values have no inline localisation.
- **Overwrite is refused, not performed.** A planned item, recipe or technology whose prefixed name already exists in data.raw is a refusal, not a silent data:extend overwrite. This also closes a cycle-validator blind spot found in review: a duplicate node in the overlay would bind incoming edges to the stale copy and accept a real cycle. Consequence, to document for consumers: Emit runs from exactly one data-stage hook.
- **TechSpec.AfterTech is a surface addition.** The design's InsertBetween/After anchor only external names, so a plan could not chain its own technologies (the anchor probe against data.raw logged a drop and detached the chain). AfterTech anchors on a plan-declared technology by handle. An AfterTech edge cannot close a cycle: the handle only exists after the technology it names, so the edges run backwards through declaration order, and Before with AfterTech is refused so nothing existing is rewritten to require the new node. The argument is recorded at the branch in both halves.
- **A pre-existing cycle in the world's own tree is refused with the full path** even when the plan adds no edge to it. The design only demands the overlay walk; refusing the inherited cycle is strictly more information than the engine's eight-word abort, which the load was heading for anyway.

## The adversarial review rounds

Maintainer rule, applied retroactively: all Rust must pass adversarial review before commit; after the Rust rounds kept finding real defects, the maintainer extended the pass to the Go half, informed by the Rust catalogue. The Go round found three blockers the Rust pass structurally could not see: the per-Lib id counter was a plain package var (proven racy: 4662 duplicate ids under 20000 concurrent New calls, reopening silent cross-plan handle resolution for consumers running parallel tests; fixed with sync/atomic and a -race gate), all seven handle types were mutually convertible because Go treats identically-shaped structs as identical types (fkrecipes.BoolSettingRef(anIntSettingRef) compiled in a consumer package and silently hid a technology; fixed with distinct zero-size marker fields), and the two languages' shortest-round-trip float formatters break representation ties differently, so host transcripts diverged on real float paths including the verbatim CostOf unit copy (6118 divergences per 1.15M matched samples, with a formatter-parity test that could not fail; fixed by replacing both test formatters with one deterministic 17-significant-digit rule). It also found: Lib copyable by value with two copies appending into one backing array (fixed with a noCopy vet guard), specs aliasing caller slices so one unchanged Lib could give two different plans (fixed by copying on declaration), missing sign guards on every amount but the unit count plus a negative CraftTime silently swallowed (fixed with refusals), silent rounding of int64 amounts above 2^53 (fixed with a refusal at the exact boundary), a unit-shape guard on CostOf, Go returning live slices where Rust clones, and a nil-World panic (Go-only guard; a Rust &dyn World cannot be null). The TinyGo build tag, the message parity (every refusal string byte-identical across the halves), the full re-run of the Rust attack catalogue against Go, and the pure planner's TinyGo compile (12340 flash bytes, no reflect, no fmt, no sort) all held. After the guard round the library measures 13914 flash bytes (+12.8 percent, the guards and their message strings; sync/atomic cost 23 bytes, the handle markers cost zero).

One symmetry to leave alone: Go carries a nil-World guard that Rust cannot reach (&dyn World is never null), while Rust carries a zero-id Lib check that safe Rust cannot reach (Default is gone; kept for mirror symmetry with Go, whose zero value cannot be suppressed). These are opposite policies about unreachable guards, each right for its language; do not tidy either away.

Honest caveats from the closing round, recorded rather than smoothed: the noCopy guard on Lib converts a silent corruption into a vet finding, and copylocks is not in go test's default vet subset, so a consumer who never runs go vet can still copy a Lib and corrupt both copies' plans; that is the standard Go idiom's ceiling, stated so "fixed" is not read as "unrepresentable". A typed-nil World still panics (no interface nil check can see it; Go-inherent, no Rust equivalent). The defensive clones (Go cloning where Rust clones) carry no red proof, because the aliased lists are read-only downstream and removing the clones changes no output; that is mirror parity, not a fixed bug. The refusal-ordering fix's originally-briefed test shape was unreachable (two invalid no-name recipes refuse on the first result check before any duplicate scan) and the vacuous first test was replaced with the reachable shape. Two thresholds sit deliberately one apart: validation accepts amounts up to and including 2^53 while the transcript formatter's plain-digit path stops strictly below it, so the boundary value renders in scientific form, identically in both halves. Left open for the next phase: a declaration with a literally empty name is not refused yet. The first review (whole rust/ tree, differential-fuzzed against the Go mirror with matched LCG generators, 8000 cases, zero divergence) confirmed the core and found: two blockers (EnabledBy indexed without validation, a reachable panic from a foreign handle; the example placeholder not compiling for wasm because nothing linked the panic handler), three defects (the cycle blind spot above; in-range handles from another Lib silently resolving to the wrong declaration; no intra-plan chaining), and hardening items (per-Lib handle tags, non-finite float refusals including UnitSpec.Seconds, empty mod name refusals, a 100-node cap on the cycle path message, Go int widths moved to int64 because TinyGo's wasm int is 32 bits, fixture units re-sorted to the order fkdata actually hands back, resolver/profile/autoexamples added to rust/Cargo.toml, the host-gate comment corrected: cargo resolves and clones the git graph even for a host test run, so that gate needs the network once). One review item accepted as-is: the Op stream's payloads are public and therefore forgeable; the op stream is the emit layer's internal seam and a consumer forging ops is out of contract. One requested red proof was correctly refused by the implementer: a cycle through AfterTech edges is unreachable by construction (argument above).

One proof vacuity was found and fixed during the round: the guard keeping max_level off hand-rolled units could not go red because the fixture answered "no cap" for the empty name; the fixture now answers for any name, making the guard observable. Lesson recorded: a red proof that stays green indicts the test first.

The verification round then found the fix code's own gaps, and they were closed too: autoexamples was in the wrong TOML table (a [package] key under [lib], silently ignored and warning on every build including consumers'); Lib::default() and Go's zero-value Lib bypassed the new id tag (Rust dropped the Default derive, and both languages refuse a zero-id Lib at both entry points; clippy's new_without_default demand was refused with the reason inline, since a Default is exactly the defect); and plan_settings taking the mod name as a parameter could drift from what plan_data derives, so the parameter is gone and both entry points take the World. The AfterTech no-cycle comment was reworded to make the overlay the load-bearing defence. Phase totals: 31 mirrored test functions per language, 109 injected defects across all rounds, zero that failed to go red (stale patch strings after churn were restated and re-run). The reviewer's informational items stand recorded: World.recipe_exists is a new required method with no default (correct pre-1.0, the only implementor is the unwritten emit layer), and the cycle walk's linear node scan measures 658 microseconds at base-game scale, 42 ms at 3000 technologies.

## Repository decisions

- Trunk is `master` (renamed from the freshly-initialized `main` before the first commit; matches the house convention and FkLua's /blob/master/ link scheme).
- The Go example guest will be its own module and the Rust example guest is a workspace member with wasm-gated dependencies, so `go test ./...` and `cargo test` on the host never try to compile fkdata (which cannot build off-target in either language, by design).

## Gate results

Recorded per commit as the gates land. Bootstrap baseline, 2026-08-31:

- `cd go && go mod tidy` exit 0 (network fetch of the v0.1.0 tag)
- `cd go && go test ./...` exit 0 (scaffold test)
- `cd rust && cargo fetch` exit 0 (git resolution)
- `cd rust && cargo test` exit 0 (scaffold test, 1 passed)

Planner and validators, 2026-08-31, after the adversarial round:

- `cd go && gofmt -l .` empty, `go vet ./...` exit 0, `go test ./...` exit 0, `go test -race ./...` exit 0 (35 test functions; the four beyond Rust's 31 cover Go-native hazards that are compile-time impossibilities in Rust: concurrent ids, two aliasing shapes, nil World)
- `cd rust && cargo fmt --check` exit 0, `cargo test` exit 0 (31 passed), `cargo clippy --lib` exit 0
- `cd rust && cargo build --target wasm32-unknown-unknown --workspace` exit 0 (the library and the example placeholder both compile for the shipping target; this gate exists because cargo test alone builds the std host shape and reads green over a wasm break)
- Red proofs across the phase: over 130 proofs across five rounds (initial planner, max_level, Rust review fixes, zero-id and seam fixes, Go review fixes), mirrored across the languages where both halves share the defect, spanning injected defects, compile-failure proofs (handle forgeries, Lib copies under vet) and a -race witness; zero failed to go red, and stale patch strings after churn were restated and re-run rather than trusted.

## What v1 leaves open

Collected as the build proceeds.
