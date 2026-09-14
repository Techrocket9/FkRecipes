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
# TWO ROWS PER ENGINE, AND THE SECOND ONE IS THE CUSTOMIZER. The DEFAULT row is
# every setting left where the mod declared it, which is the load a player who
# never opens the settings screen gets. The FLIPPED row is that same mod with
# testdata/ingame/flipped.json written into the mod directory as
# mod-settings.dat by `fklua modsettings write`: a dropdown on custom with a
# player-typed ingredient list carrying a FLUID, a research priced out of three
# settings and placed by its own ladder with a TYPO in its pack text, a whole
# ingredient list typed into a setting with no dropdown in front of it, and a
# TYPO in an ingredient text behind a preset. Nothing but the engine can say
# those work, because the settings stage reads a stored value and no stand-in
# has one.
#
# THE TWO TYPOS ARE THE ROW'S SECOND SUBJECT, ONE PER CHANNEL. An input the
# PLAYER controls never refuses the load: the run must exit 0, the research must
# come out priced on the packs the mod declared, the quench recipe must come out
# of the preset its dropdown names, and the log must carry exactly two ERROR
# lines, one naming each setting. Before that decision this row exited 1 with
# "Failed to load mod" and wrote no dump, and the file that caused it was one
# nothing in the game could then edit, so a valid edit and a refused one in ONE
# row is what says both halves of the rule hold on a real engine.
#
# AND THE RECIPE TYPO IS THE CHANNEL THAT COULD NOT BE WALKED AT ALL until the
# library chunked its composed notes. A localised string element may be 200
# BYTES on a data-stage prototype and the recipe note is 245 with its newline,
# so this row exited 1 with "Localised string key is too large: 245 > 200
# (limit)." and no dump. Both channels are walked here now, and the recipe's own
# four description elements are pinned below.
#
# THE DEFAULT ROW RUNS TWICE AND THE FLIPPED ROW ONCE. Determinism is a
# property of the data stage and not of a particular settings file, so the
# second default run is what proves it and a second flipped run would buy
# nothing but twenty seconds.
#
# ASSERTIONS, IN ORDER OF STRENGTH:
#   1. The normalised hashes match the committed golden, for BOTH rows. The
#      real one.
#   2. The Go and Rust guests produce the SAME hashes, for both rows. Two
#      hand-written libraries drift, and this is the in-game half of that
#      mirror.
#   3. Two runs of one guest agree. A tripwire: the day it stops being true the
#      data stage has become nondeterministic and every mod built on this
#      library is a join refusal waiting to happen.
#   4. The flipped row DIFFERS from the default one. A settings file the engine
#      ignored, or one this script failed to install, would otherwise produce a
#      second row identical to the first and every check above would pass.
#   5. Cause-naming assertions a hash cannot make. A hash says "different"; jq
#      over the dump says WHICH decision moved.
#   6. The settings file this run writes is byte for byte
#      testdata/ingame/flipped.golden.dat. The writer belongs to the toolchain
#      now, so what this repository can still say about it is exactly that: for
#      THIS JSON it produces THESE bytes. Without the pin an upstream layout
#      change would arrive as a moved dump hash, which names the engine run and
#      not the file that fed it.
#   7. The mod-settings.dat the ENGINE REWROTE carries the values the flipped
#      file installed. The engine resets a number out of its range and a
#      dropdown value off its list to the default (measured, in FkLua), so a
#      row it discarded that way is still sitting in the file this script
#      wrote: only the file the engine wrote says what actually ran.
#
# FLAGS:
#   --update   capture the golden lines for this engine, deliberately, and
#              re-record testdata/ingame/flipped.golden.dat from what the
#              writer produces now. That file is a function of flipped.json and
#              of the writer and of nothing the engine does, so it is recorded
#              before the first run rather than after the last.
#   --strict   make an environmental SKIP a FAILURE (exit 1), and refuse to
#              RECORD the hash rows when their mod set differs from the one the
#              golden already carries. It says nothing about
#              flipped.golden.dat: those bytes are a function of flipped.json
#              and of the writer alone, so they are not keyed by a mod set and
#              strict has nothing there to guard. Also set by
#              FKRECIPES_STRICT=1, which is the form CI wants and the reason
#              the capture needs its own guard: a job that sets it once covers
#              every run in the job, --update included.
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
FLIPPED_JSON="$ROOT/testdata/ingame/flipped.json"
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

# AND ONE COPY of the jump-row reader, shared with scripts/run-mirror.sh. It
# needs refuse() and FKLUA_CHECKOUT, both set above.
LIBREPORT="$ROOT/scripts/lib-report.sh"
[ -f "$LIBREPORT" ] || refuse "no lib-report.sh at $LIBREPORT; it ships beside this script"
# shellcheck source=/dev/null
. "$LIBREPORT"

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

# THE FLIPPED ROW'S SETTINGS FILE, written by the fklua this script just built.
# There is no CLI for a mod-settings.dat outside that one: the engine writes it
# and reads it back, and a headless gate that wants a player's typed text in
# front of the settings stage has to write those bytes itself. This repository
# carried an encoder of its own until the toolchain grew one; keeping it would
# have meant a second thing to hold in step with the format and a second answer
# on the day the two disagreed, so the gate uses the writer it already builds.
# `write` decodes what it encoded before writing, so a JSON edited into
# something that does not round trip is a sentence naming the byte here rather
# than a Factorio that refuses to start twenty seconds later.
FLIPPED_DAT="$TMP/mod-settings.dat"
FLIPPED_GOLDEN_DAT="$ROOT/testdata/ingame/flipped.golden.dat"
echo "== writing the flipped row's mod-settings.dat"
[ -f "$FLIPPED_JSON" ] || refuse "no flipped settings at $FLIPPED_JSON"

