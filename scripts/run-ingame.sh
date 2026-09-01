#!/usr/bin/env bash
# THE IN-GAME GATE: both example guests, in a real Factorio, hashed.
#
# This is the strongest gate this library has and it is the only one that is
# the engine. The host tests compare what each planner BELIEVES it will emit;
# scripts/run-mirror.sh compares what a packaged mod does to a stand-in written
# from measurements. Neither is Factorio: the prototype loader, data:extend's
# own validation, the settings-to-data round trip and every base prototype the
# plan reads are all outside what a stand-in can speak to. A stand-in that is
# wrong about the world is exactly what this gate exists to catch.
#
# --dump-data is what makes it cheap. It runs the settings and data stages,
# writes script-output/data-raw-dump.json and mod-settings-dump.json, and
# STOPS: it never reaches control.lua, so this is a pure data-stage instrument.
#
# BOTH DUMPS ARE HASHED, AND THE SECOND ONE IS NOT DECORATION. Setting
# prototypes never reach the data dump, so a golden over data-raw-dump.json
# alone stays green for a guest whose fk_settings did nothing at all. That is
# FkLua's recorded lesson and it applies here twice over, because this library
# generates settings AND reads them back at the data stage.
#
# ASSERTIONS, IN ORDER OF STRENGTH:
#   1. The normalised hashes match the committed golden. The real one.
#   2. The Go and Rust guests produce the SAME hashes. Two hand-written
#      libraries drift, and this is the in-game half of that mirror.
#   3. Two runs of one guest agree. A tripwire: the day it stops being true the
#      data stage has become nondeterministic and every mod built on this
#      library is a join refusal waiting to happen.
#   4. Cause-naming assertions a hash cannot make. A hash says "different"; jq
#      over the dump says WHICH decision moved.
#
# FLAGS:
#   --update   capture the golden line for this engine, deliberately.
#   --strict   make an environmental SKIP a FAILURE (exit 1), and refuse to
#              RECORD a golden whose mod set differs from the one the golden
#              already carries. Also set by FKRECIPES_STRICT=1, which is the
#              form CI wants and the reason the capture needs its own guard:
#              a job that sets it once covers every run in the job, --update
#              included.
#
# WHY --strict IS OPT-IN RATHER THAN THE DEFAULT. A mod-set mismatch means the
# machine owns different DLC from the machine that captured the golden, so the
# hashes describe two different worlds and neither one is wrong. Failing a
# developer's run for that would be a gate crying wolf, and FkLua's convention
# is to skip. CI is the other case: there the mod set is fixed by the image, so
# a skip means the gate silently stopped running, and a gate that can quietly
# stop running is worse than one that is occasionally inconvenient.
#
# --strict AND --update TOGETHER MEAN "re-record, but not from a world the
# golden does not describe". Plain --update still re-records any mod set,
# because re-recording after a deliberate environment change is exactly what
# it is for; strict only removes the case where nobody meant to change worlds.
#
# THE PROTOTYPE LIST CHECKSUM IS NOT USED AS A GATE. It is order-insensitive,
# which sounds right, and it is blind to field values (FkLua measured it
# unchanged when a stack_size went 1 -> 42), so quoting it as an equivalence
# proof would be a gate that cannot fail on this library's likeliest defect.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FKLUA_CHECKOUT="${FKLUA_CHECKOUT:-$ROOT/../FkLua}"
FACTORIO="${FACTORIO_BIN:-$HOME/Library/Application Support/Steam/steamapps/common/Factorio/factorio.app/Contents/MacOS/factorio}"

# A PRIVATE WRITE-DATA DIRECTORY. Factorio LOCKS its user directory, so a
# second process pointed at a real install dies at startup while the player's
# game is open, which reads as a broken gate rather than as two copies of the
# game. It is also where --dump-data writes, so a private one keeps this gate
# out of anybody's install.
#
# ONE INSTANCE PER USERDIR, and that is the engine's rule rather than this
# script's: two gates sharing this directory would collide on Factorio's own
# lock. A second concurrent run must set FACTORIO_USERDIR to a directory of its
# own.
USERDIR="${FACTORIO_USERDIR:-/tmp/fkrecipes}"

TMP="$ROOT/tmp/ingame"
GOLDEN="$ROOT/testdata/ingame/dump-sha256.txt"
MODNAME=fkrecipes-example
MODVER=0.1.0

