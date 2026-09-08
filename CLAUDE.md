# CLAUDE.md -- working context for FkRecipes

FkRecipes ("Factorio: konfigurierbare Recipes") is a guest library for mods built with FkLua: a mod declares user-configurable crafting recipes and research (recipes, technology-tree position, research cost, crafting time, each optionally driven by startup settings the library generates), in Go or Rust, and the library validates and emits at the data stage through fkdata. The design charter is FkLua's agents/drafts/recipes-library-design.md, decisions resolved 2026-08-31. Conventions are inherited from FkLua and BetterBeltBalancer; read FkLua's CLAUDE.md in a sibling checkout before working here. The rules below repeat only what is load-bearing daily.

## Critical rules

- **Rebase, never merge.** Trunk is `master`, history is linear, and `git merge --ff-only` is the guard: if it refuses, the branch was not rebased first.
- **Every behavioural test is red-proven.** The guarded code is broken on purpose, the designed failure message is observed, the break is reverted, and the commit message says so. A proof that fails to go red is a finding, not a formality.
- **Measured, not argued.** Every number in these notes carries the command that produced it. Engine behaviour (the crafting-time floor, refusal shapes) is probed on a real binary, never assumed, and the binary is re-asked its version (`"$FACTORIO" --version`) before any in-game gate.
- **Documentation drift is a gate failure, not a follow-up.** This file and the docs are updated in the same commit as the change they describe. Human-facing documents follow [`agents/docs-style.md`](agents/docs-style.md); run its grep check before committing.
- **A skipped gate reads exactly like a pass.** Anything a gate needs and cannot find (lua52f, a Factorio binary, a wasm toolchain) fails or prints a loud NOT RUN naming the remedy; it never exits 0 silently.
- **Determinism is a correctness property.** Plans are slices in declaration order; no map iteration anywhere in either language; everything host-visible is sorted or in declared order. Lua `pairs` order over string keys is seeded per run, so nothing may depend on it, in the library or in the harness.
- **Pin-transparency is the headline property.** The library imports fkdata and nothing else, in both languages: no fkapi under any feature, ever, and `fklua mod`'s data-module import check is the enforcement. The library never exports a stage hook (`fk_settings`, `fk_data`, ...): the consumer owns the exports and routes in.
- **Unprefixed names are unrepresentable.** The prefix derives from `fkdata.ModName()` at Emit; there is no prefix parameter, and nothing the library emits can carry a setting or prototype name it did not prefix.
- **Never test Lua against the Homebrew lua** (5.5, integer subtype; Factorio is doubles-only 5.2.1). The harness uses `bin/lua52f` from an FkLua checkout, located via `FKLUA_CHECKOUT` (default `../FkLua`).
- **No em-dashes or en-dashes anywhere in this repository**, working notes and error messages included. This deliberately extends docs-style.md's ban (the parents exempt working notes; this repo does not), so one grep covers everything.

## Gates

Run every gate that exists before a commit; a gate added by a commit is listed here in the same commit.

```sh
cd go && gofmt -l . | tee /dev/stderr | (! read)   # formatting; any filename is a failure
cd go && go vet ./...
cd go && go test ./...        # pure half on the host: planner, validators. No wasm toolchain needed
cd go && go test -race ./...  # the id counter is atomic for consumers' parallel tests; -race is what proves it stays so
cd go/examples/notext && go vet .   # the size fixture compiles on the host; nothing in scripts/ builds it
cd rust && cargo test         # the Rust mirror of the same pure half. No wasm target needed
cd rust && cargo build --target wasm32-unknown-unknown --workspace
                              # every member compiles for the target it ships on; cargo test alone
                              # builds the std host shape and would read as green over a wasm break
scripts/run-mirror.sh         # the cross-language mirror: both example guests packaged with a freshly
                              # built fklua, run under lua52f against the strict stand-in, transcripts
                              # byte-compared and pinned by testdata/mirror/transcript.golden. Needs
                              # FKLUA_CHECKOUT (default ../FkLua), tinygo, cargo and the checkout's
                              # bin/lua52f; anything missing fails loudly with the remedy. --update
                              # recaptures the golden and refuses to capture a divergent mirror
scripts/run-ingame.sh         # the engine gate: both packaged examples under a real Factorio via
                              # --dump-data, three runs per language: two on the declared defaults
                              # (which must agree, the determinism check) and one FLIPPED, with a
                              # mod-settings.dat written into the packaged mod by
                              # go run ./internal/modsettings/cmd/writesettings from
                              # testdata/ingame/flipped.json (edited ingredient texts, a custom arm
                              # on, a custom research cost). Hashes of BOTH normalised dumps are
                              # pinned per engine in testdata/ingame/dump-sha256.txt as two tagged
                              # rows, <version> default|flipped <data> <settings> <mod set>; the
                              # settings hash is the same on both rows because the settings dump
                              # holds prototypes, not values. Re-asks the binary its version first;
                              # FACTORIO_USERDIR=/tmp/fkrecipes so a running game's lock cannot
                              # kill it; needs FACTORIO_BIN (or the Steam default) and jq. About 38
                              # seconds. The mirror flips the settings the in-game row leaves on a
                              # preset and vice versa, so each dropdown is on custom in one gate and
                              # on a preset in the other. A mod-set mismatch reports SKIPPED and
                              # exits 0 (an environmental difference, the FkLua convention);
                              # --strict or FKRECIPES_STRICT=1 makes it exit 1 for a CI job that
                              # only reads exit codes
```