# AN fklua WITHOUT THE SUBCOMMAND IS AN ENVIRONMENT, NOT A FAILING WRITE, and
# the two are told apart BEFORE the write runs, because otherwise the remedy
# arrives buried in a page of usage text. fklua prints `unknown command
# "modsettings"` and exits 2 when its dispatch has no such case, and that
# string is the one thing that means the checkout is older than the subcommand.
# Every other way the write can fail is reported below, as the write's own
# failure, with its log.
"$FKLUA" modsettings >"$TMP/modsettings-probe.log" 2>&1 || true
if grep -q 'unknown command "modsettings"' "$TMP/modsettings-probe.log"; then
  refuse "NOT RUN: FKLUA_CHECKOUT points at
    $FKLUA_CHECKOUT
  and the fklua built from it has no modsettings subcommand, so the flipped
  row's mod-settings.dat cannot be written and this gate would cover only the
  settings the mod itself declares.
  Point FKLUA_CHECKOUT at an FkLua checkout at 2a541a7 or later: the
  modsettings subcommand arrived at c21ff07, and the jumps row this gate reads
  next arrived one commit after it."
fi

"$FKLUA" modsettings write --from "$FLIPPED_JSON" --out "$FLIPPED_DAT" \
  >"$TMP/modsettings-write.log" 2>&1 ||
  { cat "$TMP/modsettings-write.log" >&2; refuse "the flipped settings file did not encode"; }
cat "$TMP/modsettings-write.log"

# AND THE BYTES ARE PINNED, because the writer belongs to somebody else now.
# What this repository can still say about it is exactly this: for THIS JSON it
# produces THESE bytes. Without the pin an upstream layout change would arrive
# as a moved dump hash, which names the engine run and not the file that fed it.
#
# THE CHECK IS BEHIND THE FLAG, because --update is the remedy this refusal
# names: run unconditionally it would refuse the very command it tells the
# reader to run, and a golden that went missing could never be recorded again.
# The capture below needs no file to be there: `cmp -s` against a missing
# operand exits non-zero without a word, so the else arm records it.
if [ "$UPDATE" != 1 ] && [ ! -f "$FLIPPED_GOLDEN_DAT" ]; then
  refuse "no byte golden at $FLIPPED_GOLDEN_DAT
  Record it deliberately with: scripts/run-ingame.sh --update"
fi
if [ "$UPDATE" = 1 ]; then
  # RECORDED HERE rather than beside the hash rows at the end, because this
  # file is a function of flipped.json and of the writer and of nothing the
  # engine does. A run that later fails on a hash has still written the right
  # bytes, and holding the capture back would make re-recording them wait on a
  # green engine they have nothing to do with.
  if cmp -s "$FLIPPED_DAT" "$FLIPPED_GOLDEN_DAT"; then
    echo "== byte golden unchanged: $FLIPPED_GOLDEN_DAT"
  else
    cp "$FLIPPED_DAT" "$FLIPPED_GOLDEN_DAT" ||
      refuse "could not record the byte golden at $FLIPPED_GOLDEN_DAT"
    echo "== byte golden recorded: $FLIPPED_GOLDEN_DAT ($(wc -c <"$FLIPPED_GOLDEN_DAT" | tr -d ' ') bytes)"
  fi
elif ! cmp -s "$FLIPPED_DAT" "$FLIPPED_GOLDEN_DAT"; then
  # A REFUSAL RATHER THAN A fail(), and that is the split this script already
  # draws: fail() is for an assertion about the library, and this is the run's
  # own INPUT. Every flipped assertion below, and the flipped hash row itself,
  # would be speaking about a settings file nobody pinned, so six engine runs
  # would buy a page of failures naming everything except the cause.
  echo "run-ingame: the flipped settings file is not the bytes the golden pins" >&2
  echo "    written: $FLIPPED_DAT ($(wc -c <"$FLIPPED_DAT" | tr -d ' ') bytes)" >&2
  echo "    golden:  $FLIPPED_GOLDEN_DAT ($(wc -c <"$FLIPPED_GOLDEN_DAT" | tr -d ' ') bytes)" >&2
  echo "  Either the writer in $FKLUA_CHECKOUT changed what it produces for this" >&2
  echo "  JSON, or testdata/ingame/flipped.json was edited without re-recording" >&2
  echo "  the golden, or the golden in the tree was itself edited or corrupted." >&2
  echo "  The first two also move the flipped row's dump hashes, so re-record" >&2
  echo "  both together and read the hash diff as the real report; the third" >&2
  echo "  changes nothing the engine reads, so that diff comes back empty." >&2
  refuse "re-record deliberately with: scripts/run-ingame.sh --update"
fi
echo "  ok: the settings file is byte for byte $FLIPPED_GOLDEN_DAT"

DUMP="$USERDIR/script-output/data-raw-dump.json"
SDUMP="$USERDIR/script-output/mod-settings-dump.json"

