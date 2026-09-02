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
# The ladder, both halves, inside the ingredient plan the OIL medium selects:
# a rung fell back to a present candidate, and a rung with nothing present was
# dropped with a line through fkdata.Log.
grep -q '"name"="steel-plate"' "$T" || fail "the ingredient ladder did not fall back to steel-plate"
grep -q "^LOG fkrecipes: hardened-steel-plate-quenching: none of light-oil-barrel, crude-oil-barrel is present, so the ingredient is dropped$" "$T" ||
  fail "the dropped ladder logged nothing"
if grep -q '"tungsten-plate"' "$T"; then fail "an absent ingredient reached a prototype"; fi
# The chosen medium is the one that reached the prototype, and the plan the
# player did NOT pick left nothing behind. Matched on the WHOLE ingredient
# list rather than on one amount: the recipe name is serialised after the
# ingredients, and "amount"=4 on its own also appears in an unrelated recipe's
# results. The water plan asks for four rivets, the oil plan for two.
oil_plan='"ingredients"={1={"amount"=2,"name"="steel-plate","type"="item"},2={"amount"=2,"name"="fkrecipes-example-steel-rivet","type"="item"}}'
water_plan='"ingredients"={1={"amount"=2,"name"="steel-plate","type"="item"},2={"amount"=4,"name"="fkrecipes-example-steel-rivet","type"="item"}}'
grep -qF "$oil_plan" "$T" || fail "the chosen ingredient plan did not reach the recipe"
if grep -qF "$water_plan" "$T"; then
  fail "the ingredient plan nobody chose reached a prototype"
fi
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
# The cost ladder. These name the GENERATED prototype rather than any unit
# field on its own: the stand-in's own rows carry those fields too, so a grep
# for the field alone would pass whether or not anything was copied. The level
# cap and the count_formula are the in-game gate's subject, where the declared
# default walks the other ladder and copies them out of the real base.
grep -q '^TRANSCRIPT extend#[0-9]*.*"name"="fkrecipes-example-hardened-tips".*"count"=250' "$T" ||
  fail "the ladder did not copy the chosen source unit"
grep -q '^TRANSCRIPT extend#[0-9]*.*"name"="fkrecipes-example-hardened-tips".*"prerequisites"={1="military-4"}' "$T" ||
  fail "the prerequisite did not move with the copied unit"
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