UPDATE=0
# Defaults to the environment so CI can set it once for the whole job; the flag
# is the interactive form of the same switch.
STRICT=0
if [ "${FKRECIPES_STRICT:-0}" = "1" ]; then STRICT=1; fi
for arg in "$@"; do
  case "$arg" in
    --update) UPDATE=1 ;;
    --strict) STRICT=1 ;;
    *) echo "run-ingame: unknown argument $arg; --update and --strict are the only ones" >&2; exit 1 ;;
  esac
done

FAIL=0
SKIPPED=0
fail() { echo "  FAIL: $*" >&2; FAIL=1; }
refuse() { echo "run-ingame: $*" >&2; exit 1; }

[ -x "$FACTORIO" ] || refuse "no Factorio at: $FACTORIO
  set FACTORIO_BIN to the binary inside factorio.app/Contents/MacOS"
[ -d "$FKLUA_CHECKOUT" ] || refuse "no FkLua checkout at $FKLUA_CHECKOUT; set FKLUA_CHECKOUT"
command -v tinygo >/dev/null || refuse "tinygo is not on PATH; the Go guest cannot be built"
command -v cargo  >/dev/null || refuse "cargo is not on PATH; the Rust guest cannot be built"
command -v jq     >/dev/null || refuse "jq is not on PATH; the dumps cannot be normalised or read"

# ONE SOURCE for the version derivation, sourced after FACTORIO is set because
# every function in it reads that variable. head -1 inside it is load-bearing:
# `factorio --version` prints several Version: lines and only the first is the
# build.
LIBENGINE="$FKLUA_CHECKOUT/scripts/lib-engine.sh"
[ -f "$LIBENGINE" ] || refuse "no lib-engine.sh at $LIBENGINE
  FKLUA_CHECKOUT points at $FKLUA_CHECKOUT, which is not an FkLua checkout.
  Set FKLUA_CHECKOUT to one, or clone it beside this repository."
# shellcheck source=/dev/null
. "$LIBENGINE"

# RE-ASKED EVERY RUN. A recorded engine version is a claim about a machine, and
# the golden is keyed by this line: trusting a stale one is how a gate reports
# a pass for an engine nobody ran.
ENGINE="$(factorio_version_triple)"
SERIES="$(factorio_series)"

echo "=== the data stage in a real Factorio ==="
echo "engine:     $ENGINE (info.json will declare $SERIES)"
echo "normaliser: jq $(jq --version)"
echo "userdir:    $USERDIR"
echo

# Only this gate's subtree is wiped: run-mirror.sh owns $ROOT/tmp and the two
# must not clobber each other's builds.
rm -rf "$TMP"
mkdir -p "$TMP/bin"
FKLUA="$TMP/bin/fklua"

mkdir -p "$USERDIR/config"
CFG="$USERDIR/config/config.ini"
if [ ! -f "$CFG" ]; then
  DEFAULT_CFG="$HOME/Library/Application Support/factorio/config/config.ini"
  if [ -f "$DEFAULT_CFG" ]; then
    sed "s|^write-data=.*|write-data=$USERDIR|" "$DEFAULT_CFG" > "$CFG"
  else
    cat > "$CFG" <<EOF
[path]
read-data=__PATH__system-read-data__
write-data=$USERDIR

[general]
locale=auto
EOF
  fi
fi

# fklua embeds its runtime shim, so a stale binary packages a shim nobody
# ships. Rebuilt every run, into this repo, never into the checkout.
echo "== building fklua from $FKLUA_CHECKOUT"
( cd "$FKLUA_CHECKOUT" && go build -o "$FKLUA" ./cmd/fklua ) >"$TMP/fklua-build.log" 2>&1 ||
  { cat "$TMP/fklua-build.log" >&2; refuse "fklua did not build; if this persists, rm -rf $TMP"; }

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

DUMP="$USERDIR/script-output/data-raw-dump.json"
SDUMP="$USERDIR/script-output/mod-settings-dump.json"