# dump_once LANG RUN [SETTINGS] -- one packaged mod, one engine run, two hashes
# written to $TMP/hash-LANG-RUN, and the two dumps kept as
# $TMP/raw-{data,settings}-LANG-RUN.json.
#
# SETTINGS, when given, is a mod-settings.dat copied into the mod directory
# before the run; without it the engine writes its own from the declared
# defaults. THE DUMPS ARE KEPT because the assertions below have to name a
# particular row: reading whatever the last run happened to leave in
# script-output would silently re-point every default-row assertion at the
# flipped one the day a run is added.
#
# THE ENGINE REWRITES THE FILE after the run with the values it settled on
# (measured), which is harmless here only because the mod directory is torn
# down and repackaged for every single run.
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
  local settings="${3:-}"
  local moddir="$TMP/mods-$lang"
  local ndata="$TMP/normalised-data-$lang-$run.json"
  local nsettings="$TMP/normalised-settings-$lang-$run.json"
  local report="$TMP/report-$lang.json"
  local dhash shash

  rm -rf "$moddir"
  "$FKLUA" mod --data-module "$TMP/datastage-$lang.wasm" \
    --name "$MODNAME" --version "$MODVER" --author Techrocket9 \
    --factorio-version "$SERIES" \
    -o "$moddir" --report "$report" >"$TMP/pack-$lang.log" 2>&1 ||
    { cat "$TMP/pack-$lang.log" >&2; refuse "$lang: packaging failed"; }

  # THE JUMP ROW, ONCE PER LANGUAGE AND BEFORE THE ENGINE. Three runs package
  # the same wasm with the same flags (the flipped row differs only in a
  # settings file copied in after packaging), so the three reports are the same
  # report and the line belongs on the first. It sits here, above the engine
  # run, so an fklua too old to carry the row refuses in a second rather than
  # after three minutes of Factorio.
  if [ "$run" = 1 ]; then
    report_jumps "$lang" "$report"
  fi

  # MEASURED: the engine reads MODS/mod-settings.dat, beside the mod folders
  # rather than inside one.
  if [ -n "$settings" ]; then
    cp "$settings" "$moddir/mod-settings.dat" ||
      refuse "$lang: could not install $settings into $moddir"
  fi

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

  cp "$DUMP"  "$TMP/raw-data-$lang-$run.json"     || refuse "$lang: could not keep the data dump"
  cp "$SDUMP" "$TMP/raw-settings-$lang-$run.json" || refuse "$lang: could not keep the settings dump"

  # AND THE FILE THE ENGINE REWROTE, kept for the same reason and named by the
  # same row. It is what the engine SETTLED ON rather than what this script
  # asked for, and the two are not the same file: a number out of its range and
  # a dropdown value off its list are both reset to the default (measured, in
  # FkLua), and a row discarded that way is still sitting in the input.
  if [ -n "$settings" ]; then
    cp "$moddir/mod-settings.dat" "$TMP/settled-$lang-$run.dat" ||
      refuse "$lang: the engine left no mod-settings.dat in $moddir to read back"
  fi

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
DEFAULTHASH=""
FLIPPEDHASH=""
MODSET=""
for lang in go rust; do
  echo "--- $lang ---"
  started=$(date +%s)
  dump_once "$lang" 1
  dump_once "$lang" 2
  dump_once "$lang" flipped "$FLIPPED_DAT"
  h1="$(cat "$TMP/hash-$lang-1")"
  h2="$(cat "$TMP/hash-$lang-2")"
  hf="$(cat "$TMP/hash-$lang-flipped")"
  elapsed=$(( $(date +%s) - started ))

  echo "  sha256 default (data settings) $h1"
  echo "  sha256 flipped (data settings) $hf"
  echo "  three engine runs in ${elapsed}s"

  if [ "$h1" != "$h2" ]; then
    fail "$lang: two runs of the same guest produced different dumps"
    echo "        $h1" >&2
    echo "        $h2" >&2
  else
    echo "  ok: two runs agree (the data stage is deterministic)"
  fi

  # A SETTINGS FILE THE ENGINE IGNORED would leave the flipped row identical to
  # the default one, and every other check in this script would still pass:
  # the two languages would agree, both runs would agree, and the golden would
  # hold two identical hashes that prove nothing about the customizer.
  if [ "$hf" = "$h1" ]; then
    fail "$lang: the flipped row produced the same dump as the default one, so mod-settings.dat changed nothing"
    echo "        $FLIPPED_DAT was installed into $TMP/mods-$lang" >&2
  else
    echo "  ok: the flipped settings file moved the dump"
  fi

  if [ -z "$FIRSTLANG" ]; then
    FIRSTLANG="$lang"
    DEFAULTHASH="$h1"
    FLIPPEDHASH="$hf"
    MODSET="$(mod_set "$TMP/dump-$lang-2.log")"
  else
    if [ "$h1" != "$DEFAULTHASH" ]; then
      fail "$lang and $FIRSTLANG disagree about the default row"
      echo "        $FIRSTLANG $DEFAULTHASH" >&2
      echo "        $lang $h1" >&2
    else
      echo "  ok: $lang and $FIRSTLANG agree on the default row"
    fi
    if [ "$hf" != "$FLIPPEDHASH" ]; then
      fail "$lang and $FIRSTLANG disagree about the flipped row"
      echo "        $FIRSTLANG $FLIPPEDHASH" >&2
      echo "        $lang $hf" >&2
    else
      echo "  ok: $lang and $FIRSTLANG agree on the flipped row"
    fi
  fi
done

# ---------------------------------------------------------------------------
# ANTI-VACUITY AND CAUSE-NAMING. A hash says "different"; these say WHICH
# decision moved, and they fail on a dump that ran but proved nothing.
# ---------------------------------------------------------------------------
echo "== checking the dump says something"
LOG="$TMP/dump-rust-2.log"
FLOG="$TMP/dump-rust-flipped.log"
# EVERY ASSERTION NAMES ITS ROW. These are the kept dumps rather than whatever
# script-output holds, so adding a run cannot silently re-point them.
DDUMP="$TMP/raw-data-rust-2.json"
DSDUMP="$TMP/raw-settings-rust-2.json"
FDUMP="$TMP/raw-data-rust-flipped.json"

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

jqassert "the prerequisite splice reached logistics-2" "$DDUMP" \
  '(.technology["logistics-2"].prerequisites // []) | index("fkrecipes-example-hardened-steel") != null'
jqassert "the bound crafting time reached the quenching recipe" "$DDUMP" \
  '.recipe["fkrecipes-example-hardened-steel-plate-quenching"].energy_required == 3'
jqassert "the switched-on technology is enabled with no hidden field" "$DDUMP" \
  '.technology["fkrecipes-example-hardened-tips"] | (.enabled == true) and (has("hidden") | not)'
# NO PLAYER HAS TOUCHED A SETTING HERE, so every EnabledBy reads its DECLARED
# DEFAULT: hardened-tools is true, and the technology comes out enabled with no
# hidden key. The switched-OFF branch belongs to scripts/run-mirror.sh, whose
# stand-in sets the setting false; between them the two gates cover both sides
# of the hidden-not-absent decision, and neither could cover both alone.
jqassert "the setting default reached the technology it gates" "$DDUMP" \
  '.technology["fkrecipes-example-hardened-steel"] | (.enabled == true) and (has("hidden") | not)'
jqassert "research still gates the recipes it unlocks" "$DDUMP" \
  '.recipe["fkrecipes-example-hardened-steel-plate-quenching"].enabled == false and .recipe["fkrecipes-example-steel-rivet"].enabled == false'
