#!/usr/bin/env python3
"""How much text each prototype this library composes puts in front of a player.

Usage:

    python3 scripts/tooltip-sizes.py testdata/mirror/transcript.golden
    python3 scripts/tooltip-sizes.py testdata/mirror/transcript.golden -v

It reads the mirror transcript's TRANSCRIPT extend# lines, takes the
localised_description off each prototype, and renders it the way a client with
NO locale entries of its own would: an alternatives table {"?", {"key"}, "raw"}
renders as its LAST alternative, the raw fallback, because an undefined key is a
failed alternative and the raw string always resolves. Every other table
renders as the concatenation of its parameters past the first. That is the
worst case a consumer can ship, and it is also the case the library's own
sentences are the whole of.

Two numbers per row. `whole` is what the player reads; `library` is the same
text with parameters 1 and 2 left out, which are the leading "" and the head of
the composition, either the consumer's own [mod-setting-description] reference
or the literal DescribeSetting wrote, so it is the part this library composed
rather than the part the consumer wrote. A description that is a plain string
is one the plan wrote and nothing was composed onto, so its library part is 0. `-v` prints the
rendered text under each row.

It is a measurement tool and not a gate: nothing runs it, it asserts nothing,
and it reads a committed file rather than a game.
"""

import re
import sys


def parse(s, i):
    """Parse one Lua-ish value out of the transcript at offset i."""
    if s[i] == "{":
        i += 1
        items = []
        while s[i] != "}":
            m = re.match(r'(\d+|"[^"]*")=', s[i:])
            i += m.end()
            v, i = parse(s, i)
            items.append((m.group(1).strip('"'), v))
            if s[i] == ",":
                i += 1
        return items, i + 1
    if s[i] == '"':
        j = i + 1
        out = ""
        while s[j] != '"':
            if s[j] == "\\":
                out += {"n": "\n", '"': '"', "\\": "\\", "t": "\t"}[s[j + 1]]
                j += 2
            else:
                out += s[j]
                j += 1
        return out, j + 1
    m = re.match(r"[^,}]+", s[i:])
    return m.group(0), i + m.end()


def render(v):
    """Render a parsed localised string as a client with no locale entries would."""
    if isinstance(v, str):
        return v
    d = dict(v)
    if d.get("1") == "?":
        return render(d[str(len(d))])
    return "".join(render(x) for _, x in v)


def main(argv):
    if len(argv) < 2:
        sys.exit(__doc__)
    verbose = "-v" in argv
    for line in open(argv[1]).read().split("\n"):
        if not line.startswith("TRANSCRIPT extend#"):
            continue
        value, _ = parse(line[line.index("{"):], 0)
        proto = dict(dict(value)["1"])
        if "localised_description" not in proto:
            continue
        desc = proto["localised_description"]
        whole = render(desc)
        # A PLAIN STRING IS A DESCRIPTION THE PLAN WROTE AND THE LIBRARY
        # COMPOSED NOTHING ONTO: DescribeSetting on a setting with no
        # composition of its own emits the literal alone, so there is no
        # parameter list to drop the first two of and the library's part is
        # empty. Where a composition DOES open with such a literal the shape is
        # unchanged, because the literal stands in parameter 2's place.
        if isinstance(desc, str):
            library = ""
        else:
            library = "".join(render(x) for k, x in desc if k not in ("1", "2"))
        print(
            f"{proto['name']:52s} {proto['type']:15s}"
            f" whole={len(whole):4d} library={len(library):4d}"
        )
        if verbose:
            print(whole)
            print("-----")


if __name__ == "__main__":
    main(sys.argv)
