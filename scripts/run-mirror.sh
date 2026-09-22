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
grep -q '^TRANSCRIPT extend#[0-9]*.*"name"="fkrecipes-example-hardened-tips".*"unit"={"count"=45,"ingredients"={1={1="automation-science-pack",2=1},2={1="logistic-science-pack",2=1}},"time"=30}' "$T" ||
  fail "the partial custom cost did not take the tier's packs and time beside the player's count"
# TWO OF THE TIER'S THREE PACKS AND NOT THREE, which is the degradation this
# stand-in demotes a pack to reach: military-science-pack is an ITEM here and no
# longer a TOOL, so the verbatim copy of military-4's unit loses it with a line
# of its own. Without the filter the stand-in refuses the load with the engine's
# own sentence ("Invalid research unit (military-science-pack). Research unit(s)
# can only be tool type items at the moment."), naming neither this mod nor any
# setting, which is finding 13 exactly.
grep -q '^LOG fkrecipes: hardened-tips: military-science-pack is not a science pack this game has, so it is left out of the military-4 cost$' "$T" ||
  fail "the demoted science pack was not dropped out of the copied unit"
# AND THE PLAYER IS TOLD WHERE THEY LOOK. A dropped pack is presence a player
# cannot check, so the technology's own tooltip carries the note beside the
# author's description, and it carries NO assembling-machine sentence: repricing
# a research destroys nothing.
grep '^TRANSCRIPT extend#' "$T" |
  grep -qF '"localised_description"={1="",2="Every level puts a harder edge on the same tools.",3="\nThis game has no military-science-pack, so this research was priced without it. The reason is in the log."}' ||
  fail "the technology whose copied cost lost a pack carries no note in its own description"
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
# THE WHOLE TEXT DESCRIPTION, all six parameters, because the four lines after
# the list are the ones a player has nowhere else to read: that the list the
# game builds from it can be shorter than the list shown, the ceiling the parser
# enforces, which of the two fields is deciding, and what a text this library
# cannot use costs them. The prototype declares no maximum length of its
# own (the engine would store 98000 characters), so the sentence IS the limit as
# far as the screen goes; and the settings screen has no conditional visibility
# at all (measured), so the switch line is the only place the pairing is stated.
# THIS IS THE STANDALONE SHAPE, and it carries the ladder line: beside an
# INGREDIENT dropdown it is the dropdown that renders the lists, and the check
# further down says the text setting there carries no ladder line at all.
grep -qF '{1="",2={1="?",2={1="mod-setting-description.fkrecipes-example-rivet-ingredients"},3="fkrecipes-example-rivet-ingredients"},3="\ndefault: 1 iron-plate",4="\nAn entry your mods lack takes the mod'"'"'s next name for it or is left out; two landing on one name are added, so what you craft can be shorter than shown.",5="\nInternal names, as on the default line, up to 2000 characters. The word none empties the list, so the recipe costs nothing to craft.",6="\nLeave this as default and this mod'"'"'s own list applies; anything else applies instead.",7="\nText this mod cannot use is set aside as though it said default; the reason is in the log or the load error."}' "$T" ||
  fail "the text setting's composed description is not in the transcript"
# AND THE PACKS TWIN, WHOLE, which is the same six parameters with TWO
# differences: its format line stops at the ceiling and does not name the word
# none, and its ladder line is in the packs vocabulary. A packs list REFUSES none ("research takes at least one science pack"),
# so a description naming it there would be telling a player to type a word this
# library turns down. The two greps below say the same thing negatively, per
# packs setting, so a clause that leaked would be caught even if this whole
# pin were re-recorded around it.
grep -qF '{1="",2={1="?",2={1="mod-setting-description.fkrecipes-example-chain-packs"},3="fkrecipes-example-chain-packs"},3="\ndefault: 1 automation-science-pack",4="\nA pack your mods lack takes the mod'"'"'s next name for it or is left out; two landing on one pack are added, so the research can take fewer packs than shown.",5="\nInternal names, as on the default line, up to 2000 characters.",6="\nLeave this as default and this mod'"'"'s own list applies; anything else applies instead.",7="\nText this mod cannot use is set aside as though it said default; the reason is in the log or the load error."}' "$T" ||
  fail "the packs setting's composed description is not in the transcript"