jqassert "the recipe nothing unlocks is enabled from the start" "$DDUMP" \
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
jqassert "the cost ladder copied the real formula out of base" "$DDUMP" \
  '.technology["fkrecipes-example-hardened-tips"].unit == .technology["physical-projectile-damage-7"].unit'
jqassert "the cost ladder carried the real level cap out of base" "$DDUMP" \
  '.technology["fkrecipes-example-hardened-tips"].max_level == .technology["physical-projectile-damage-7"].max_level'
# THE PREREQUISITE MOVES WITH THE UNIT, and the first rung of that ladder names
# a technology no vanilla install has, so this also says the ladder stepped
# past what is not there rather than stopping at it.
jqassert "the prerequisite moved with the copied unit" "$DDUMP" \
  '.technology["fkrecipes-example-hardened-tips"].prerequisites == ["physical-projectile-damage-7"]'
jqassert "the ladder stepped past the technology no install has" "$DDUMP" \
  '.technology["tungsten-hardening"] == null'
jqassert "all seventeen generated settings reached the settings dump" "$DSDUMP" \
  '[paths(scalars) | select(length > 1) | .[1]] | map(select(startswith("fkrecipes-example-"))) | unique | length == 17'
# The one-sided NumericSpec arms, in the engine's own settings dump rather than
# only in the stand-in's: a declared maximum with no minimum of its own, and a
# declared minimum with no maximum. The second is craft-time-bound, so a
# maximum appearing on it would mean the generated floor-safe bound had
# replaced the consumer's declaration rather than filling a gap.
jqassert "the min-only setting kept its declared minimum and gained no maximum" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-tempering-hold")] | length > 0 and all(.minimum_value == 0.5 and (has("maximum_value") | not))'
# The two prototype-field slots the consumer round added, checked in the ENGINE
# dump rather than only in the stand-in: an order the library has a slot for,
# and a real 2.0 recipe field it does not, passed through Extra verbatim. The
# pilot measured all four of its fields DROPPED before these existed.
jqassert "the item order reached the dump" "$DDUMP" \
  '.item["fkrecipes-example-steel-rivet"].order == "b[steelworks]-a[rivet]"'
jqassert "the recipe order reached the dump" "$DDUMP" \
  '.recipe["fkrecipes-example-hardened-steel-plate-quenching"].order == "b[steelworks]-b[quenching]"'
jqassert "the Extra passthrough reached the dump" "$DDUMP" \
  '.recipe["fkrecipes-example-steel-rivet"].allow_productivity == true'
jqassert "the generated craft-time minimum reached the settings dump" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-forging-time") | .minimum_value] | any(. == 0.002)'
# A COST PRESET NAMES ITS TECHNOLOGY THROUGH ITS LOCALE KEY, in the engine's
# own settings dump, and the whole WRAPPED reference is what is pinned: the
# alternatives form, the key table, and the raw internal name LAST. A bare
# {"technology-name.<source>"} would still sit inside the wrapper, so an assert
# that only looked for the key table would go on passing after a regression;
# this one names all three slots. The cost is real: on the client an undefined
# key in a composed description costs the setting its info icon and its whole
# tooltip, while the dump says the description is present either way.
jqassert "the cost dropdown's composed description names its technology through the alternatives form" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-tips-research-tier") | .localised_description | .. | arrays
    | select(.[0]? == "?" and .[1]? == ["technology-name.military-4"] and .[2]? == "military-4")]
   | length > 0'
# AND NOTHING ANYWHERE IN THE SETTINGS DUMP IS A BARE KEY. Every string that
# names one of the three sections this library composes under must sit in slot 1
# of a wrapper whose LAST slot is a raw string, so the two counts agree. It
# catches the reordered wrapper too: with the raw fallback first, slot 1 is a
# string rather than the key table and the wrapped count drops.
# The $d and $all below are JQ variables inside a jq program, which is why the
# program is single quoted; shellcheck reads them as shell expansions.
# shellcheck disable=SC2016
jqassert "every composed locale key in the settings dump rides in the alternatives form" "$DSDUMP" \
  '[.. | objects | select(.name? // "" | startswith("fkrecipes-example-")) | .localised_description // empty] as $d
   | def isKey: type == "string" and (startswith("mod-setting-description.") or startswith("string-mod-setting.") or startswith("technology-name."));
     ([$d[] | .. | strings | select(isKey)] | length) as $all
   | ([$d[] | .. | arrays | select(.[0]? == "?") | select(.[-1] | type == "string") | .[1] | select(type == "array") | .[0] | select(isKey)] | length) as $wrapped
   | $all > 0 and $all == $wrapped'