# dump_once LANG RUN -- one packaged mod, one engine run, two hashes written to
# $TMP/hash-LANG-RUN.
#
# NOT A COMMAND SUBSTITUTION, and NO PIPES ANYWHERE, and both are the same
# lesson. `jq -S . < dump | shasum` reports only shasum's status, so a
# malformed or truncated dump produces the sha256 of EMPTY INPUT: a constant.
# That constant is identical for both languages and for both runs, so it
# satisfies the mirror check and the determinism check at once, and --update
# would happily record it as the golden. The normalise step therefore writes a
# file and is checked on its own, and the result lands in a file rather than in
# a substitution so a refusal here exits the SCRIPT rather than a subshell.
dump_once() {
  local lang="$1"
  local run="$2"
  local moddir="$TMP/mods-$lang"
  local ndata="$TMP/normalised-data-$lang-$run.json"
  local nsettings="$TMP/normalised-settings-$lang-$run.json"
  local dhash shash

  rm -rf "$moddir"
  "$FKLUA" mod --data-module "$TMP/datastage-$lang.wasm" \
    --name "$MODNAME" --version "$MODVER" --author Techrocket9 \
    --factorio-version "$SERIES" \
    -o "$moddir" >"$TMP/pack-$lang.log" 2>&1 ||
    { cat "$TMP/pack-$lang.log" >&2; refuse "$lang: packaging failed"; }

  rm -f "$DUMP" "$SDUMP"
  "$FACTORIO" -c "$CFG" --mod-directory "$moddir" --dump-data \
    >"$TMP/dump-$lang-$run.log" 2>&1 ||
    { cat "$TMP/dump-$lang-$run.log" >&2
      refuse "$lang: the engine run failed (its log is above).
  If the complaint is about the configuration or the write-data directory, the
  seeding at the top of this script recreates both: rm -rf $USERDIR (or just
  $CFG) and rerun."; }

  [ -f "$DUMP" ]  || refuse "$lang: the engine wrote no data dump at $DUMP"
  [ -f "$SDUMP" ] || refuse "$lang: the engine wrote no settings dump at $SDUMP"

  jq -S . "$DUMP" > "$ndata" ||
    refuse "$lang: the data dump is not readable JSON: $DUMP"
  jq -S . "$SDUMP" > "$nsettings" ||
    refuse "$lang: the settings dump is not readable JSON: $SDUMP"

  dhash="$(shasum -a 256 "$ndata")" || refuse "$lang: could not hash $ndata"
  shash="$(shasum -a 256 "$nsettings")" || refuse "$lang: could not hash $nsettings"
  printf '%s %s\n' "${dhash%% *}" "${shash%% *}" > "$TMP/hash-$lang-$run"
}

# mod_set LOG -- the data-stage mod set, as one comparable string.
#
# THE DUMP IS A FUNCTION OF EVERY MOD THAT RAN, not only of this one. Factorio's
# bundled DLC data loads whatever --mod-directory says, so a machine owning
# different DLC produces a different dump for a mod that is perfectly fine. It
# is recorded beside the hash so a mismatch can name its own cause instead of
# saying only "does not match".
mod_set() {
  sed -n 's/.*Loading mod \([^ ]*\) \([^ ]*\) (data\.lua).*/\1@\2/p' "$1" |
    sort | tr '\n' ' ' | sed 's/ $//'
}

FIRSTLANG=""
FIRSTHASH=""
MODSET=""
for lang in go rust; do
  echo "--- $lang ---"
  started=$(date +%s)
  dump_once "$lang" 1
  dump_once "$lang" 2
  h1="$(cat "$TMP/hash-$lang-1")"
  h2="$(cat "$TMP/hash-$lang-2")"
  elapsed=$(( $(date +%s) - started ))

  echo "  sha256 (data settings) $h1"
  echo "  two engine runs in ${elapsed}s"

  if [ "$h1" != "$h2" ]; then
    fail "$lang: two runs of the same guest produced different dumps"
    echo "        $h1" >&2
    echo "        $h2" >&2
  else
    echo "  ok: two runs agree (the data stage is deterministic)"
  fi

  if [ -z "$FIRSTLANG" ]; then
    FIRSTLANG="$lang"
    FIRSTHASH="$h1"
    MODSET="$(mod_set "$TMP/dump-$lang-2.log")"
  elif [ "$h1" != "$FIRSTHASH" ]; then
    fail "$lang and $FIRSTLANG disagree about what the data stage produced"
    echo "        $FIRSTLANG $FIRSTHASH" >&2
    echo "        $lang $h1" >&2
  else
    echo "  ok: $lang and $FIRSTLANG agree (the in-game mirror holds)"
  fi
done

# ---------------------------------------------------------------------------
# ANTI-VACUITY AND CAUSE-NAMING. A hash says "different"; these say WHICH
# decision moved, and they fail on a dump that ran but proved nothing.
# ---------------------------------------------------------------------------
echo "== checking the dump says something"
LOG="$TMP/dump-rust-2.log"

# The library's own log line, through fkdata.Log, in a real base game where
# neither candidate for the optional hardener exists either.
grep -q "fkrecipes: hardened-steel-plate-quenching: none of tungsten-carbide, titanium-plate is present, so the ingredient is dropped" "$LOG" ||
  fail "the ingredient ladder logged nothing in the engine's own log"

jqassert() {
  local what="$1" file="$2" filter="$3"
  local got
  got="$(jq -r "$filter" "$file" 2>&1)" || { fail "$what: the query failed: $got"; return; }
  [ "$got" = "true" ] || fail "$what (the dump says $got)"
}

jqassert "the prerequisite splice reached logistics-2" "$DUMP" \
  '(.technology["logistics-2"].prerequisites // []) | index("fkrecipes-example-hardened-steel") != null'
jqassert "the bound crafting time reached the quenching recipe" "$DUMP" \
  '.recipe["fkrecipes-example-hardened-steel-plate-quenching"].energy_required == 3'
jqassert "the switched-on technology is enabled with no hidden field" "$DUMP" \
  '.technology["fkrecipes-example-hardened-tips"] | (.enabled == true) and (has("hidden") | not)'
# NO PLAYER HAS TOUCHED A SETTING HERE, so every EnabledBy reads its DECLARED
# DEFAULT: hardened-tools is true, and the technology comes out enabled with no
# hidden key. The switched-OFF branch belongs to scripts/run-mirror.sh, whose
# stand-in sets the setting false; between them the two gates cover both sides
# of the hidden-not-absent decision, and neither could cover both alone.
jqassert "the setting default reached the technology it gates" "$DUMP" \
  '.technology["fkrecipes-example-hardened-steel"] | (.enabled == true) and (has("hidden") | not)'
jqassert "research still gates the recipes it unlocks" "$DUMP" \
  '.recipe["fkrecipes-example-hardened-steel-plate-quenching"].enabled == false and .recipe["fkrecipes-example-steel-rivet"].enabled == false'
jqassert "the recipe nothing unlocks is enabled from the start" "$DUMP" \
  '.recipe["fkrecipes-example-salvaged-steel-rivet"].enabled == true'

# THE STAND-IN GUESSED AT THE WORLD AND THIS IS WHERE THE GUESS IS CHECKED. The
# mirror models physical-projectile-damage-7 as formula-priced and infinite; if
# base ever stops carrying it, or carries it differently, the copy is what
# moves. Compared against the SOURCE prototype in the same dump rather than
# against a number written here, so this asserts "the copy is faithful" rather
# than "the copy is what I typed".
#
# NO PLAYER HAS TOUCHED A SETTING HERE either, so tips-research-tier reads its
# declared default and the ladder walked is the PROJECTILE one. The mirror's
# stand-in sets that setting to military and walks the other; between them the
# two gates cover both ladders, and neither could cover both alone.
jqassert "the cost ladder copied the real formula out of base" "$DUMP" \
  '.technology["fkrecipes-example-hardened-tips"].unit == .technology["physical-projectile-damage-7"].unit'
jqassert "the cost ladder carried the real level cap out of base" "$DUMP" \
  '.technology["fkrecipes-example-hardened-tips"].max_level == .technology["physical-projectile-damage-7"].max_level'
# THE PREREQUISITE MOVES WITH THE UNIT, and the first rung of that ladder names
# a technology no vanilla install has, so this also says the ladder stepped
# past what is not there rather than stopping at it.
jqassert "the prerequisite moved with the copied unit" "$DUMP" \
  '.technology["fkrecipes-example-hardened-tips"].prerequisites == ["physical-projectile-damage-7"]'
jqassert "the ladder stepped past the technology no install has" "$DUMP" \
  '.technology["tungsten-hardening"] == null'
jqassert "all seven generated settings reached the settings dump" "$SDUMP" \
  '[paths(scalars) | select(length > 1) | .[1]] | map(select(startswith("fkrecipes-example-"))) | unique | length == 7'
# The one-sided NumericSpec arms, in the engine's own settings dump rather than
# only in the stand-in's: a declared maximum with no minimum of its own, and a
# declared minimum with no maximum. The second is craft-time-bound, so a
# maximum appearing on it would mean the generated floor-safe bound had
# replaced the consumer's declaration rather than filling a gap.
jqassert "the min-only setting kept its declared minimum and gained no maximum" "$SDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-tempering-hold")] | length > 0 and all(.minimum_value == 0.5 and (has("maximum_value") | not))'
jqassert "the generated craft-time minimum reached the settings dump" "$SDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-forging-time") | .minimum_value] | any(. == 0.002)'