for packs in fkrecipes-example-chain-packs fkrecipes-example-tips-packs; do
  if grep '^TRANSCRIPT extend' "$T" | grep -F "\"name\"=\"$packs\"" | grep -qF 'The word none'; then
    fail "the packs setting $packs names a word the library refuses there"
  fi
done
# ONE WHOLE WRAPPED REFERENCE, PINNED ON ITS OWN. Every composed locale key
# rides in the engine's alternatives form with the raw fallback LAST, and a
# regression to the bare {"section.key"} is invisible everywhere a headless run
# can look: the engine's dump holds either table verbatim, exit 0, no warning
# and no fkrecipes: line, while on the CLIENT an undefined key costs the setting
# its info icon and its whole tooltip (measured, 2.0.77 build 84539). So the
# shape is pinned here, and the bare form is refused by name below.
grep -qF '{1="?",2={1="mod-setting-description.fkrecipes-example-rivet-ingredients"},3="fkrecipes-example-rivet-ingredients"}' "$T" ||
  fail "the composed description key is not in the engine's alternatives form"
for bare in '{1="",2={1="mod-setting-description.' '3={1="string-mod-setting.' '5={1="technology-name.'; do
  if grep -qF "$bare" "$T"; then
    fail "a composed locale reference went out bare rather than wrapped: $bare"
  fi
done
# AND THE OTHER SWITCH LINE, on a text setting that HAS a dropdown beside it:
# the sentence names which way the settings screen sorts the two rather than
# guessing at a declaration order the consumer is free to choose.
grep -qF '5="\nLeave this as default and the option chosen above decides; anything else applies instead."' "$T" ||
  fail "a text setting beside a dropdown does not say which option decides"
# AND ITS TWIN ON THE DROPDOWN, which is the half the player reads while they
# are looking at the picker rather than at the text field.
grep -qF '"\nThe setting below applies instead while it does not say default."' "$T" ||
  fail "the dropdown does not say the text setting beside it overrides it"
# A RESEARCH NUMBER STATES ITS RANGE, and beside a research dropdown it states
# what 0 means: both sentences, because a number with no ceiling and a 0 that
# silently defers are two different things nobody can guess from the screen.
grep -qF '3="\nLeave this at 0 and the option chosen above supplies the number; otherwise a whole number up to 100000."' "$T" ||
  fail "a research number beside a dropdown does not say what 0 means"
grep -qF '3="\nA whole number from 1 to 600."' "$T" ||
  fail "a research number with no dropdown does not state its range"
# AN INGREDIENT PRESET IS TWO LINES, not one run of text in two vocabularies.
# The label is the consumer's display prose and the client truncates it at
# about 37 characters; the internal names the field beside it takes are on
# their own line, under the word a player acts on, so the copyable half is
# never the truncated half.
grep -qF '{1="",2="\n",3={1="?",2={1="string-mod-setting.fkrecipes-example-chain-links-long"},3="long"},4="\n  to type: 8 fkrecipes-example-steel-rivet, 1 steel-plate"}' "$T" ||
  fail "the dropdown's composed preset line is not in the transcript"
if grep -qF '4=": 8 fkrecipes-example-steel-rivet, 1 steel-plate"' "$T"; then
  fail "an ingredient preset line still joins the two vocabularies with a colon"
fi
# AND THE LADDER SITS BETWEEN THE LAST PRESET AND THE SWITCH LINE, which is
# where it belongs: the line is about the lists directly above it, so a sentence
# wedged between two presets would point at a list that is not the last one.
grep -qF '5="\nAn entry your mods lack takes the mod'"'"'s next name for it or is left out; two landing on one name are added, so an option can craft a shorter list than it shows.",6="\nThe setting below applies instead while it does not say default."' "$T" ||
  fail "an ingredient dropdown's ladder line does not sit under its last preset"