# WHAT THE SCREEN OWES A PLAYER TYPING INTO A TEXT FIELD, in the engine's own
# settings dump. Both sentences are the library's own composition, and both
# answer something a client measurement found stated nowhere a player looks:
# the prototype declares NO maximum-length key at all (the engine would store
# 98000 characters), so the length sentence IS the limit as far as the settings
# screen goes; and a text this library cannot use is set aside for the declared
# list, which the screen cannot show because the field still holds what the
# player typed. That second sentence promises the NARROW claim and not a load:
# the declared list is held to every rule it always was, so a modpack where it
# cannot produce a legal result still stops the load, and on a refused load no
# log op reaches the host, which is why the sentence names the load error too.
# The dump carries the localised
# string TABLE and not the rendered text (--dump-data does not read locale), so
# what this proves is the shape and the bytes, which is what a later edit would
# drop. One row per sentence, so a failure names which one went.
jqassert "the text setting's composed description states the length limit" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-rivet-ingredients") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nWrite internal names, as the default line above does, in at most 2000 characters. The word none empties the list, so the recipe costs nothing to craft."))'
# THE WORD none IS A FEATURE AND IT IS DISCLOSED ON THE ONE KIND THAT TAKES IT.
# It empties the ingredient list, the recipe reaches the game with no
# ingredients at all and the engine's derived recycling recipe goes with it, and
# nothing a player reads used to say so. A PACKS list REFUSES the word
# ("research takes at least one science pack"), so naming it there would tell a
# player to type something this library turns down: the row below reads every
# packs setting in the dump and asserts the clause is absent from all of them.
jqassert "no packs setting names a word the library refuses there" "$DSDUMP" \
  '[.. | objects
    | select((.name? // "") | startswith("fkrecipes-example-") and endswith("-packs"))
    | .localised_description // []
    | .. | strings]
   | length > 0 and all(contains("The word none") | not)'
# THE WRAP, DISCLOSED WHERE THE WRAPPED TEXT IS. A line too long for the tooltip
# breaks and the continuation starts at the LEFT MARGIN (measured on the
# client), so it reads as a line of its own and a player who copies what looks
# like a whole line loses the last ingredient. The wrap is the engine's and no
# composition can change it; saying that the continuation belongs to the line
# above it is what a composition can do. It says "the continuation" and not
# "both lines" because two is not a bound: the rendered list is the author's own
# and runs to the language's 2000-character ceiling, so three and more visual
# lines are reachable, and on a dropdown the line above the list is the
# consumer's label, which is not a list at all. Both places a typeable list is
# rendered carry it: the text setting's own default line and an ingredient
# dropdown's preset lines.
jqassert "the text setting's composed description discloses the wrap" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-rivet-ingredients") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nA list too long for one line continues on the next; the continuation is part of the same list."))'
jqassert "an ingredient dropdown discloses the wrap beside its preset lines" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-quench-medium") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nA list too long for one line continues on the next; the continuation is part of the same list."))'
# AND NOT ON A COST DROPDOWN, whose preset is a localised label followed by a
# localised technology name: one vocabulary, prose throughout, nothing in it to
# copy. A sentence there would be about a hazard that preset does not carry.
jqassert "a cost dropdown carries no line about a list it does not render" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-tips-research-tier") | .localised_description | .. | strings]
   | length > 0 and all(contains("A list too long") | not)'
jqassert "the text setting's composed description states what an unusable text costs" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-rivet-ingredients") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nA text this mod cannot use is set aside and that default applies instead; the reason is in the log, or in the load error if the load stops anyway."))'
# WHICH OF THE TWO FIELDS IS DECIDING, which the settings screen cannot show at
# all: it has no conditional visibility (measured), so a player looking at a
# text field beside a dropdown has nowhere else to learn that one of them wins.
# Both sides of the pairing, because the player may be looking at either.
jqassert "a text setting with no dropdown beside it names its own list" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-rivet-ingredients") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nWhile this says default this mod'"'"'s own list applies."))'
jqassert "a text setting beside a dropdown names the option that decides" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-quench-ingredients") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nWhile this says default the option chosen above applies; anything else applies instead of it."))'
jqassert "the dropdown says the text setting beside it overrides it" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-quench-medium") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nThe setting below applies instead while it does not say default."))'
# A RESEARCH NUMBER STATES ITS RANGE, and beside a research dropdown it states
# what 0 means: a numeric field on that screen shows no bounds at all, and a 0
# that silently defers to a dropdown is not something anyone can guess.
jqassert "a research number beside a dropdown says what 0 means" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-tips-count") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nA whole number from 0 to 100000. While it is 0 the option chosen above decides."))'
jqassert "a research number with no dropdown states its range" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-chain-seconds") | .localised_description]
   | length > 0 and all(any(.[]; . == "\nA whole number from 1 to 600."))'
# AN INGREDIENT PRESET IS TWO LINES, not one run of text in two vocabularies.
# The label before it is the consumer's display prose, which the client
# truncates at about 37 characters in the closed dropdown; the internal names
# the field beside it takes are on their own line under the word a player acts
# on, so the copyable half of the tooltip is never the truncated half.
jqassert "an ingredient preset puts the internal names on their own line" "$DSDUMP" \
  '[.. | objects | select(.name? == "fkrecipes-example-quench-medium") | .localised_description | .. | arrays
    | select(.[2]? | type == "array")
    | select(.[2][1]? | type == "array")
    | select(.[2][1][0]? | startswith("string-mod-setting."))
    | .[3]]
   | length > 0 and all(startswith("\n  type: "))'

# ---------------------------------------------------------------------------
# THE FLIPPED ROW. Everything above reads the default dump, where no player has
# touched anything. These read the run that had testdata/ingame/flipped.json in
# front of it, and each one names a path only the engine can walk: the settings
# stage stores a value, the data stage reads it back, and no stand-in has a
# stored value to read.
# ---------------------------------------------------------------------------
echo "== checking the flipped row says something"

# THE DROPDOWN DECIDES WHERE THE TEXT IS REFUSED, and that is the narrow
# property the whole player-fallback decision rests on, measured here on a real
# engine for the first time: quench-medium is on the oil preset and
# quench-ingredients holds a text the language refuses, so the recipe comes out
# of the preset's own plan, ladders and all, exactly as it would for a player
# who typed nothing. The mirror leaves a dropdown deciding through an UNTOUCHED
# text on the chain pair, so between the two gates both ways of saying "the row
# above decides" are walked.
# MEASURED: neither rung of that plan's oil ladder is in a stock 2.0.77 install,
# so the third ingredient is dropped with its line and the two that resolve are
# what the recipe carries.
jqassert "the preset applied while the text beside it was refused" "$FDUMP" \
  '.recipe["fkrecipes-example-hardened-steel-plate-quenching"].ingredients ==
   [{"amount":2,"name":"steel-plate","type":"item"},{"amount":2,"name":"fkrecipes-example-steel-rivet","type":"item"}]'
grep -q "fkrecipes: hardened-steel-plate-quenching: none of light-oil-barrel, crude-oil-barrel is present, so the ingredient is dropped" "$FLOG" ||
  fail "the preset's own ingredient ladder logged nothing in the engine's own log"
# THE WHOLE LIST OF A RECIPE WITH NO DROPDOWN in front of it, in the order the
# player wrote rather than the order the mod declared. The mirror puts a REFUSED
# text on this same setting, so one gate covers each side of the arm.
jqassert "the player's typed list reached the recipe that has no dropdown" "$FDUMP" \
  '.recipe["fkrecipes-example-steel-rivet"].ingredients ==
   [{"amount":2,"name":"iron-stick","type":"item"},{"amount":1,"name":"steel-plate","type":"item"}]'