# ---------------------------------------------------------------------------
# The golden.
# ---------------------------------------------------------------------------
LINE="$ENGINE $FIRSTHASH $MODSET"
mkdir -p "$(dirname "$GOLDEN")"

if [ "$UPDATE" = 1 ]; then
  # THE CAPTURE NEEDS ITS OWN STRICT GUARD, because SKIPPED is set in the
  # comparison branch below and is structurally 0 here: --strict on its own
  # would guard the comparison and wave the capture straight through, which is
  # the worse of the two. A wrong comparison reports; a wrong capture WRITES
  # the wrong world into the file every later run is measured against, and the
  # header above invites a CI job to set FKRECIPES_STRICT=1 once for
  # everything it runs.
  #
  # Only against an EXISTING line for this engine: a first capture has nothing
  # to disagree with, so strict has nothing to say about it.
  strict_mods=""
  if [ "$STRICT" != 0 ] && [ -f "$GOLDEN" ]; then
    strict_mods="$(grep "^$ENGINE " "$GOLDEN" | cut -d' ' -f4- || true)"
  fi
  if [ -n "$strict_mods" ] && [ "$strict_mods" != "$MODSET" ]; then
    echo "  refusing to record a golden from a mod set the golden does not carry; drop --strict to re-record after an environment change" >&2
    echo "    golden: $strict_mods" >&2
    echo "    here:   $MODSET" >&2
    FAIL=1
  elif [ "$FAIL" != 0 ]; then
    echo "  refusing to record a golden from a failing run" >&2
  else
    if [ -f "$GOLDEN" ]; then
      grep -v "^$ENGINE " "$GOLDEN" > "$GOLDEN.tmp" || true
      mv "$GOLDEN.tmp" "$GOLDEN"
    else
      cat > "$GOLDEN" <<'EOF'
