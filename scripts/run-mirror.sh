#!/usr/bin/env bash
# THE MIRROR HARNESS: two guest libraries, one transcript.
#
# docs/library-parity.md's recipe, rebuilt out of tree. Two example guests, one
# per language, declare the SAME mod in the same order; each is packaged by
# `fklua mod` into a real Factorio mod and its settings and data stages are run
# under lua52f against one strict engine stand-in. The two transcripts are
# compared byte for byte, and then against a committed golden.
#
# WHY THIS AND NOT UNIT TESTS. The two halves already have mirrored unit tests,
# and those compare what each planner BELIEVES it will emit. This compares what
# a packaged mod actually does to a data.raw: through fkdata's codec, through
# the generated stage files, through Lua. A planner pair can agree perfectly
# and still differ here, which is the drift the parity recipe exists to catch.
#
# THE FRESH BUILD IS DELIBERATE. fklua embeds its runtime shim, so a stale
# binary tests a runtime nobody ships. It is rebuilt from the checkout on every
# run, into this repo's tmp, and NOTHING is ever written into the checkout.
#
# A MISSING GOLDEN FAILS. It does not skip and it does not silently record:
# BetterBeltBalancer's recorded trap is a gate that answers "nothing to compare
# against" with a pass. Regenerate deliberately with --update.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FKLUA_CHECKOUT="${FKLUA_CHECKOUT:-$ROOT/../FkLua}"
# THIS SCRIPT OWNS tmp/mirror AND NOTHING ELSE. It used to own all of tmp and
# wipe it, which took scripts/run-ingame.sh's tree with it: a concurrent
# in-game run lost its packaged mod mid-flight and reported a failure that
# blamed the engine. Each gate wipes only its own subtree.
TMP="$ROOT/tmp/mirror"
GOLDEN="$ROOT/testdata/mirror/transcript.golden"
STANDIN="$ROOT/testdata/mirror/standin.lua"
MODNAME=fkrecipes-example
MODVER=0.1.0

UPDATE=0
if [ "${1:-}" = "--update" ]; then UPDATE=1; fi