# AND THE LADDER IS DISCLOSED BY THIS LIBRARY RATHER THAN BY THE CONSUMER. A
# resolve-or-drop ladder gets no note on the emitted recipe, deliberately,
# because it is the advertised contract; until this line existed the only text
# saying so was a locale entry the PILOT CONSUMER happened to write, which a
# consumer is free to write differently or not at all. Three clauses, because
# the ladder does three things: the mod's next name for the entry where there is
# one, the entry left out where there is not, and two entries landing on one
# name having their amounts added.
grep -qF '"\nAn entry your mods lack takes the mod'"'"'s next name for it or is left out; two landing on one name are added, so an option can craft a shorter list than it shows."' "$T" ||
  fail "an ingredient dropdown does not disclose what a name this game lacks costs the list"
grep -qF '"\nAn entry your mods lack takes the mod'"'"'s next name for it or is left out; two landing on one name are added, so what you craft can be shorter than shown."' "$T" ||
  fail "an ingredient text setting does not disclose what a name this game lacks costs the list"
grep -qF '"\nA pack your mods lack takes the mod'"'"'s next name for it or is left out; two landing on one pack are added, so the research can take fewer packs than shown."' "$T" ||
  fail "a packs text setting does not disclose what a pack this game lacks costs the research"
# AND THE PACKS VOCABULARY IS THE PACKS ONE. An ingredient setting's arm names
# an entry and a craft, a packs setting's names a pack and the research, because
# a packs field takes nothing else; the two greps below catch either arm
# composed onto the other kind. The closing clause is what they read, because
# that is the whole of the difference between the two ingredient arms as well.
for packs in fkrecipes-example-chain-packs fkrecipes-example-tips-packs; do
  if grep '^TRANSCRIPT extend' "$T" | grep -F "\"name\"=\"$packs\"" | grep -qF 'so what you craft can be shorter than shown'; then
    fail "the packs setting $packs carries the ingredient vocabulary of the ladder line"
  fi
done
for ing in fkrecipes-example-rivet-ingredients fkrecipes-example-quench-ingredients; do
  if grep '^TRANSCRIPT extend' "$T" | grep -F "\"name\"=\"$ing\"" | grep -qF 'so the research can take fewer packs'; then
    fail "the ingredient setting $ing carries the packs vocabulary of the ladder line"
  fi
done
# AND NOT ON A TEXT SETTING WITH AN INGREDIENT DROPDOWN BESIDE IT, which is
# where the line moved FROM: the lists a player there is choosing between are
# the dropdown's presets, so the dropdown's own row is where the ladder is
# disclosed and a second copy on the text field would say one thing twice on one
# screen.
for beside in fkrecipes-example-quench-ingredients fkrecipes-example-chain-ingredients; do
  if grep '^TRANSCRIPT extend' "$T" | grep -F "\"name\"=\"$beside\"" | grep -qF 'next name for it or is left out'; then
    fail "the text setting $beside carries the ladder line the dropdown beside it already carries"
  fi
done
# AND ON A PACKS TEXT BESIDE A COST DROPDOWN IT IS THERE, which is the other arm
# of that rule and the one a rule keyed on "is there a dropdown" got wrong: a
# cost dropdown renders no list of internal names and carries no ladder line, so
# this field is the only one of the pair where a player can read the rule at
# all. tips-packs is that shape in the example guest.
grep '^TRANSCRIPT extend' "$T" | grep -F '"name"="fkrecipes-example-tips-packs"' | grep -qF 'so the research can take fewer packs than shown' ||
  fail "a packs text setting beside a cost dropdown carries no ladder line, and the cost dropdown carries none either"
# ---------------------------------------------------------------------------
# THE SHARED DROPDOWN: which declaration describes it, and what falls out.
# ---------------------------------------------------------------------------
#
# The example's scaffolding line is one legacy dropdown named by TWO recipes and
# one technology. Without Describes the technology would win, because that walk
# runs second; the first recipe carries it, so what a player reads over that row
# is the bill they can paste into the field above it.
#
# ONE HELPER, SCOPED TO ONE SETTING'S OWN extend LINE. The stand-in's FINAL dump
# is one line holding every prototype at once, so a grep over the file would
# pass on a string landing anywhere at all.
setting_carries() {
  grep '^TRANSCRIPT extend' "$T" | grep -F "\"name\"=\"$1\"" | grep -qF "$2"
}
# (a) THE DESCRIBING RECIPE'S PRESETS, as to-type lines.
setting_carries steelworks-scaffold-tier '\n  to type: 2 iron-plate' ||
  fail "the shared dropdown does not carry the describing recipe's light preset"
