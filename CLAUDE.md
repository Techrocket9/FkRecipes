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
- **A check that asks the `World` anything is ENVIRONMENTAL and degrades loudly; a check that reads only the DECLARATION is an author bug and refuses by name.** That is the rule, it keys on what the check ASKS rather than on where an author would have noticed it (a fixture World that answers `ToolExists` true for a pack the shipped mod set demotes is green in development and red in play), and the exceptions are ENUMERATED rather than argued: an environmental check stays a refusal only where degrading would overwrite another mod's prototype, hand the engine something it refuses anyway, or invent a value the author never declared, plus one the threat model rather than the rule supplies, a site the game cannot reach at all. By name, those are the three `already exists in data.raw` sites and the two named-prototype probes (`place_result`, `ResultNamed`) in `validate`; `PlanData`'s two host-wiring sites (`was given a nil World`, `the mod name is empty`), which ask the World whether it is there at all; `checkCycles`' `a prerequisite cycle: ...`; the six `CostOf(...)` sentences; the copied unit whose pack LIST or pack ENTRY is in NEITHER engine form; the packless refusal below; `readDropdown`'s `holds "<v>", which is not one of its values`, which the engine resets before any stage runs so only a hand-edited file arrives there; and every declared-default post-condition, which is not environmental at all. `TestEveryRefusalSiteIsClassified` / `every_refusal_site_is_classified` holds every refusal site in the source to that table, counted, so a new refusal is a red suite rather than a silent policy change, and `TestTheTwoClassificationListsAgree` / `the_two_classification_lists_agree` holds the table to a SECOND, independently written list grouped by class, so a RELABEL needs two edits in two places (the class check alone cannot see one: a check that takes a resolution rather than a World passes the World test whatever it is labelled). **Do not write a total claim.** "A player is never refused" shipped once and was false; "nothing an environment does can stop the load" is the same mistake one level up. What IS true: a stored setting value the library cannot use behaves exactly as if the player had left the field alone, logs ONE `fkrecipes: ERROR: ...` line naming the setting and the screen it is fixed on, AND puts one trailing line into the emitted recipe's or technology's own `localised_description`, because THE LOG IS NOT A DISCLOSURE and a tooltip is one of the three places a player looks; an ENVIRONMENTAL degradation (a science pack dropped out of a copied unit, a cost falling back to the mod's own declared one, a merged amount clamped at a ceiling) gets the same trailing line from its own composer and a line of its own, and it is NOT a player's fallback, so it never names a field to go and fix. Every note is an ENGLISH LITERAL and never a locale key: measured on 2.0.77, an undefined key anywhere in a prototype description deletes the WHOLE description on the client, silently, while the dump every gate here reads still holds every byte of it. The SETTINGS side does compose keys, and every one of them goes out in the engine's alternatives form, `{"?", {"section.key"}, "raw"}`, through the single writer `localeRef` / `locale_ref`, with the raw fallback LAST because a plain string always resolves and would short circuit the key: a bare key there costs the setting its info icon and its whole tooltip, measured on a client, with exit 0 and nothing in the log. The destruction sentence rides on what MOVED and never on the prototype kind, because a recipe whose crafting time fell back emits a byte-identical ingredient list. Because the accumulated log ops never reach the host on a refused load, every refusal raised after resolution carries ONE added sentence when a stored value fell back, naming the first such setting in walk order: `. The stored value of <setting> could not be used, so the mod's own declaration applied.` It is a FACT AND NOT A ROUTE, and NO REFUSAL NAMES THE SETTINGS SCREEN any more, re-measured on the client this cycle: the `Error loading mods` dialog offers Disable listed mods, Disable all mods, Manage mods, Restart, Exit and a Reset mod settings checkbox; `Manage mods` has no Mod settings button and its Back returns to the same dialog; `Restart` relaunches into an identical dialog over a file whose sha256 has not moved. What was wrong there was the route, not the fact, so the sentence keeps the fact and sends nobody anywhere: see `fallbackFact` / `fallback_fact`, which is `res.fellBack`'s second reader (its first is the per-setting dedupe that keeps one log line per field). And an environment CAN still stop the load: a mod set that leaves a technology with no science pack the game has, and no author-declared cost to fall back on, is refused by name (measured in play on 2.0.77: an empty research unit is a FREE research rather than a stuck one, so emitting one is a balance change the player did not choose). The reasoning and the whole client walk are in `playerFallback` / `player_fallback` and in `agents/customizer-design.md` decision 2. The language itself is unchanged: it names the problem rather than guessing a substitute, and `testdata/ingredient-list/cases.txt` still pins every sentence it builds.
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
cd rust && cargo fmt --check  # formatting, the Rust twin of the gofmt row
cd rust && cargo test         # the Rust mirror of the same pure half. No wasm target needed; runs the
                              # public-surface witness in rust/tests/ as well as the crate's own suite
cd rust && RUSTFLAGS=-Dwarnings cargo clippy --workspace --all-targets
                              # lints AND warnings as errors, every target: a helper that only a wasm
                              # caller and a host test reach is cfg-gated to exactly those two, so a
                              # deleted caller is a build error here rather than a silenced warning
cd rust && cargo build --target wasm32-unknown-unknown --workspace
                              # every member compiles for the target it ships on; cargo test alone
                              # builds the std host shape and would read as green over a wasm break
scripts/run-mirror.sh         # the cross-language mirror: both example guests packaged with a freshly
                              # built fklua, run under lua52f against the strict stand-in, transcripts
                              # byte-compared and pinned by testdata/mirror/transcript.golden. Needs
                              # FKLUA_CHECKOUT (default ../FkLua), tinygo, cargo, jq and the checkout's
                              # bin/lua52f; anything missing fails loudly with the remedy. --update
                              # recaptures the golden and refuses to capture a divergent mirror.
                              # The stand-in DEMOTES military-science-pack from a tool to a plain item
                              # after base's own rows are extended, which is the measured modpack shape
                              # of finding 13: a copied research unit that kept it would be refused by
                              # the stand-in with the engine's own sentence, so this gate is where the
                              # copied-unit pack filter is proven end to end.
                              # IT IS ALSO THE ONLY GATE THAT COMPILES THE PACKAGED LUA, so a Go change
                              # that grows (*Lib).PlanData past Lua 5.2's per-function register ceiling
                              # (200 locals, 250 registers; resolve inlines into it) fails HERE with
                              # `function or expression too complex`, naming a Lua line and no Go symbol.
                              # That ceiling is NOT the jump limit the row below tracks, and the relay
                              # does not work around it: keep the post-resolution gate in
                              # afterResolution and keep PlanData thin.
                              # It also prints each guest's jump row out of the packaging report: the
                              # widest span BEFORE the relay, which is how near Lua's 18-bit limit
                              # the guest already is, and the block room, which is the ceiling this
                              # library meets as it grows. An fklua older than 2a541a7 writes no
                              # jumps object and the gate reports NOT RUN naming it
scripts/run-ingame.sh         # the engine gate: both packaged examples under a real Factorio via
                              # --dump-data, three runs per language: two on the declared defaults
                              # (which must agree, the determinism check) and one FLIPPED, with a
                              # mod-settings.dat written into the packaged mod by
                              # `fklua modsettings write` (the fklua this script already builds) from
                              # testdata/ingame/flipped.json (a typed ingredient list that takes a
                              # dropdown's chosen preset over, one that has no dropdown beside it,
                              # a research cost overridden whole whose PACK TEXT is a typo the
                              # language refuses), and cmp'd against
                              # testdata/ingame/flipped.golden.dat before any engine runs, so an
                              # upstream codec change refuses here by name. Hashes of BOTH
                              # normalised dumps are pinned per engine in
                              # testdata/ingame/dump-sha256.txt as two tagged
                              # rows, <version> default|flipped <data> <settings> <mod set>; the
                              # settings hash is the same on both rows because the settings dump
                              # holds prototypes, not values. The flipped row also keeps the
                              # mod-settings.dat the ENGINE rewrote and reads it back with
                              # `fklua modsettings read`, so what the engine kept is asserted
                              # rather than assumed; a dump says what the data stage did, only
                              # that file says which values survived. Re-asks the binary its
                              # version first; FACTORIO_USERDIR=/tmp/fkrecipes so a running game's
                              # lock cannot kill it; needs FACTORIO_BIN (or the Steam default),
                              # jq, and an FkLua checkout at c21ff07 or later via FKLUA_CHECKOUT
                              # (default ../FkLua) for the writer and the reader: an older one has
                              # no modsettings subcommand and the gate reports NOT RUN naming it.
                              # About 35 seconds. The two gates are COMPLEMENTS field by field:
                              # where one types into a text the other leaves it alone and lets the
                              # dropdown decide, and the research cost overridden whole here is
                              # overridden by ONE FIELD in the mirror. Each gate also carries one
                              # text the language refuses, an ingredient list in the mirror and a
                              # pack list here, and this run must EXIT 0 with the research priced on
                              # the chosen TIER's own packs and EXACTLY ONE `fkrecipes: ERROR: `
                              # line in the engine's log (the count is asserted, not the presence).
                              # A mod-set mismatch reports
                              # SKIPPED and exits 0 (an environmental difference, the FkLua
                              # convention); --strict or FKRECIPES_STRICT=1 makes it exit 1 for a CI
                              # job that only reads exit codes.
                              # It packages with --report and prints the same jump row as the
                              # mirror, once per language, before that language's first engine run;
                              # an fklua older than 2a541a7 carries no jumps object and the gate
                              # reports NOT RUN naming it, which makes 2a541a7 the checkout floor
                              # here rather than c21ff07
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
                        Cargo.lock pins that repository at b88965d (moved only by
                        `cargo update -p fkdata -p fk`, never by a replace or a patch). THAT PIN AND
                        THE HARNESS CAN DRIFT APART and currently have: the lock pins the fkdata
                        DEPENDENCY this crate compiles against, while run-mirror.sh and run-ingame.sh
                        build the `fklua` BINARY out of whatever the FKLUA_CHECKOUT sibling holds,
                        which is 01d640a as of 2026-09-13. Nothing reconciles the two, so a size or a
                        transcript figure names the head it was taken at. Syncing this repository
                        onto the newer head is owed as its own round; see
                        agents/implementation-notes.md. src/ingredient_list.rs is the language's
                        mirror; tests/ is the public-surface witness, a separate crate that sees only
                        what a consumer sees (it is what proved World was sealed by accident)
go/examples/datastage   the Go example guest, its own module (a consumer-shaped project; fkrecipes by
                        replace, the FkLua substrate by the real v0.2.0 require)
rust/examples/datastage the Rust example guest (workspace member), the mirror harness's Rust arm
go/examples/notext      the size-measurement fixtures: a BetterBeltBalancer-shaped guest (two legacy
rust/examples/notext    dropdowns over IngredientsBy and CostBy, no text setting) written twice, so "a
                        plan with no text setting links no PARSER, no RENDERER, no AMOUNT FORMATTER
                        and no CUSTOM-COST RESOLVER" is a number somebody can re-take with the
                        commands in agents/implementation-notes.md. Those four and not everything:
                        the description prose a text setting's composition holds is named from a
                        runtime branch every guest compiles in, so it ships in all four guests, and
                        the same section measures that beside them. The Go one is its own module and
                        only `go vet .` in its directory keeps it compiling; the Rust one is a
                        workspace member, so the wasm workspace build does. Nothing runs either
                        under an engine or the stand-in
scripts/                gate scripts: run-mirror.sh is the cross-language mirror, run-ingame.sh the
                        engine gate, and lib-report.sh the one copy both source. lib-report.sh reads
                        the jumps row out of `fklua mod --report` and prints it per language, and it
                        is where the remedy for an fklua too old to carry that row lives
testdata/mirror/        the strict engine-shaped stand-in and the committed transcript golden
testdata/locale/        the locale checker's committed fixture cfg and findings golden, the
                        cross-language pin that needs no toolchain (both suites reproduce it). The
                        golden holds the required findings and NOTHING ELSE: a key inside the mod's
                        prefix is required, and the one key composed outside it
                        (technology-name.<source>) is never required, because the locale namespace
                        is flat and requiring it would be requiring the squat this checker's own
                        collision scan exists to flag. That key gets an ADVISORY instead, out of
                        CheckLocaleAdvisories / check_locale_advisories, which is a report of its
                        own with a cap of its own and reads no .cfg at all; it is deliberately NOT
                        in CheckLocale's return, because docs/migration.md tells a consumer to
                        assert that report is empty and an advisory there would be a permanent red
                        test over a key the consumer is told not to define. Each half pins the
                        sentences in its own suite rather than in a third copy on disk
testdata/ingredient-list/ the ingredient-list language's corpus: every case an input and the exact
                        rendering or refusal, run by both suites byte for byte. It is the language's
                        CONTRACT: a message changes in the corpus first, then in both halves, never
                        in one half alone. Its header documents the fixture and the escape syntax
testdata/ingame/        the engine gate's per-engine golden (two tagged rows, default and flipped,
                        each two dump hashes and the mod set), flipped.json (the stored values the
                        flipped row installs) and flipped.golden.dat (the bytes
                        `fklua modsettings write` must produce for it, cmp'd on every run before
                        the engine is touched, so a codec change upstream fails here loudly and by
                        name; the same --update re-records it and the hash rows)
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
| [`agents/threat-model.md`](agents/threat-model.md) | **The scope every assessment grades to and every customizer decision is reviewed against.** Who the player is and the three places they look (the settings screen, the in-game tooltip, the changelog; never the log), the seven in-scope situations that must be CLEAN or engine-owned and disclosed, the five out-of-scope ones that are recorded and never block, the rubric with an owner and a scope letter per finding, the stopping rule, and the client checklist a decision touching recovery, rollback or saves needs before it is adopted. Read before grading, and before changing what a stored value does. |
| [`agents/customizer-design.md`](agents/customizer-design.md) | **The customizer round's design record.** Why a player-typed ingredient list, the engine facts it rests on (each with its probe), the surface, the semantics both halves share, the adversarial design review and what it changed, the harness. Read before touching the language, its corpus, or a text setting. |
