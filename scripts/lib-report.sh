# shellcheck shell=bash
#
# THE JUMP ROW OF A PACKAGING REPORT, read by both gates.
#
# WHY A GATE READS THIS AT ALL. Lua refuses a function whose goto-to-label
# distance crosses 18 bits, and a generated data module concentrates a whole
# plan into a handful of functions. Module size is free (a chunk is parsed once
# and never reaches a save); that CONCENTRATION is not. Until `fklua mod
# --report` carried the numbers, the only way an author learned where the wall
# was was the refusal, and this library is the one that learned it the wrong
# way: it transcribed the emitter's scan into a script of its own, and read the
# widest span left AFTER the relay as headroom. That number is about one hop
# whatever the guest's size, so a module half again over the limit reads as a
# comfortable half of it. The report exists to make that reading impossible,
# and this function is the gates taking it up.
#
# WHAT THE LINE SAYS. Two numbers, and only the first is about today. The
# widest span BEFORE the relay is how near Lua's wall this guest already is.
# The BLOCK ROOM is the ceiling this library will actually meet: a station is a
# label, a label only goes at function body level, and the relay refuses when a
# run with no body-level statement boundary in it is longer than one hop. So
# the room left of one hop over the module's longest such run is what bounds
# the guest as it grows, and the span is what bounds it today. The two live in
# different functions on every real guest measured, which is why the line names
# both.
#
# NEITHER GATE OWNS IT. run-mirror.sh and run-ingame.sh package the same two
# guests, so a jump row printed by one and not the other is a number whose
# meaning depends on which script somebody ran.
#
# The sourcing script supplies refuse() (so the message carries that gate's own
# name), FKLUA_CHECKOUT (so the remedies name a real directory) and jq on PATH.

# report_jumps LANG REPORT -- print the data module's jump row out of the
# packaging report at REPORT, and refuse when the relay is out of room.
report_jumps() {
  local lang="$1"
  local report="$2"
  local row limit hop span spanfn stations block blockfn room n pct

  if ! declare -F refuse >/dev/null; then
    echo "lib-report.sh: the sourcing script defines no refuse()" >&2
    exit 1
  fi
  # jq IS THIS FUNCTION'S OWN PRECONDITION, checked here as well as by the
  # gates: without it the refusal below would blame the report for a tool that
  # is not on PATH.
  command -v jq >/dev/null || refuse "$lang: jq is not on PATH; the packaging report cannot be read"
  if [ ! -f "$report" ]; then
    refuse "$lang: no packaging report at $report; \`fklua mod\` needs --report FILE"
  fi

  # ONE jq CALL, AND ITS STATUS READ DIRECTLY. `jq ... | read` reports read's
  # status, so an unreadable report would leave every field empty and the gate
  # would print a line about nothing and carry on. The assignment is on its own
  # line for the same reason: `local row="$(jq ...)"` is local's status and
  # never jq's.
  row="$(jq -r '
    if has("jumps") and (.jumps.data != null) then
      [ .jumps.data.limit_bytes,
        .jumps.data.hop_bytes,
        .jumps.data.widest_span_bytes,
        (.jumps.data.widest_span_function | if . == null or . == "" then "(no function)" else . end),
        .jumps.data.stations,
        .jumps.data.widest_block_bytes,
        (.jumps.data.widest_block_function | if . == null or . == "" then "(no function)" else . end),
        .jumps.data.block_room_bytes ] | @tsv
    else
      "no-jumps-object"
    end' "$report")" ||
    refuse "$lang: the packaging report at $report is not readable JSON, so the
  jump row cannot be read. It is written by \`fklua mod --report\`; a truncated
  or empty one means the packaging step above is where to look."
  # AND AN EMPTY REPORT IS NOT AN EMPTY LINE. jq over a file holding no JSON
  # document at all runs the filter zero times and exits 0, which is the one
  # way an unreadable report gets past the check above.
  if [ -z "$row" ]; then
    refuse "$lang: the packaging report at $report holds no JSON document"
  fi

  # AN OLDER fklua IS AN ENVIRONMENT, NOT A DEFECT, and it has to say so out
  # loud: a gate that quietly prints nothing about the jump limit reads exactly
  # like a gate that found the guest comfortable.
  if [ "$row" = "no-jumps-object" ]; then
    refuse "NOT RUN: the packaging report for $lang carries no jumps object.
  FKLUA_CHECKOUT points at
    $FKLUA_CHECKOUT
  and the fklua built from it is older than 2a541a7, where \`fklua mod --report\`
  grew the per-module jump row. Without it neither gate can say how near Lua's
  18-bit jump limit this guest is, or how much room the relay has left.
  Point FKLUA_CHECKOUT at an FkLua checkout at 2a541a7 or later."
  fi

  # THE TWO NAMES ARE NEVER EMPTY, by the fallback in the filter above: tab is
  # IFS whitespace, so an empty field would collapse and shift every later one
  # left, and the numeric check below would then blame a number that was never
  # wrong. A module with no jumps at all (every measure 0, no function named)
  # reads "(no function)" and prints as such.
  IFS=$'\t' read -r limit hop span spanfn stations block blockfn room <<<"$row" ||
    refuse "$lang: could not split the jump row read out of $report"

  # EVERY MEASURED FIELD IS A NUMBER OR THIS SAYS SO. An absent field arrives
  # from @tsv as the empty string, bash arithmetic reads that as 0, and the
  # line would report a guest with no jumps at all in perfect confidence.
  for n in "$limit" "$hop" "$span" "$stations" "$block" "$room"; do
    if ! [[ "$n" =~ ^-?[0-9]+$ ]]; then
      refuse "$lang: the jumps row in $report is not a row of numbers (read \"$n\")"
    fi
  done
  if [ "$limit" -le 0 ]; then
    refuse "$lang: the jumps row in $report gives a limit of $limit bytes"
  fi

  # Rounded, not truncated, so this agrees with the percentage `fklua mod`
  # prints for the same module (math.Round in cmd/fklua's jumpNote). The span
  # is never negative, so rounding half away from zero needs no sign arm.
  pct=$(( (span * 200 + limit) / (2 * limit) ))
  # "relayed through N stations" is what says why a guest over 100% still
  # loads; "no relay needed" is fklua's own wording for the other case.
  local relay
  if [ "$stations" -gt 0 ]; then
    relay="relayed through $stations stations"
  else
    relay="no relay needed"
  fi
  echo "   $lang data jumps: widest span $span bytes ($pct% of the $limit-byte limit) in $spanfn, $relay; widest block $block bytes in $blockfn, $room bytes of block room"

  # THE ONE FAILING NUMBER. Negative block room is not a warning about growth:
  # it is the relay having nowhere to put a station inside a run it has to
  # break up, and the module stops packaging on the day its span crosses.
  if [ "$room" -lt 0 ]; then
    refuse "$lang: the relay is out of room. $blockfn carries a run of $block bytes
  with no body-level statement boundary in it, against the $hop bytes one hop
  covers, which leaves $room bytes of block room. The relay cannot break that
  run up, so this module is refused the day the span in $blockfn crosses
  $limit bytes (the widest span above may belong to another function, and on
  every guest measured so far it does).
  What to do about it is in $FKLUA_CHECKOUT/docs/lua-limits.md."
  fi
}