setting_carries steelworks-scaffold-tier '\n  to type: 4 steel-plate' ||
  fail "the shared dropdown does not carry the describing recipe's heavy preset"
# AND NOT THE OTHER RECIPE'S, which is the negative that says one declaration
# describes rather than all of them: scaffold-tie's presets are rivets.
if setting_carries steelworks-scaffold-tier 'to type: 2 fkrecipes-example-steel-rivet'; then
  fail "the shared dropdown carries the presets of a recipe that does not describe it"
fi
# (b) THE INGREDIENT LADDER LINE, because what it shows is a list of internal
# names and the ladder is what the game does to one.
setting_carries steelworks-scaffold-tier 'so an option can craft a shorter list than it shows' ||
  fail "the shared dropdown carries no ingredient ladder line"
# (c) AND ITS SWITCH LINE NAMES THE DESCRIBING DECLARATION'S TEXT. The word is a
# DIRECTION, so the fixture puts the ingredient text ABOVE the dropdown and the
# pack text below it: "above" is the ingredient text and nothing else.
setting_carries steelworks-scaffold-tier 'The setting above applies instead while it does not say default.' ||
  fail "the shared dropdown's switch line does not name the describing declaration's text setting"
if setting_carries steelworks-scaffold-tier 'The setting below applies instead while it does not say default.'; then
  fail "the shared dropdown's switch line names the pack text, which belongs to the declaration that does not describe it"
fi
# (d) AND NO COST PRESET LINE IS ON IT, which is the same fact from the other
# side and is also what says CheckLocaleAdvisories has no key to name for this
# row: a cost preset is the only thing that composes a technology-name key.
for unwanted in ': cost of ' 'technology-name.' ': the fallback cost'; do
  if setting_carries steelworks-scaffold-tier "$unwanted"; then
    fail "the shared dropdown carries $unwanted, which belongs to the technology that does not describe it"
  fi
done
# (e) THE DESCRIBING RECIPE'S INGREDIENT TEXT CARRIES NO LADDER LINE, because
# the dropdown beside it now says the same thing in the same vocabulary.
if setting_carries steelworks-scaffold-parts 'next name for it or is left out'; then
  fail "the ingredient text beside the described dropdown carries a ladder line as well"
fi
# (f) AND THE PACK TEXT BESIDE THE SAME DROPDOWN KEEPS ITS OWN. That is the pair
# decision 10 is about: the sentence on the dropdown is about a list of
# ingredients it shows and says nothing about a research taking fewer packs.
setting_carries steelworks-scaffold-packs 'so the research can take fewer packs than shown' ||
  fail "the pack text beside the described dropdown lost its packs ladder line"
if setting_carries steelworks-scaffold-packs 'so what you craft can be shorter than shown'; then
  fail "the pack text beside the described dropdown carries the ingredient vocabulary of the ladder line"
fi

# ---------------------------------------------------------------------------
# A COST PRESET IN THE AUTHOR'S OWN WORDS, and a description the plan writes.
# ---------------------------------------------------------------------------
#
# The settings stage sees mods and never data.raw, so the composed tail names
# the ladder's FIRST rung. On the tips tier that rung is tungsten-hardening, an
# overhaul pack's technology in no game most players run, which is the case
# Display exists for.
setting_carries fkrecipes-example-tips-research-tier ': as much as the seventh projectile damage level, or the overhaul pack'"'"'s own hardening' ||
  fail "the overridden cost preset does not carry the author's own words"
if setting_carries fkrecipes-example-tips-research-tier 'technology-name.tungsten-hardening'; then
  fail "the overridden cost preset still composes the technology-name key it replaced"
fi
# AND THE CHOICE BESIDE IT IS UNTOUCHED, which is what says the override is per
# choice rather than per dropdown.
setting_carries fkrecipes-example-tips-research-tier 'technology-name.military-4' ||
  fail "the choice with no override lost its technology-name key"