# A TYPED LIST THAT TAKES A PRESET OVER, carrying a FLUID and a FRACTIONAL
# amount. This is the whole reason the chain recipe is crafting-with-fluid: the
# engine refuses a fluid in the crafting category (measured), so a customizable
# recipe that wants to allow water has to say where it is crafted, and a fluid
# ingredient reaching a real prototype loader is the only proof that holds. The
# fraction is what says a fluid amount is a double all the way through rather
# than an item count wearing a decimal point.
jqassert "the player's typed list took the dropdown's choice over" "$FDUMP" \
  '.recipe["fkrecipes-example-steel-chain"].ingredients ==
   [{"amount":2,"name":"fkrecipes-example-steel-rivet","type":"item"},{"amount":0.5,"name":"water","type":"fluid"}]'
# A RESEARCH COST OVERRIDDEN WHOLE, in the short tuple form, and PLACED BY THE
# TIER: the count and the time are the player's, the pack TEXT IS THE TYPO and
# so falls back exactly as the reserved word behaves, which beside a tier means
# the MILITARY tier's own packs. "Loaded with its own default instead" in the
# prototype rather than only in the log, on a real engine. This is the
# measurement the whole decision rests on: before it, this row exited 1 with
# "Failed to load mod" and no dump at all, and a player in that state could not
# reach the Mod Settings screen to undo it (see the client walk in the library's
# player_fallback). The mirror moves ONE of the three fields on the same
# technology, so between the two gates the merge is covered whole and per field.
# THE PACKS ARE military-4'S OWN, read out of the same dump rather than written
# here: this asserts "the tier supplied the field the text left at its default"
# rather than "the packs are what I typed into this script", and a base game
# that reprices military-4 moves both sides together.
jqassert "the player's research cost reached the technology" "$FDUMP" \
  '.technology["fkrecipes-example-hardened-tips"].unit ==
   ((.technology["military-4"].unit) + {"count":40,"time":20})'
jqassert "the tier's own packs are what the refused text fell back onto" "$FDUMP" \
  '.technology["fkrecipes-example-hardened-tips"].unit.ingredients ==
   .technology["military-4"].unit.ingredients'
jqassert "the chosen tier placed the technology" "$FDUMP" \
  '.technology["fkrecipes-example-hardened-tips"].prerequisites == ["military-4"]'
# military-4 carries no level cap, so neither does the technology priced from
# it: max_level is a field a COPY brings across from its source, and the default
# row's own source is the one that has one.
jqassert "the unit carried no level cap its source never had" "$FDUMP" \
  '.technology["fkrecipes-example-hardened-tips"] | has("max_level") | not'
# THE LOG IS NOT A DISCLOSURE, and this is the line that is. The technology
# whose pack text the language refused says so in its OWN description, which is
# what the player hovers in the tech tree, joined onto the author's own sentence
# rather than replacing it. Pinned whole and verbatim: the author's line, a
# newline, and the library's.
#
# ENGLISH LITERALS AND NO LOCALE KEY, deliberately. An UNDEFINED key anywhere in
# a prototype description deletes the WHOLE description on the client, silently
# and with exit 0, while the dump below still holds every byte of it (measured
# on 2.0.77 on a recipe prototype), so no assertion in this file could ever see
# that happen and the library composes nothing a consumer has to define.
jqassert "the technology whose pack text was set aside says so in its own tooltip" "$FDUMP" \
  '.technology["fkrecipes-example-hardened-tips"].localised_description ==
   ["", "Every level puts a harder edge on the same tools.",
    "\nThe stored value of fkrecipes-example-tips-packs could not be used, so this mod'"'"'s own choice applies instead. The reason is in the log."]'
# AND A PROTOTYPE NOTHING FELL BACK ON CARRIES NO NOTE, which is what says the
# line is a consequence of the fallback rather than something every prototype
# now has. The chain recipe in this row took the list the player TYPED, so
# nothing about it was set aside and its description is the author's alone.
jqassert "a recipe with no fallback carries the author's description and no note" "$FDUMP" \
  '.recipe["fkrecipes-example-steel-chain"].localised_description == ["", "Links of rivets"]'
# AND THE RECIPE WHOSE INGREDIENT TEXT WAS REFUSED SAYS SO IN ITS OWN TOOLTIP,
# WHICH IS THE ONE THING A RECIPE COULD NOT DO. A localised string element may
# be 200 BYTES on a data-stage prototype (measured on 2.0.77: 201 refuses with
# "Localised string key is too large: 201 > 200 (limit)." naming the 0-based
# element, bytes and not characters, with no aggregate budget), and this note is
# 245 bytes with its newline for this setting's name. Until the library chunked
# it, this exact row exited 1 with no dump at all, which is the lock-out the
# fallback exists to prevent reintroduced by the fallback's own disclosure. The
# four elements are pinned WHOLE: the author's own sentence first, then the note
# split at a word boundary, and the engine concatenates them back into one
# sentence for the player.
jqassert "the recipe whose ingredient text was set aside says so in its own tooltip" "$FDUMP" \
  '.recipe["fkrecipes-example-hardened-steel-plate-quenching"].localised_description ==
   ["", "Quench the plate, then temper it back to workable.",
    "\nThe stored value of fkrecipes-example-quench-ingredients could not be used, so this mod'"'"'s own choice applies instead. The reason is in the log. Changing a recipe empties an ",
    "assembling machine'"'"'s input slots of anything the new list does not use."]'
# AND NO ELEMENT OF ANY LOCALISED STRING IN THE WHOLE DUMP IS OVER THE CEILING,
# base's own prototypes included. This is the general form of the assertion
# above and the one that does not have to be re-written when a sentence moves: a
# composition this library grows past 200 bytes fails here by name rather than
# at a player's engine. utf8bytelength and not length, because the rule is bytes.
jqassert "every localised element in the flipped dump is inside the engine ceiling" "$FDUMP" \
  '[.. | objects | (.localised_description?, .localised_name?) | select(. != null)
    | .. | strings | select(utf8bytelength > 200)] == []'
jqassert "every localised element in the default dump is inside the engine ceiling" "$DDUMP" \
  '[.. | objects | (.localised_description?, .localised_name?) | select(. != null)
    | .. | strings | select(utf8bytelength > 200)] == []'

