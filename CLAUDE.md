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
- **A check that asks the `World` anything is ENVIRONMENTAL and degrades loudly; a check that reads only the DECLARATION is an author bug and refuses by name.** That is the rule, it keys on what the check ASKS rather than on where an author would have noticed it (a fixture World that answers `ToolExists` true for a pack the shipped mod set demotes is green in development and red in play), and the exceptions are ENUMERATED rather than argued: an environmental check stays a refusal only where degrading would overwrite another mod's prototype, hand the engine something it refuses anyway, or invent a value the author never declared, plus one the threat model rather than the rule supplies, a site the game cannot reach at all. By name, and with the reason PER ROW rather than a category claim the list cannot support: the three `already exists in data.raw` sites in `validate`, because degrading clobbers a stranger's prototype; the two named-prototype probes (`place_result`, `ResultNamed`) and the six `CostOf(...)` sentences in the same walk, because there is nothing declared to degrade to and the library would have to INVENT a value the author never wrote, into the author's own prototype; `PlanData`'s two host-wiring sites (`was given a nil World`, `the mod name is empty`), which ask the World whether it is there at all; `checkCycles`' ONE surviving `a prerequisite cycle: ...`, which is a ring holding no edge this plan made, the game's own technology tree looping without this mod in it (out of scope by I) and where the only alternative to the sentence is rewriting somebody else's tree; `readDropdown`'s `holds "<v>", which is not one of its values`, which the engine resets before any stage runs so only a hand-edited file arrives there; and every declared-default post-condition, which is not environmental at all. `TestEveryRefusalSiteIsClassified` / `every_refusal_site_is_classified` holds every refusal site in the source to that table, counted, so a new refusal is a red suite rather than a silent policy change, and `TestTheTwoClassificationListsAgree` / `the_two_classification_lists_agree` holds the table to a SECOND, independently written list grouped by class, so a RELABEL needs two edits in two places (the class check alone cannot see one: a check that takes a resolution rather than a World passes the World test whatever it is labelled). **Do not write a total claim.** "A player is never refused" shipped once and was false; "nothing an environment does can stop the load" is the same mistake one level up. What IS true: a stored setting value the library cannot use behaves exactly as if the player had left the field alone, logs ONE `fkrecipes: ERROR: ...` line naming the setting and the screen it is fixed on, AND puts one trailing line into the emitted recipe's or technology's own `localised_description`, because THE LOG IS NOT A DISCLOSURE and a tooltip is one of the three places a player looks. THAT RULE IS ALSO THE ONLY THING ANY OF THOSE FOUR SENTENCES MAY SAY THE OUTCOME IS, which is fix round 3 decision 2: BESIDE A DROPDOWN, A FIELD LEFT ALONE IS THE DROPDOWN'S CURRENTLY CHOSEN PRESET AND NOT THE MOD'S OWN DECLARED LIST, so `fallbackNote`, `textFallbackLine`, `playerFallback` and `fallbackFact` all say the field behaved as though it had been left alone and NONE of them says the mod's own default, choice, list or declaration applied (the consumer measured one byte-identical note over six different emitted ingredient lists). The word `default` in the settings line points at the SWITCH LINE `textSwitchLine` / `text_switch_line` composes one row above it, which already spells out which field decides; `fallbackFact` is worded differently from the other three because it decorates a REFUSAL and so may not say `the mod loaded` in any form; an ENVIRONMENTAL degradation (every way this library's own plan comes out different because of what the player's mod set does or does not have: a pack dropped, a copied cost unusable or unreadable, a research left with no pack at all, a RECIPE left with no ingredient at all, a merged amount clamped, an edge dropped to open a ring) gets the same trailing line from its own composer and a line of its own, and it is NOT a player's fallback, so it never names a field to go and fix. **THE SET IS HELD BY A TEST AND NOT BY THIS SENTENCE.** `TestEveryNoteCallSiteIsAccountedFor` / `every_note_call_site_is_accounted_for` walks the source for every composer handed to `noteOn` / `note_on`, holds it to a table carrying each sentence's byte length at an EMPTY argument slot and one line saying what the degradation is, and fails naming the composer in BOTH directions, so a degradation added without a note, or a note whose call was deleted, is a red suite rather than a prose list somebody forgot. The player's own `fallbackNote` is in that table too, marked as the one that is a player's fallback: a set with a hand-written exclusion in it is a set with a hole where the exclusion is. Every note is an ENGLISH LITERAL and never a locale key: measured on 2.0.77, an undefined key anywhere in a prototype description deletes the WHOLE description on the client, silently, while the dump every gate here reads still holds every byte of it. THE NOTE IS THE LITERAL AND THE COMPOSITION AROUND IT IS NOT, which is fix round 3's decision 4 and the one place a prototype references a key: a note with NO declared `Description` opens with `{"?", {"", {"<kind>-description.<emitted name>"}, "\n"}, ""}` out of `descriptionRef` / `description_ref`, because a prototype's own `localised_description` field WINS OVER the locale entry of the same name and the bare note DISPLACED the description of every author who wrote one in a `.cfg` rather than in the plan. It is safe for the reason the bare key is not: a concatenation group holding an undefined key is ITSELF a failed alternative (measured on 2.0.77), so the group and its separator newline vanish together and the note stands alone with no blank line. The key is ONE element and cannot be chunked, so it is DROPPED above 181 bytes of emitted recipe name or 177 of technology name, where it would break the 200-byte element ceiling. Those two keys are the author's OWN OPTIONAL entry and `CheckLocale` neither requires nor advises them. The SETTINGS side composes keys too, and every one of them goes out in the engine's alternatives form, `{"?", {"section.key"}, "raw"}`, through the single writer `localeRef` / `locale_ref`, with the raw fallback LAST because a plain string always resolves and would short circuit the key: a bare key there costs the setting its info icon and its whole tooltip, measured on a client, with exit 0 and nothing in the log. THE RAW-FALLBACK-LAST RULE IS BOTH WRITERS' and the source-property walk polices both shapes. The destruction sentence rides on what MOVED and never on the prototype kind, because a recipe whose crafting time fell back emits a byte-identical ingredient list. Because the accumulated log ops never reach the host on a refused load, every refusal raised after resolution carries ONE added sentence when a stored value fell back, naming the first such setting in walk order: `. The stored value of <setting> could not be used and was set aside, so what applied is what that field gives when it is left alone.` It is a FACT AND NOT A ROUTE, and NO REFUSAL NAMES THE SETTINGS SCREEN any more, re-measured on the client this cycle: the `Error loading mods` dialog offers Disable listed mods, Disable all mods, Manage mods, Restart, Exit and a Reset mod settings checkbox; `Manage mods` has no Mod settings button and its Back returns to the same dialog; `Restart` relaunches into an identical dialog over a file whose sha256 has not moved. What was wrong there was the route, not the fact, so the sentence keeps the fact and sends nobody anywhere: see `fallbackFact` / `fallback_fact`, which is `res.fellBack`'s second reader (its first is the per-setting dedupe that keeps one log line per field). EVERY REFUSAL REACHABLE FROM THE SCOPE-F MOD SET THE CONSUMER'S THIRD ASSESSMENT MEASURED IS A DEGRADATION NOW, which is the narrow statement and not a total one: a technology left with no science pack the game has is EMITTED with an empty research unit (measured in play on 2.0.77 and recorded here: such a research loads with exit 0, `force.add_research` returns true, progress advances in a lab holding nothing, and it COMPLETES after `count * time` ticks consuming nothing, so it is a FREE research rather than a stuck one, which is a balance change the player did not choose and is therefore disclosed in the technology's own tooltip rather than paid for with a locked-out game); a copied research unit whose pack list is in neither engine form is priced at the mod's own declared cost where the author declared one and emitted with an empty ingredient list where they did not; and a prerequisite ring closed by an edge this plan made drops that edge, the first in the ring's own order, and walks again. What a MOD SET can still stop the load over is the enumeration above and nothing beyond it, and the summary is FOUR rows rather than three because "a bare name" does not cover the unit-shape sentences a literal reader would miss: a name this plan would overwrite in `data.raw`; a bare name with nothing declared behind it (`place_result`, `ResultNamed`, and a `CostOf` naming a technology that is not there); a `CostOf` naming a technology that IS there whose unit this library cannot copy (the other five of the six sentences: no unit, a `research_trigger` with none, a unit that is not a dictionary, a unit table it cannot copy faithfully, a `max_level` table it cannot copy faithfully); and a ring this plan is not part of. The reasoning and the whole client walk are in `playerFallback` / `player_fallback` and in `agents/customizer-design.md`, first-series decision 2 as amended by fix round 3's decision 1. The language itself is unchanged: it names the problem rather than guessing a substitute, and `testdata/ingredient-list/cases.txt` still pins every sentence it builds.
- **Nothing this library composes can exceed the engine's localised-string ceiling, and it is answered by construction rather than by keeping sentences short.** 200 BYTES PER STRING ELEMENT on a DATA-STAGE prototype, the key slot and every literal parameter alike, bytes not characters, no aggregate budget (measured on 2.0.77; a SETTING prototype is exempt, which is what scopes the rule). The note a recipe carries to disclose a fallback is 229 bytes before any name goes into it, so until this was answered a player's typo stopped the load with `Localised string key is too large` and no dump, which is exactly the lock-out the fallback exists to prevent. Every literal reaching an item's, a recipe's or a technology's `localised_name` or `localised_description` now goes through ONE splitter per half (`chunkLocalised` / `chunk_localised`), which fills elements to at most 180 bytes at word boundaries and hands the result to `localisedGroup` / `localised_group` for the 20-parameter nesting. `TestNoCompositionReachesTheElementCeiling` / `no_composition_reaches_the_element_ceiling` walk every composition the library can produce with 200-byte names, `testdata/mirror/standin.lua` refuses what the engine refuses, and the engine gate pins a chunked note read out of a real dump. THE ONE THING THE SPLITTER CANNOT ANSWER IS A KEY, because a key is ONE element by definition: the `<kind>-description.<emitted name>` reference a note composes is DROPPED above 181 bytes of emitted recipe name or 177 of technology name rather than chunked, and the ceiling walk carries one of each kind named at exactly the fitting length so it measures a real 200-byte key element.
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
                              # --dump-data, FOUR runs per language. Three are on the golden's own
                              # mod set: two on the declared defaults
                              # (which must agree, the determinism check) and one FLIPPED, with a
                              # mod-settings.dat written into the packaged mod by
                              # `fklua modsettings write` (the fklua this script already builds) from
                              # testdata/ingame/flipped.json (a typed ingredient list that takes a
                              # dropdown's chosen preset over, one that has no dropdown beside it,
                              # a research cost overridden whole whose PACK TEXT is a typo the
                              # language refuses, and an INGREDIENT TEXT behind a preset that is
                              # a typo too, so both channels of a refused player text are walked
                              # on a real engine), and cmp'd against
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
                              # The fourth is the DEMOTE ARM, one run per language under a SECOND
                              # MOD SET: the Lua-only fixture at testdata/ingame/demote/ moves
                              # automation-science-pack out of data.raw.tool and into data.raw.item
                              # at data.lua and repairs base's own technologies and labs around it
                              # at data-final-fixes (by PREFIX, so the guest's own prototypes are
                              # left for the library to handle), which is the measured modpack shape
                              # of findings 13 and 14 of the consumer's third assessment. The
                              # packaged guest's info.json gains `? fkrecipes-demote` so the
                              # fixture's data stage runs BEFORE the guest's. The arm asserts exit 0
                              # (its refusal names those two findings), that the fixture actually
                              # demoted the pack, both `fkrecipes: ERROR: ` lines in BOTH languages'
                              # logs, both emitted units empty through a term that names the
                              # prototype, both tooltips pinned WHOLE, the neighbouring copied-unit
                              # drop line, and that the two languages agree on the normalised dump.
                              # THERE IS NO GOLDEN ROW FOR IT because the mod set is not the
                              # golden's, and a hash here would describe a world no other row
                              # describes; what is compared instead is the two LANGUAGES against
                              # each other. It uses refuse where the hash rows use SKIPPED, because
                              # it asserts nothing against a golden and the pack it demotes is
                              # base's own.
                              # About 50 seconds wall (measured at this commit). The two gates are
                              # COMPLEMENTS field by field:
                              # where one types into a text the other leaves it alone and lets the
                              # dropdown decide, and the research cost overridden whole here is
                              # overridden by ONE FIELD in the mirror. This run must EXIT 0 with
                              # the research priced on the chosen TIER's own packs, the quench
                              # recipe on the preset its dropdown names, and EXACTLY TWO
                              # `fkrecipes: ERROR: ` lines in the engine's log, one per refused
                              # text (the count is asserted, not the presence). It also pins the
                              # quench recipe's four description ELEMENTS whole, and asserts that
                              # no element of any localised string in either DATA dump is over the
                              # engine's 200-byte ceiling. The SETTINGS dump is deliberately not
                              # walked for that: a setting prototype is exempt (measured to 5000
                              # bytes), and the three ladder lines this library composes onto a
                              # setting's description are 256, 268 and 269 bytes, so walking it
                              # would enforce a rule the engine does not have.
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
                        Cargo.lock pins that repository at 9709989 (moved only by
                        `cargo update -p fkdata -p fk`, never by a replace or a patch), the same
                        head the harness builds fklua from as of 2026-09-14. The pin and the
                        harness CAN drift apart, because the lock pins the fkdata DEPENDENCY
                        this crate compiles against while run-mirror.sh and run-ingame.sh build
                        the `fklua` BINARY out of whatever the FKLUA_CHECKOUT sibling holds;
                        nothing reconciles the two, a size or transcript figure names the head it
                        was taken at, and the pin can only follow a head that is on GitHub.
                        src/ingredient_list.rs is the language's
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
testdata/mirror/        the strict engine-shaped stand-in and the committed transcript golden. The
                        stand-in polices the 200-byte-per-element localised-string ceiling on every
                        data-stage prototype and skips the four setting types, which is measured
                        rather than assumed: a setting prototype is exempt on the engine
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
testdata/ingame/demote/ the demote arm's Lua-only fixture mod: info.json, data.lua (which moves
                        automation-science-pack from data.raw.tool to data.raw.item and keeps every
                        other field) and data-final-fixes.lua (which takes the demoted pack out of
                        every technology's unit EXCEPT the guest's own, by prefix, and out of every
                        lab's inputs, so the base game still loads and the row is scope F rather
                        than scope I). It has no golden row: its mod set is not the golden's
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
| [`agents/threat-model.md`](agents/threat-model.md) | **The scope every assessment grades to and every customizer decision is reviewed against.** Who the player is and the three places they look (the settings screen, the in-game tooltip, the changelog; never the log, and never the portal README either, with the reason), the seven in-scope situations that must be CLEAN or engine-owned and disclosed, the five out-of-scope ones that are recorded and never block, the rubric with an owner and a scope letter per finding, the stopping rule, and the client checklist a decision touching recovery, rollback or saves needs before it is adopted. Read before grading, and before changing what a stored value does. |
| [`agents/customizer-design.md`](agents/customizer-design.md) | **The customizer round's design record.** Why a player-typed ingredient list, the engine facts it rests on (each with its probe), the surface, the semantics both halves share, the adversarial design review and what it changed, the harness. Read before touching the language, its corpus, a text setting, or any line this library composes onto a setting's description. |