# A DESCRIPTION THE PLAN WROTE stands where the consumer's own
# [mod-setting-description] key stood, and on a setting nothing is composed onto
# it is the whole description.
setting_carries fkrecipes-example-hardened-tools '"localised_description"="Adds the hardened steel line, its scaffolding and the research that unlocks them."' ||
  fail "the inline-described bool does not carry the plan's own description as its whole tooltip"
if setting_carries fkrecipes-example-hardened-tools 'mod-setting-description.fkrecipes-example-hardened-tools'; then
  fail "the inline-described bool still composes its own [mod-setting-description] key"
fi
# AND UNDER A COMPOSITION IT IS THE HEAD AND NOTHING ELSE: the library's own
# lines are where they were.
setting_carries steelworks-scaffold-parts 'What one scaffold bracket is made of while this is not on default.' ||
  fail "the inline-described ingredient text does not open with the plan's own description"
if setting_carries steelworks-scaffold-parts 'mod-setting-description.steelworks-scaffold-parts'; then
  fail "the inline-described ingredient text still composes its own [mod-setting-description] key"
fi
setting_carries steelworks-scaffold-parts 'Text this mod cannot use is set aside as though it said default' ||
  fail "the inline-described ingredient text lost a composed line under the plan's own description"

# AND THE LINE ABOUT A WRAPPED LIST IS GONE FROM EVERY COMPOSITION. It said the
# one thing about a rendered line a player cannot see, and it said it on every
# text setting and every ingredient dropdown of every consumer; what it
# disclosed is documented for AUTHORS in docs/usage.md instead.
if grep -qF 'A list too long for one line continues' "$T"; then
  fail "a composed description still carries the list-wrap line"
fi
# AND A COST DROPDOWN CARRIES NO LADDER LINE, because its preset is a localised
# label followed by a localised technology name: one vocabulary, prose
# throughout, and no rendered list of internal names for a ladder to shorten.
if grep '^TRANSCRIPT extend' "$T" | grep -F '"name"="fkrecipes-example-tips-research-tier"' | grep -qF 'next name for it or is left out'; then
  fail "a cost dropdown carries the ladder line, which is about a list it does not render"
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
grep -q '^LOG fkrecipes: ERROR: fkrecipes-example-rivet-ingredients, entry 2 ("2 iron-stik"): no item or fluid is named iron-stik\. The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart\. Changing a recipe empties an assembling machine'"'"'s input slots of anything the new list does not use\.$' "$T" ||
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
#
# AND IT IS TWO ELEMENTS RATHER THAN ONE, which is the whole of the ceiling fix
# in one line. This sentence is 243 bytes and a localised string element may be
# 200 (measured on 2.0.77: 201 refuses the load with "Localised string key is
# too large"), so the library chunks every composed literal at a word boundary
# into elements of at most 180 bytes. The engine concatenates them, so the
# player reads the same sentence; what moved is the shape, and the stand-in
# refuses the old one now, by the same rule and with the same sentence the
# engine uses.
#
# AND IT OPENS WITH THE AUTHOR'S OWN OPTIONAL KEY. This recipe declares a
# DisplayName and NO Description, which is the arm where the note used to stand
# in the [recipe-description] entry's place: a prototype's own
# localised_description field wins over the locale entry, so an author who wrote
# their description the ordinary Factorio way lost it for the whole of that
# load. The wrapper here resolves to that entry plus a newline where the author
# defined it and to NOTHING where they did not (measured on 2.0.77: a
# concatenation group holding an undefined key is itself a failed alternative,
# so the newline dies with it and no blank line is left behind).
grep '^TRANSCRIPT extend#' "$T" |
  grep -qF '"localised_description"={1="",2={1="?",2={1="",2={1="recipe-description.fkrecipes-example-steel-rivet"},3="\n"},3=""},3="The stored value of fkrecipes-example-rivet-ingredients could not be used, so the game loaded as though that setting had been left alone. The reason is in the log. Changing a ",4="recipe empties an assembling machine'"'"'s input slots of anything the new list does not use."},"localised_name"={1="",2="Steel rivets"},"name"="fkrecipes-example-steel-rivet"' ||
  fail "the recipe whose text was set aside carries no note in its own description"