FAIL=0
fail() { echo "  FAIL: $*" >&2; FAIL=1; }
refuse() { echo "run-mirror: $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# What this needs, and the remedy for each. Nothing here is skipped quietly: a
# gate that cannot find its tools has to say so and fail, or it reads exactly
# like a pass.
# ---------------------------------------------------------------------------
[ -d "$FKLUA_CHECKOUT" ] || refuse "no FkLua checkout at $FKLUA_CHECKOUT; set FKLUA_CHECKOUT"
LUA52F="$FKLUA_CHECKOUT/bin/lua52f"
[ -x "$LUA52F" ] || refuse "no lua52f at $LUA52F; build it with: make -C $FKLUA_CHECKOUT lua52f"
command -v tinygo >/dev/null || refuse "tinygo is not on PATH; the Go guest cannot be built"
command -v cargo  >/dev/null || refuse "cargo is not on PATH; the Rust guest cannot be built"
command -v jq     >/dev/null || refuse "jq is not on PATH; the packaging report cannot be read"
[ -f "$STANDIN" ] || refuse "no stand-in at $STANDIN"
# ONE COPY of the jump-row reader, shared with scripts/run-ingame.sh: two gates
# packaging the same two guests must say the same thing about the same number.
LIBREPORT="$ROOT/scripts/lib-report.sh"
[ -f "$LIBREPORT" ] || refuse "no lib-report.sh at $LIBREPORT; it ships beside this script"
# shellcheck source=/dev/null
. "$LIBREPORT"

# EVERY RUN STARTS FROM NOTHING. A leftover in tmp/mirror survives a green run
# and is then indistinguishable from something this run produced: a poisoned
# bin/fklua (a text file, or worse a directory) bricks every later run behind a
# message about the build rather than about the leftover. Wiping costs a cold
# cargo build of the wasm dependencies and buys a run that means what it says.
rm -rf "$TMP"
mkdir -p "$TMP/bin"
FKLUA="$TMP/bin/fklua"

echo "== building fklua from $FKLUA_CHECKOUT"
( cd "$FKLUA_CHECKOUT" && go build -o "$FKLUA" ./cmd/fklua ) >"$TMP/fklua-build.log" 2>&1 ||
  { cat "$TMP/fklua-build.log" >&2; refuse "fklua did not build; if this persists, rm -rf $TMP"; }

# ---------------------------------------------------------------------------
# The two guests.
# ---------------------------------------------------------------------------
echo "== building the Go guest"
( cd "$ROOT/go/examples/datastage" &&
  tinygo build -target=wasm-unknown -scheduler=none -gc=leaking -opt=2 \
    -o "$TMP/datastage-go.wasm" . ) >"$TMP/build-go.log" 2>&1 ||
  { cat "$TMP/build-go.log" >&2; refuse "the Go guest did not build"; }

echo "== building the Rust guest"
( cd "$ROOT/rust" && CARGO_TARGET_DIR="$TMP/cargo" \
  cargo build --release --target wasm32-unknown-unknown -p datastage ) \
  >"$TMP/build-rust.log" 2>&1 ||
  { cat "$TMP/build-rust.log" >&2; refuse "the Rust guest did not build"; }
cp "$TMP/cargo/wasm32-unknown-unknown/release/datastage.wasm" "$TMP/datastage-rust.wasm"

# ---------------------------------------------------------------------------
# Package, run, transcribe. Both mods carry the SAME name, so the generated
# stage files and everything downstream of them are comparable.
# ---------------------------------------------------------------------------
package_and_run() {
  local lang="$1"
  local wasm="$TMP/datastage-$lang.wasm"
  local moddir="$TMP/mods-$lang"
  local report="$TMP/report-$lang.json"

  rm -rf "$moddir"
  "$FKLUA" mod --data-module "$wasm" --name "$MODNAME" --version "$MODVER" \
    --author Techrocket9 --factorio-version 2.0 \
    -o "$moddir" --report "$report" >"$TMP/pack-$lang.log" 2>&1 ||
    { cat "$TMP/pack-$lang.log" >&2; refuse "packaging the $lang guest failed"; }

  [ -f "$report" ] || refuse "fklua mod wrote no report for $lang"
  local ok
  ok="$(jq -r '.ok' "$report")"
  [ "$ok" = "true" ] || { cat "$report" >&2; fail "$lang: the packaging report says ok=$ok"; }

  # EXACTLY the two hooks the examples export. fklua generates a stage file
  # only for a hook the guest actually has, so this is what says the examples
  # export what they claim to, and that no data-updates or final-fixes stage
  # was wired behind our back.
  local hooks
  hooks="$(jq -r '.hooks.data | sort | join(",")' "$report")"
  echo "   $lang data hooks: $hooks"
  [ "$hooks" = "fk_data,fk_settings" ] ||
    fail "$lang: the wired data hooks are [$hooks], want exactly fk_data,fk_settings"
  local ctl
  ctl="$(jq -r '.hooks.control | length' "$report")"
  [ "$ctl" = "0" ] || fail "$lang: $ctl control hooks were wired by a data-only mod"

  # HOW NEAR LUA'S JUMP LIMIT THIS GUEST IS, and how much room the relay has
  # left inside it. Read here rather than measured: this library used to
  # transcribe the emitter's scan into a script of its own, and read the span
  # after the relay as headroom. See scripts/lib-report.sh.
  report_jumps "$lang" "$report"

  local inner
  inner="$(find "$moddir" -maxdepth 1 -mindepth 1 -type d | head -1)"
  [ -n "$inner" ] || refuse "$lang: the packaged mod has no directory under $moddir"
  [ -f "$inner/settings.lua" ] || fail "$lang: no settings.lua was generated"
  [ -f "$inner/data.lua" ] || fail "$lang: no data.lua was generated"
  # Written as `if`, not as `[ ] && fail`: an AND-list whose left side fails
  # is exempt from set -e in bash but its status is the line's, and a future
  # reader moving it under a pipeline would get a silent early exit.
  if [ -f "$inner/data-updates.lua" ]; then
    fail "$lang: data-updates.lua exists for a guest that exports no such hook"
  fi
  if [ -f "$inner/data-final-fixes.lua" ]; then
    fail "$lang: data-final-fixes.lua exists for a guest that exports no such hook"
  fi

  "$LUA52F" "$STANDIN" "$inner" >"$TMP/transcript-$lang.txt" 2>&1 ||
    { cat "$TMP/transcript-$lang.txt" >&2; refuse "$lang: the stages did not run"; }
}

for lang in go rust; do
  echo "== packaging and running the $lang mod"
  package_and_run "$lang"
done

# ---------------------------------------------------------------------------
# The comparisons, weakest last.
# ---------------------------------------------------------------------------
echo "== comparing the two transcripts"
if ! diff -q "$TMP/transcript-go.txt" "$TMP/transcript-rust.txt" >/dev/null; then
  fail "the Go and Rust transcripts differ; first differing line:"
  # DELIBERATELY NON-FATAL. diff exits 1 when it finds a difference, pipefail
  # gives that status to the pipeline, and as the last command of this if body
  # set -e would kill the script HERE: the accumulator, the banner and every
  # later comparison would be skipped by the very failure they exist to
  # report.
  diff "$TMP/transcript-go.txt" "$TMP/transcript-rust.txt" | head -6 >&2 || true
fi

# ---------------------------------------------------------------------------
# ANTI-VACUITY. A transcript that ran but proved nothing is the failure mode
# these greps exist for: each one names a decision that has to be visible.
# ---------------------------------------------------------------------------
T="$TMP/transcript-go.txt"
echo "== checking the transcript says something"
grep -q "TRANSCRIPT extend#" "$T" || fail "no prototype was ever extended"
# THE LADDER IS THE IN-GAME GATE'S NOW, and deliberately: every dropdown in this
# stand-in is overridden by the text beside it or left on a preset whose plan has
# no absent rung, so no ingredient ladder is walked here. The in-game gate walks
# them instead, one plan per row: the DEFAULT row walks the water plan's ladder
# and the FLIPPED row the oil plan's, and each asserts the fallback and the drop.
# What the mirror holds is the SWITCH, which no engine run can show twice.
#
# THE TYPED LIST IS WHAT REACHED THE RECIPE, and neither preset did. Matched on
# the whole ingredient list rather than on one amount: the recipe name is
# serialised after the ingredients, and an amount on its own also appears in an
# unrelated recipe.
typed_quench='"ingredients"={1={"amount"=2,"name"="steel-plate","type"="item"},2={"amount"=6,"name"="iron-stick","type"="item"}}'
oil_plan='"ingredients"={1={"amount"=2,"name"="steel-plate","type"="item"},2={"amount"=2,"name"="fkrecipes-example-steel-rivet","type"="item"}}'
water_plan='"ingredients"={1={"amount"=2,"name"="steel-plate","type"="item"},2={"amount"=4,"name"="fkrecipes-example-steel-rivet","type"="item"}}'
grep -qF "$typed_quench" "$T" || fail "the typed ingredient list did not reach the recipe"
if grep -qF "$oil_plan" "$T"; then
  fail "the dropdown choice the text set aside reached a prototype"
fi
if grep -qF "$water_plan" "$T"; then
  fail "the ingredient plan nobody chose reached a prototype"
fi
if grep -q '"tungsten-plate"' "$T"; then fail "an absent ingredient reached a prototype"; fi
# THE DROPDOWN DECIDES WHERE THE TEXT IS UNTOUCHED, which is the other side of
# the same switch on the other pair: chain-links is on long and
# chain-ingredients is not in the table at all.
long_links='"ingredients"={1={"amount"=8,"name"="fkrecipes-example-steel-rivet","type"="item"},2={"amount"=1,"name"="steel-plate","type"="item"}}'
grep -qF "$long_links" "$T" || fail "the preset did not apply while its text was untouched"
grep -q '"enabled"=false' "$T" || fail "no prototype came out disabled"
grep -q '"hidden"=true' "$T" || fail "the switched-off technology is not hidden"
grep -q '"energy_required"=7.5' "$T" || fail "the bound crafting time did not reach a recipe"
grep -q '"minimum_value"=0.002' "$T" || fail "the generated craft-time minimum is missing"
grep -q '"maximum_value"=120' "$T" || fail "the declared craft-time maximum is missing"
# The two prototype-field slots the consumer round added, on both the item and
# the recipe: an order the library has a slot for, and a REAL 2.0 recipe field
# it does not, passed through Extra verbatim.
grep -q '"order"="b\[steelworks\]-a\[rivet\]"' "$T" || fail "the item order did not reach a prototype"
grep -q '"order"="b\[steelworks\]-b\[quenching\]"' "$T" || fail "the recipe order did not reach a prototype"
grep -q '"allow_productivity"=true' "$T" || fail "the Extra passthrough did not reach a prototype"
# BOTH ONE-SIDED NumericSpec ARMS, which is what this pair is for: forging-time
# declares a maximum and no minimum, tempering-hold a minimum and no maximum.
# The second is also craft-time-bound, so its presence says the DECLARED
# minimum stood rather than being replaced by the generated floor-safe one.
min_only='{"default_value"=1.5,"minimum_value"=0.5,"name"="fkrecipes-example-tempering-hold","order"="ag","setting_type"="startup","type"="double-setting"}'
grep -qF "$min_only" "$T" || fail "the min-only setting is not in the transcript in its declared shape"
if grep -q '"maximum_value"[^,}]*,"name"="fkrecipes-example-tempering-hold"' "$T"; then
  fail "the min-only setting emitted a maximum it never declared"
fi
# The other side of every branch the golden is here to hold: a recipe nothing
# unlocks, a technology whose setting is ON, and a copied level cap.
grep -q '"name"="fkrecipes-example-salvaged-steel-rivet"[^}]*' "$T" ||
  fail "the recipe nothing unlocks is missing"
grep -q 'FINAL .*"fkrecipes-example-hardened-tips"[^}]*"enabled"=true' "$T" ||
  fail "the switched-on technology is not enabled from the start"
if grep -q '"fkrecipes-example-hardened-tips"[^}]*"hidden"' "$T"; then
  fail "a switched-on technology must carry no hidden field"
fi
# THE TIER STILL PLACES THE TECHNOLOGY and still pays for the fields the player
# left alone: the military ladder's source is the prerequisite, and the packs
# and the time in the unit are that source's own while the count is the
# player's. These name the GENERATED prototype rather than any unit field on its
# own: the stand-in's own rows carry those fields too, so a grep for the field
# alone would pass whether or not anything was copied.
grep -q '^TRANSCRIPT extend#[0-9]*.*"name"="fkrecipes-example-hardened-tips".*"unit"={"count"=45,"ingredients"={1={1="automation-science-pack",2=1},2={1="logistic-science-pack",2=1},3={1="military-science-pack",2=1}},"time"=30}' "$T" ||
  fail "the partial custom cost did not take the tier's packs and time beside the player's count"
grep -q '^TRANSCRIPT extend#[0-9]*.*"name"="fkrecipes-example-hardened-tips".*"prerequisites"={1="military-4"}' "$T" ||
  fail "the prerequisite did not move with the chosen tier"

# ---------------------------------------------------------------------------
# THE CUSTOMIZER, which is what this stand-in's settings table exists for. Each
# check below names one path a player can reach through the settings screen,
# and each one is a line the golden would otherwise hold silently.
# ---------------------------------------------------------------------------
# The FIELD IS THE WORD DEFAULT and never the rendered list: a mod that changes
# its list must not turn every player who never opened the screen into an
# edited-text player. The list lives in the description instead.
grep -q '"auto_trim"=true,"default_value"="default"' "$T" ||
  fail "no text setting came out with the reserved word as its default"
if grep -q '"auto_trim"=true,"default_value"="1 iron-plate"' "$T"; then
  fail "a text setting's default is a rendered list rather than the word default"
fi
# THE WHOLE TEXT DESCRIPTION, all five parameters, because the three lines after
# the list are the ones a player has nowhere else to read: the ceiling the
# parser enforces, which of the two fields is deciding, and what a text this
# library cannot use costs them. The prototype declares no maximum length of its
# own (the engine would store 98000 characters), so the sentence IS the limit as
# far as the screen goes; and the settings screen has no conditional visibility
# at all (measured), so the switch line is the only place the pairing is stated.
grep -qF '{1="",2={1="mod-setting-description.fkrecipes-example-rivet-ingredients"},3="\ndefault: 1 iron-plate",4="\nWrite internal names, as the default line above does, in at most 2000 characters.",5="\nWhile this says default this mod'"'"'s own list applies.",6="\nA text this mod cannot use is set aside and that default applies instead; the reason is in the log, or in the load error if the load stops anyway."}' "$T" ||
  fail "the text setting's composed description is not in the transcript"
# AND THE OTHER SWITCH LINE, on a text setting that HAS a dropdown beside it:
# the sentence names which way the settings screen sorts the two rather than
# guessing at a declaration order the consumer is free to choose.
grep -qF '5="\nWhile this says default the option chosen above applies; anything else applies instead of it."' "$T" ||
  fail "a text setting beside a dropdown does not say which option decides"
# AND ITS TWIN ON THE DROPDOWN, which is the half the player reads while they
# are looking at the picker rather than at the text field.
grep -qF '"\nThe setting below applies instead while it does not say default."' "$T" ||
  fail "the dropdown does not say the text setting beside it overrides it"
# A RESEARCH NUMBER STATES ITS RANGE, and beside a research dropdown it states
# what 0 means: both sentences, because a number with no ceiling and a 0 that
# silently defers are two different things nobody can guess from the screen.
grep -qF '3="\nA whole number from 0 to 100000. While it is 0 the option chosen above decides."' "$T" ||
  fail "a research number beside a dropdown does not say what 0 means"
grep -qF '3="\nA whole number from 1 to 600."' "$T" ||
  fail "a research number with no dropdown does not state its range"
# AN INGREDIENT PRESET IS TWO LINES, not one run of text in two vocabularies.
# The label is the consumer's display prose and the client truncates it at
# about 37 characters; the internal names the field beside it takes are on
# their own line, under the word a player acts on, so the copyable half is
# never the truncated half.
grep -qF '{1="",2="\n",3={1="string-mod-setting.fkrecipes-example-chain-links-long"},4="\n  type: 8 fkrecipes-example-steel-rivet, 1 steel-plate"}' "$T" ||
  fail "the dropdown's composed preset line is not in the transcript"
if grep -qF '4=": 8 fkrecipes-example-steel-rivet, 1 steel-plate"' "$T"; then
  fail "an ingredient preset line still joins the two vocabularies with a colon"
fi
# A TEXT THE LANGUAGE REFUSES, on the arm with no dropdown in front of it. An
# input the PLAYER controls never refuses the load on its own: the line names
# the setting, quotes the language's own sentence with the shared prefix trimmed
# off it, and says where to fix it. Before this the same text ended the load, so
# neither this line nor the recipe below could exist.
# AND THE SENTENCE ONLY A RECIPE'S INGREDIENT TEXT CARRIES, at the end of the
# same line: what the engine does to an assembling machine when the recipe it is
# running changes. The pack text and the two numbers do not carry it, and the
# grep below for the pack text's own line is what says so.
grep -q '^LOG fkrecipes: ERROR: fkrecipes-example-rivet-ingredients, entry 2 ("2 iron-stik"): no item or fluid is named iron-stik\. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart\. Changing a recipe empties an assembling machine'"'"'s input slots of anything the new list does not use\.$' "$T" ||
  fail "the refused ingredient text logged no ERROR line"
# THE LOG IS NOT A DISCLOSURE, which is what this line exists for: the whole
# composed localised_description of the recipe whose text was set aside, pinned
# verbatim, because the tooltip the player hovers is the only place they find
# out the recipe is not what they typed. English literals throughout: an
# UNDEFINED locale key anywhere in a prototype description deletes the WHOLE
# description on the client, silently, and the dump this transcript mirrors
# cannot see that happen (measured on 2.0.77).
#
# THE EXTEND LINES AND NOT THE WHOLE FILE, here and in the negative below: the
# stand-in's FINAL dump is one line holding every prototype at once, so a grep
# over the file would pass on the note landing anywhere at all.
grep '^TRANSCRIPT extend#' "$T" |
  grep -qF '"localised_description"={1="",2="The stored value of fkrecipes-example-rivet-ingredients could not be used, so this mod'"'"'s own choice applies instead. The reason is in the log. Changing a recipe empties an assembling machine'"'"'s input slots of anything the new list does not use."},"localised_name"={1="",2="Steel rivets"},"name"="fkrecipes-example-steel-rivet"' ||
  fail "the recipe whose text was set aside carries no note in its own description"
# AND A RECIPE NOTHING FELL BACK ON CARRIES NONE, which is what says the note is
# a consequence of the fallback rather than something every prototype now has.
if grep '^TRANSCRIPT extend#' "$T" |
  grep -F '"name"="fkrecipes-example-salvaged-steel-rivet"' | grep -q "could not be used"; then
  fail "a recipe with no fallback carries a note"
fi
# And what it landed on: the mod's OWN declared list, which is what "loaded with
# its own default instead" means in the prototype rather than only in the line.
grep -qF '"ingredients"={1={"amount"=1,"name"="iron-plate","type"="item"}}' "$T" ||
  fail "the refused text did not leave the recipe on the mod's own declared list"
# AND THE LINE THAT MUST NOT BE THERE FOR IT. A refused text is not a list that
# was read, so nothing may report it as one.
if grep -q "takes its ingredients from fkrecipes-example-rivet-ingredients" "$T"; then
  fail "a refused text was logged as a list the recipe took"
fi
# THE TEXT IS THE SWITCH, and this is the clause that says which choice it set
# aside. The ingredient override is TOTAL, so the clause is about the choice and
# not about what the dropdown still supplies.
grep -q '^LOG fkrecipes: fkrecipes-example-hardened-steel-plate-quenching takes its ingredients from fkrecipes-example-quench-ingredients: 2 steel-plate, 6 iron-stick; the fkrecipes-example-quench-medium choice oil is set aside$' "$T" ||
  fail "the text that took a dropdown's list over logged no set-aside clause"
# AND THE OTHER PAIR SAYS NOTHING AT ALL, because nobody typed into it: an
# untouched text is the reserved word and the dropdown decides, which is the
# pre-existing path and gets no line of its own.
if grep -q "takes its ingredients from fkrecipes-example-chain-ingredients" "$T"; then
  fail "an untouched text was logged as a list the recipe took"
fi
# NOTHING IS EDITED AND IGNORED ANY MORE. Every non-default value is live, so
# the sentence that used to say otherwise may not appear anywhere at all.
if grep -q "is edited, but" "$T"; then
  fail "a value was reported as edited and ignored, which nothing does now"
fi
# A COST PRESET NAMES ITS TECHNOLOGY THROUGH ITS LOCALE KEY, not by its
# internal name: the tooltip shows the player the technology's own name.
grep -qF '4=": cost of ",5={1="technology-name.military-4"}' "$T" ||
  fail "the cost dropdown's composed preset line does not name the technology through its locale key"
if grep -qF '": cost of military-4"' "$T"; then
  fail "a cost preset line still carries the internal technology name"
fi
# A PARTIAL CUSTOM COST: the count is the player's and the time and the packs
# are the tier's, in one unit, and the clause says what the tier still supplies
# rather than pretending the whole cost was overridden. The cost override is per
# FIELD, which is why this clause and the ingredient one are two sentences.
grep -q '^LOG fkrecipes: fkrecipes-example-hardened-tips takes its research cost from fkrecipes-example-tips-packs: count 45, time 30, packs 1 automation-science-pack, 1 logistic-science-pack, 1 military-science-pack; the fkrecipes-example-tips-research-tier choice military supplies what the settings leave at default$' "$T" ||
  fail "the partial custom cost logged no supplies-what-is-left clause"
# A RESEARCH COST THE PLAYER PRICED WHOLE, with no dropdown in front of it: two
# packs out of a TYPED list, the count and the seconds from their own settings,
# and the unit in the engine's short tuple form rather than the long ingredient
# form a recipe takes. This is the only place either harness pins a typed pack
# list reaching a unit without a Factorio binary.
grep -q '^LOG fkrecipes: fkrecipes-example-chain-forging takes its research cost from fkrecipes-example-chain-packs: count 25, time 12, packs 2 automation-science-pack, 1 logistic-science-pack$' "$T" ||
  fail "the custom research cost logged nothing"
grep -qF '"unit"={"count"=25,"ingredients"={1={1="automation-science-pack",2=2},2={1="logistic-science-pack",2=1}},"time"=12}' "$T" ||
  fail "the custom research unit did not reach the technology in the short tuple form"

grep -q "fkrecipes-example-hardened-steel" "$T" || fail "the generated technology is missing"
grep -q 'FINAL .*"logistics-2".*fkrecipes-example-hardened-steel' "$T" ||
  fail "the prerequisite splice is not visible in the final data.raw"

# ---------------------------------------------------------------------------
# The golden.
# ---------------------------------------------------------------------------
if [ "$UPDATE" = 1 ]; then
  # A GOLDEN IS ONLY EVER CAPTURED FROM A CLEAN RUN. Recording one while the
  # two languages disagree, or while an anti-vacuity check is failing, would
  # freeze the defect into the file the gate compares against.
  if [ "$FAIL" != 0 ]; then
    echo "  refusing to capture a golden from a failing mirror" >&2
  else
    cp "$TMP/transcript-go.txt" "$GOLDEN"
    echo "== golden updated: $GOLDEN"
  fi
elif [ ! -f "$GOLDEN" ]; then
  fail "no golden at $GOLDEN; capture it deliberately with: scripts/run-mirror.sh --update"
elif ! diff -q "$GOLDEN" "$TMP/transcript-go.txt" >/dev/null; then
  fail "the transcript does not match the golden; first differing line:"
  # Non-fatal for the reason the mirror comparison above gives.
  diff "$GOLDEN" "$TMP/transcript-go.txt" | head -6 >&2 || true
fi

if [ "$FAIL" != 0 ]; then
  echo "run-mirror: FAILED" >&2
  exit 1
fi
echo "run-mirror: OK"