# The in-game dump hashes, one line per engine.
#
#   <engine> <data-sha256> <settings-sha256> <mod set, name@version, sorted>
#
# ENGINE, because a dump is a function of the engine that produced it: a new
# Factorio moves base prototypes and every hash with them, and a line keyed by
# version lets one file hold several without either one being wrong.
#
# TWO HASHES, because setting prototypes never reach the data dump: a golden
# over the data dump alone stays green for a guest whose settings stage did
# nothing at all.
#
# THE MOD SET, because the dump is a function of every mod that ran. Factorio's
# bundled DLC data loads whatever --mod-directory says, so a machine owning
# different DLC produces a different dump for a mod that is perfectly fine.
# A mismatch here is ENVIRONMENTAL and reports SKIPPED, not FAILED.
#
# Capture a line with: scripts/run-ingame.sh --update
EOF
    fi
    printf '%s\n' "$LINE" >> "$GOLDEN"
    echo "== golden recorded: $LINE"
  fi
elif [ ! -f "$GOLDEN" ]; then
  fail "no golden at $GOLDEN; capture it deliberately with: scripts/run-ingame.sh --update"
else
  WANT="$(grep "^$ENGINE " "$GOLDEN" || true)"
  if [ -z "$WANT" ]; then
    fail "the golden has no line for Factorio $ENGINE; capture it with: scripts/run-ingame.sh --update"
  else
    want_mods="$(printf '%s' "$WANT" | cut -d' ' -f4-)"
    want_hashes="$(printf '%s' "$WANT" | cut -d' ' -f2,3)"
    if [ "$want_mods" != "$MODSET" ]; then
      # NOT A FAILURE OF THE MOD. Saying so is the whole reason the mod set is
      # in the file: a different DLC set is a different world, and a hash taken
      # in one cannot speak about the other.
      echo "  SKIPPED: the mod set differs from the golden's, so the hashes are not comparable" >&2
      echo "    golden: $want_mods" >&2
      echo "    here:   $MODSET" >&2
      SKIPPED=1
    elif [ "$want_hashes" != "$FIRSTHASH" ]; then
      fail "the dumps do not match the golden for Factorio $ENGINE"
      echo "    golden: $want_hashes" >&2
      echo "    here:   $FIRSTHASH" >&2
      echo "    the dumps are at $DUMP and $SDUMP" >&2
    else
      echo "  ok: the dumps match the golden for Factorio $ENGINE"
    fi
  fi
fi

if [ "$FAIL" != 0 ]; then
  echo "run-ingame: FAILED" >&2
  exit 1
fi
if [ "$SKIPPED" != 0 ]; then
  # BOTH SETS ARE ALREADY PRINTED ABOVE, in the same words either way: the
  # report is what a reader acts on, and only the verdict changes here.
  if [ "$STRICT" != 0 ]; then
    echo "run-ingame: FAILED (--strict: an environmental skip is a failure, because a gate that skips is a gate that stopped running)" >&2
    exit 1
  fi
  echo "run-ingame: SKIPPED (environmental: the mod set is not the golden's; --strict makes this a failure)"
  exit 0
fi
echo "run-ingame: OK"