# AND A RECIPE NOTHING FELL BACK ON CARRIES NONE, which is what says the note is
# a consequence of the fallback rather than something every prototype now has.
if grep '^TRANSCRIPT extend#' "$T" |
  grep -F '"name"="fkrecipes-example-salvaged-steel-rivet"' | grep -q "could not be used"; then
  fail "a recipe with no fallback carries a note"
fi
# And what it landed on: the mod's OWN declared list, because this field has no
# dropdown beside it, which is what "loaded as though that text had been left
# alone" means in the prototype rather than only in the line.
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
grep -qF '4=": cost of ",5={1="?",2={1="technology-name.military-4"},3="military-4"}' "$T" ||
  fail "the cost dropdown's composed preset line does not name the technology through its locale key"
if grep -qF '": cost of military-4"' "$T"; then
  fail "a cost preset line still carries the internal technology name"
fi
# A PARTIAL CUSTOM COST: the count is the player's and the time and the packs
# are the tier's, in one unit, and the clause says what the tier still supplies
# rather than pretending the whole cost was overridden. The cost override is per
# FIELD, which is why this clause and the ingredient one are two sentences.
grep -q '^LOG fkrecipes: fkrecipes-example-hardened-tips takes its research cost from fkrecipes-example-tips-packs: count 45, time 30, packs 1 automation-science-pack, 1 logistic-science-pack; the fkrecipes-example-tips-research-tier choice military supplies what the settings leave at default$' "$T" ||
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
# THE 2.1 ENGINE ARM. The same two packaged mods, run again against the
# stand-in's OTHER engine: no data.raw.tool at all, science packs as items
# carrying subgroup "science-pack", base reporting 2.1.17, and the research
# unit gated on lab coverage rather than on prototype type. All of that is
# measured on Factorio 2.1.17 build 87315; testdata/mirror/standin.lua's header
# quotes the probes.
#
# THERE IS NO SECOND GOLDEN, deliberately. A 2.1 transcript's RAW and FINAL
# lines dump a data.raw with a different SHAPE in it (no tool table, an item
# where a tool was), so a golden of them would pin the stand-in's own fixture
# rather than the library's behaviour, and it would have to be recaptured
# whenever that fixture moved. What is asserted instead is the thing the fix
# claims: THE GUEST EMITS THE SAME PROTOTYPES AND LOGS THE SAME LINES ON BOTH
# ENGINES. Those two comparisons are against the 2.0 run of the same build, so
# they need nothing on disk and they cannot go stale.
# ---------------------------------------------------------------------------
echo "== running both guests against the 2.1 engine arm"
for lang in go rust; do
  inner="$(find "$TMP/mods-$lang" -maxdepth 1 -mindepth 1 -type d | head -1)"
  [ -n "$inner" ] || refuse "$lang: the packaged mod vanished before the 2.1 arm"
  "$LUA52F" "$STANDIN" "$inner" 2.1 >"$TMP/transcript-$lang-21.txt" 2>&1 ||
    { cat "$TMP/transcript-$lang-21.txt" >&2
      refuse "$lang: the stages did not run against the 2.1 engine arm"; }
done

if ! diff -q "$TMP/transcript-go-21.txt" "$TMP/transcript-rust-21.txt" >/dev/null; then
  fail "the Go and Rust 2.1 transcripts differ; first differing line:"
  # Non-fatal for the reason the 2.0 comparison above gives.
  diff "$TMP/transcript-go-21.txt" "$TMP/transcript-rust-21.txt" | head -6 >&2 || true
fi