# THE LOG LINES, in the engine's own log. A player who typed gets one line per
# thing they changed, and it renders the list CANONICALLY rather than quoting
# what they typed, so they learn the form the library would have written.
grep -q "fkrecipes: fkrecipes-example-steel-chain takes its ingredients from fkrecipes-example-chain-ingredients: 2 fkrecipes-example-steel-rivet, 0.5 \[fluid=water\]; the fkrecipes-example-chain-links choice long is set aside" "$FLOG" ||
  fail "the text that took a dropdown's list over logged no set-aside clause"
# AND THE ERROR LINE FOR THE TEXT THE LANGUAGE REFUSED, in the engine's own
# log. Factorio's log() has one channel and no severity, so the word ERROR is in
# the text; it is uppercase so a case-sensitive grep for the engine's own Error
# lines does not collect it while a case-insensitive one still finds it. The
# sentence inside it is the language's own, verbatim, with the shared
# "fkrecipes: " prefix trimmed off because the line already opens with one.
grep -q 'fkrecipes: ERROR: fkrecipes-example-tips-packs, entry 2 ("1 militar-science-pack"): no science pack is named militar-science-pack\. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart\.' "$FLOG" ||
  fail "the refused pack text logged no ERROR line in the engine's own log"
# AND THE SAME FOR THE REFUSED INGREDIENT TEXT, which is the RECIPE channel of
# the same rule. Its sentence carries one clause the pack text's does not: what
# the engine does to an assembling machine when the recipe it is running
# changes, which is true of a moved ingredient list and of nothing else.
grep -q 'fkrecipes: ERROR: fkrecipes-example-quench-ingredients, entry 1 ("2 iron-plat"): no item or fluid is named iron-plat\. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart\. Changing a recipe empties an assembling machine'"'"'s input slots of anything the new list does not use\.' "$FLOG" ||
  fail "the refused ingredient text logged no ERROR line in the engine's own log"
# AND EXACTLY TWO OF THEM. Two player-controlled values in this row are wrong,
# one per channel, so two lines are the whole answer: a third would mean a
# setting nobody edited had been reported, or one field reported twice, and a
# gate that only asked whether an ERROR line is PRESENT could not tell either
# apart from a pass.
errors=$(grep -c "fkrecipes: ERROR: " "$FLOG" || true)
[ "$errors" = 2 ] ||
  fail "the flipped row logged $errors fkrecipes ERROR lines in the engine's own log, want 2"
grep -q "fkrecipes: fkrecipes-example-steel-rivet takes its ingredients from fkrecipes-example-rivet-ingredients: 2 iron-stick, 1 steel-plate" "$FLOG" ||
  fail "the edited ingredient text logged nothing in the engine's own log"
# THE LINES THAT MUST NOT BE THERE, and the file has to EXIST for the question
# to have been asked. A negative grep over a path that is not there answers "no
# match" and reads exactly like a pass, which is the one way this whole block
# could be vacuous.
[ -s "$FLOG" ] || fail "the engine's log $FLOG is missing or empty, so nothing below was actually checked"
# A refused text is not a list that was read, so nothing may report it as one.
if grep -q "takes its research cost from fkrecipes-example-tips-packs: count 40, time 20, packs 2 automation-science-pack" "$FLOG"; then
  fail "a refused pack text was logged as a list the research took"
fi
# AND WHAT IT DID FALL BACK ONTO, said out loud: the cost line comes out
# whatever the text said, because the count and the time are read on every load,
# and the packs it names are the TIER'S, which is where a player who typed
# nothing in that field lands.
grep -q "fkrecipes: fkrecipes-example-hardened-tips takes its research cost from fkrecipes-example-tips-packs: count 40, time 20, packs 1 automation-science-pack, 1 logistic-science-pack, 1 chemical-science-pack, 1 military-science-pack, 1 utility-science-pack; the fkrecipes-example-tips-research-tier choice military supplies what the settings leave at default" "$FLOG" ||
  fail "the custom research cost logged nothing in the engine's own log"
# An untouched text is the reserved word, so the dropdown decides and no line is
# written about the text at all.
if grep -q "takes its ingredients from fkrecipes-example-quench-ingredients" "$FLOG"; then
  fail "an untouched text was logged as an edit"
fi
# NOTHING IS EDITED AND IGNORED ANY MORE. Every non-default value is live, so
# the sentence that used to say otherwise may not appear in an engine's log
# either.
if grep -q "is edited, but" "$FLOG"; then
  fail "a value was reported as edited and ignored, which nothing does now"
fi

# WHAT THE ENGINE SETTLED ON, read back rather than assumed. Everything above
# reads a dump, which says what the data stage DID; this reads the
# mod-settings.dat the engine REWROTE after the run, which says what it kept.
# The engine resets a number out of its range and a dropdown value off its list
# to the default (measured, in FkLua), so a row discarded that way is still in
# the file this script wrote and only the file the engine wrote can say so.
# Every value in flipped.json is in range and on its list, so the answer here is
# "all of them" and a drift is a finding either way: the engine changed what it
# accepts, or the mod stopped declaring a setting the file names.
echo "== reading back what the engine settled on"
SETTLED_DAT="$TMP/settled-rust-flipped.dat"
SETTLED_JSON="$TMP/settled-rust-flipped.json"
if ! "$FKLUA" modsettings read "$SETTLED_DAT" >"$SETTLED_JSON" 2>"$TMP/settled-rust-flipped.err"; then
  fail "the mod-settings.dat the engine rewrote does not read back"
  sed 's/^/        /' "$TMP/settled-rust-flipped.err" >&2