The mirror harness (both example guests packaged with `fklua mod`, run under lua52f against the strict stand-in, transcripts byte-compared) and the in-game `--dump-data` gate land with their own commits and get their rows here then.

## Where things live

This list is what exists. A path named here that is absent, or present and empty, is a bug in this file.

```
go/                     the Go half: module github.com/Techrocket9/fkrecipes/go, requiring
                        github.com/Techrocket9/fklua/guest/go v0.2.0 (the real channel; no replace).
                        Pure files host-testable; everything touching fkdata sits behind
                        //go:build tinygo.wasm. ingredientlist.go is the player-typed language
rust/                   the Rust half: crate fkrecipes, workspace root. fkdata arrives as a git
                        dependency on https://github.com/Techrocket9/fklua, wasm-gated so the host
                        cargo test needs no wasm target; the [patch] one-source note is in Cargo.toml.
                        src/ingredient_list.rs is the language's mirror
go/examples/datastage   the Go example guest, its own module (a consumer-shaped project; fkrecipes by
                        replace, the FkLua substrate by the real v0.2.0 require)
rust/examples/datastage the Rust example guest (workspace member), the mirror harness's Rust arm
go/examples/notext      the size-measurement fixtures: a BetterBeltBalancer-shaped guest (two legacy
rust/examples/notext    dropdowns over IngredientsBy and CostBy, no text setting) written twice, so "a
                        plan with no text setting links no ingredient language" is a number somebody
                        can re-take with the commands in agents/implementation-notes.md. The Go one is
                        its own module and only `go vet .` in its directory keeps it compiling; the
                        Rust one is a workspace member, so the wasm workspace build does. Nothing runs
                        either under an engine or the stand-in
scripts/                gate scripts; run-mirror.sh is the cross-language mirror
testdata/mirror/        the strict engine-shaped stand-in and the committed transcript golden
testdata/locale/        the locale checker's committed fixture cfg and findings golden, the
                        cross-language pin that needs no toolchain (both suites reproduce it)
testdata/ingredient-list/ the ingredient-list language's corpus: every case an input and the exact
                        rendering or refusal, run by both suites byte for byte. It is the language's
                        CONTRACT: a message changes in the corpus first, then in both halves, never
                        in one half alone. Its header documents the fixture and the escape syntax
testdata/ingame/        the engine gate's per-engine golden (two tagged rows, default and flipped,
                        each two dump hashes and the mod set), flipped.json (the stored values the
                        flipped row installs) and flipped.golden.dat (the bytes the codec must
                        produce for it; a layout change is a red test)
go/internal/modsettings the host-only mod-settings.dat codec (Encode, Decode, an order-preserving
                        JSON reader) and cmd/writesettings, inside the library module so go run
                        reaches it with no second module and no wasm toolchain; nothing the library
                        ships imports it, so pin transparency is untouched
docs/                   human-facing docs (usage.md, migration.md, ingredient-list.md; docs-style.md
                        governs, run its grep before commit)
agents/                 working notes; index below
LICENSE                 MIT
```

## Index of agents/

| File | Covers |
|---|---|
| [`agents/docs-style.md`](agents/docs-style.md) | **Read before creating or editing any human-facing document.** The public voice, the formatting bans, the pre-commit grep check. |
| [`agents/implementation-notes.md`](agents/implementation-notes.md) | **The build report.** Design deviations with evidence, ecosystem friction found while dogfooding the FkLua distribution channel, cross-language mirror gaps, gate results, what v1 leaves open. |
| [`agents/customizer-design.md`](agents/customizer-design.md) | **The customizer round's design record.** Why a player-typed ingredient list, the engine facts it rests on (each with its probe), the surface, the semantics both halves share, the adversarial design review and what it changed, the harness. Read before touching the language, its corpus, or a text setting. |