# THE BEHAVIOURAL ASSERTION. Every prototype the guest extended, and every line
# it logged, byte for byte against the 2.0 run of the same wasm. Before the
# engine key went in, the 2.1 arm answered no for every science pack: each
# priced research came out with an empty ingredient list and each carried a
# dropped-pack line the 2.0 run does not have, so this is the comparison that
# goes red for the defect.
# FROM THE `--- SETTINGS ---` MARKER ONWARD, because everything above it is the
# STAND-IN's own base data:extend, which is where the two engine arms differ on
# purpose: a tool with a durability on one and an item with a subgroup on the
# other. Below the marker every line is the guest's.
#
# AND WITH ONE FIELD NORMALISED, the ONLY one the guest is supposed to spell
# differently on the two engines: a recipe's category is `category` on 2.0 and
# `categories` (a list of one) on 2.1, because 2.1 refuses the old spelling
# outright. The 2.1 slice is rewritten back to the 2.0 spelling so the rest of
# the prototype is compared byte for byte, and the field itself is asserted on
# its own below, on both arms, so normalising it here does not stop it being
# checked.
guest_lines() {
  sed -n '/^--- SETTINGS ---/,$p' "$1" | grep "^$2" |
    sed 's/"categories"={1="\([^"]*\)"}/"category"="\1"/g' || true
}

for kind in 'TRANSCRIPT extend#' 'LOG '; do
  for lang in go rust; do
    guest_lines "$TMP/transcript-$lang.txt"    "$kind" >"$TMP/$lang-20-slice.txt"
    guest_lines "$TMP/transcript-$lang-21.txt" "$kind" >"$TMP/$lang-21-slice.txt"
    # NOT EMPTY, so "the two arms agree" cannot be a statement about nothing: a
    # marker that stopped matching would compare two empty files and pass.
    for run in 20 21; do
      [ -s "$TMP/$lang-$run-slice.txt" ] ||
        fail "$lang: the 2.$((run - 20)) arm has no '$kind' line below the marker, so the arms were compared over nothing"
    done
    if ! diff -q "$TMP/$lang-20-slice.txt" "$TMP/$lang-21-slice.txt" >/dev/null; then
      fail "$lang: the '$kind' lines differ between the 2.0 and 2.1 engine arms; first difference:"
      diff "$TMP/$lang-20-slice.txt" "$TMP/$lang-21-slice.txt" | head -6 >&2 || true
    fi
  done
done

# ANTI-VACUITY FOR THE ARM ITSELF. A 2.1 run that silently fell back to the 2.0
# fixture would pass every comparison above by being the same run twice, so the
# thing that MUST differ is checked too: the 2.0 arm has a tool table and the
# 2.1 arm has none.
grep -q '^RAW tool: ' "$TMP/transcript-go.txt" ||
  fail "the 2.0 arm has no data.raw.tool, so the engine arms are not two engines"
if grep -q '^RAW tool: ' "$TMP/transcript-go-21.txt"; then
  fail "the 2.1 arm still has a data.raw.tool, so it is not modelling 2.1.17"
fi
grep -q '^RAW item: .*automation-science-pack' "$TMP/transcript-go-21.txt" ||
  fail "the 2.1 arm has no automation-science-pack among its items"

# AND THE ONE FIELD THE NORMALISER ABOVE HIDES, asserted on each arm in the
# spelling that arm's engine demands. The quench recipe is the one prototype the
# example gives a category at all.
#
# MATCHED INSIDE THAT RECIPE'S OWN EXTEND LINE and not anywhere in the file: a
# bare grep for the category would pass if the field had migrated to some other
# prototype, which is a failure this assertion exists to catch. Written as `if`
# rather than as an AND-list for the reason package_and_run gives above.
quench_field() {
  grep '^TRANSCRIPT extend' "$1" |
    grep -F '"name"="fkrecipes-example-hardened-steel-plate-quenching"' |
    grep -qF "$2"
}

for lang in go rust; do
  if ! quench_field "$TMP/transcript-$lang.txt" '"category"="crafting-with-fluid"'; then
    fail "$lang: the 2.0 arm did not emit the quench recipe's category in the 2.0 spelling"
  fi
  if ! quench_field "$TMP/transcript-$lang-21.txt" '"categories"={1="crafting-with-fluid"}'; then
    fail "$lang: the 2.1 arm did not emit the quench recipe's category as a categories list"
  fi
  if quench_field "$TMP/transcript-$lang-21.txt" '"category"="crafting-with-fluid"'; then
    fail "$lang: the 2.1 arm still emits the 2.0 category spelling, which 2.1 refuses outright"
  fi
done

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