elif [ ! -s "$SETTLED_JSON" ]; then
  # AN EMPTY DOCUMENT WOULD PASS THE QUERY BELOW WITHOUT SAYING A WORD: jq
  # over zero input runs its program zero times, so DRIFT comes back empty and
  # the ok line prints over a read that produced nothing. The check is what
  # makes that ok line mean the engine kept the values rather than mean the
  # reader had nothing to compare.
  fail "the engine's rewrite read back as an empty document"
else
  # ONE LINE NAMING EVERY KEY THAT MOVED, rather than a bare true/false: the
  # rewrite carries every setting the mod declares and this compares only the
  # ones the flipped file installed, so the interesting output is which of them
  # the engine did not keep.
  DRIFT="$(jq -r --slurpfile want "$FLIPPED_JSON" '
      .startup as $got
      | [ $want[0].startup | to_entries[] | . as $e
          | select(($got | has($e.key) | not) or ($got[$e.key] != $e.value))
          | "\($e.key): installed \($e.value | tojson), settled \($got[$e.key] | tojson)" ]
      | join("; ")' "$SETTLED_JSON")" ||
    DRIFT="the query over $SETTLED_JSON failed"
  if [ -z "$DRIFT" ]; then
    echo "  ok: the engine's own rewrite still carries every value the flipped file installed"
  else
    fail "the engine did not keep every value the flipped file installed: $DRIFT"
  fi
fi

# ---------------------------------------------------------------------------
# The golden. TWO ROWS PER ENGINE, tagged, because one file now describes two
# loads of the same mod: the settings as the mod declares them, and the
# settings as a player typed them.
# ---------------------------------------------------------------------------
DEFAULT_LINE="$ENGINE default $DEFAULTHASH $MODSET"
FLIPPED_LINE="$ENGINE flipped $FLIPPEDHASH $MODSET"
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
    strict_mods="$(grep "^$ENGINE default " "$GOLDEN" | cut -d' ' -f5- || true)"
  fi
  if [ -n "$strict_mods" ] && [ "$strict_mods" != "$MODSET" ]; then
    echo "  refusing to record the hash rows from a mod set those rows do not carry; drop --strict to re-record after an environment change" >&2
    echo "    golden: $strict_mods" >&2
    echo "    here:   $MODSET" >&2
    FAIL=1
  elif [ "$FAIL" != 0 ]; then
    # THE HASH ROWS ONLY. flipped.golden.dat has already been re-recorded
    # above and deliberately so: it is a function of flipped.json and of the
    # writer, and a run that fails on an engine hash has still written the
    # right bytes.
    echo "  refusing to record the hash rows from a failing run" >&2
  else
    if [ -f "$GOLDEN" ]; then
      grep -v "^$ENGINE " "$GOLDEN" > "$GOLDEN.tmp" || true
      mv "$GOLDEN.tmp" "$GOLDEN"
    else
      cat > "$GOLDEN" <<'EOF'
# The in-game dump hashes, two lines per engine.
#
#   <engine> <row> <data-sha256> <settings-sha256> <mod set, name@version, sorted>
#
# ENGINE, because a dump is a function of the engine that produced it: a new
# Factorio moves base prototypes and every hash with them, and a line keyed by
# version lets one file hold several without either one being wrong.
#
# THE ROW, default or flipped, because the dump is also a function of the
# settings the engine read. The default row is every setting where the mod
# declared it; the flipped row is testdata/ingame/flipped.json written into the
# mod directory as mod-settings.dat by `fklua modsettings write`, which is a
# player who opened the settings screen and typed. Only the engine can run that
# path, so only this file can pin it.
#
# THE BYTES OF THAT FILE ARE PINNED TOO, beside this one as flipped.golden.dat.
# The writer belongs to the toolchain now, so the gate compares what it
# produces for flipped.json against those bytes on every run and a codec change
# upstream fails on the file rather than only on a hash of a dump. Both goldens
# are re-recorded by the same --update.
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
# Capture both rows with: scripts/run-ingame.sh --update
EOF
    fi
    printf '%s\n%s\n' "$DEFAULT_LINE" "$FLIPPED_LINE" >> "$GOLDEN"
    echo "== golden recorded: $DEFAULT_LINE"
    echo "== golden recorded: $FLIPPED_LINE"
  fi
elif [ ! -f "$GOLDEN" ]; then
  fail "no golden at $GOLDEN; capture it deliberately with: scripts/run-ingame.sh --update"
else
  # compare_row ROW HASHES KEPT -- one tagged golden line against one row's
  # hashes. The mod-set arm sets SKIPPED and the hash arm FAILs, which is the
  # split the header argues for: a different DLC set is a different world, and
  # a hash taken in one cannot speak about the other.
  compare_row() {
    local row="$1" hashes="$2" kept="$3"
    local want want_mods want_hashes
    want="$(grep "^$ENGINE $row " "$GOLDEN" || true)"
    if [ -z "$want" ]; then
      fail "the golden has no $row row for Factorio $ENGINE; capture both with: scripts/run-ingame.sh --update"
      return
    fi
    want_mods="$(printf '%s' "$want" | cut -d' ' -f5-)"
    want_hashes="$(printf '%s' "$want" | cut -d' ' -f3,4)"
    if [ "$want_mods" != "$MODSET" ]; then
      echo "  SKIPPED: the $row row's mod set differs from the golden's, so the hashes are not comparable" >&2
      echo "    golden: $want_mods" >&2
      echo "    here:   $MODSET" >&2
      SKIPPED=1
      return
    fi
    if [ "$want_hashes" != "$hashes" ]; then
      fail "the $row dumps do not match the golden for Factorio $ENGINE"
      echo "    golden: $want_hashes" >&2
      echo "    here:   $hashes" >&2
      echo "    the dumps are at $kept" >&2
      return
    fi
    echo "  ok: the $row dumps match the golden for Factorio $ENGINE"
  }
  compare_row default "$DEFAULTHASH" "$TMP/raw-data-$FIRSTLANG-2.json and $TMP/raw-settings-$FIRSTLANG-2.json"
  compare_row flipped "$FLIPPEDHASH" "$TMP/raw-data-$FIRSTLANG-flipped.json and $TMP/raw-settings-$FIRSTLANG-flipped.json"
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
