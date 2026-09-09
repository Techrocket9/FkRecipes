//! THE CORPUS IS THE CONTRACT.
//!
//! testdata/ingredient-list/cases.txt is read by BOTH language halves, and
//! every case in it is an input and the exact text that must come back: the
//! canonical rendering, or the refusal to the byte. A message reworded in one
//! half without the corpus is a red suite here and a red suite in Go, which
//! is the only way two parsers written twice stay one language.
//!
//! The file is read from disk rather than pasted in: a copy would drift, and
//! a drifted copy would pass.
//!
//! WHAT IS NOT IN THE CORPUS is here instead, because it is not a sentence:
//! the render-then-parse identity, which is what lets a rendering be handed
//! back to the parser anywhere, the two whole-text guards a corpus case can
//! only spell one way, and the World's two default methods, whose panic is
//! the message a consumer's fixture meets.

use std::fs;
use std::panic;
use std::path::PathBuf;

use crate::ingredient_list::{format_amount, parse, render, IngredientList, ListKind, ListText};
use crate::plan::Amount;
use crate::tests::escape_bytes;
use crate::value::Value;
use crate::world::{Named, World};

/// The fixture the corpus states in its own header: three lists of names and
/// nothing else. Every other question is unreachable from `parse`, and a body
/// that panics is what says so.
struct CorpusWorld {
    items: Vec<String>,
    fluids: Vec<String>,
    tools: Vec<String>,
}

impl Named for CorpusWorld {
    fn mod_name(&self) -> String {
        String::from("mymod")
    }
}

impl World for CorpusWorld {
    fn item_exists(&self, name: &str) -> bool {
        self.items.iter().any(|n| n == name)
    }

    fn fluid_exists(&self, name: &str) -> bool {
        self.fluids.iter().any(|n| n == name)
    }

    fn tool_exists(&self, name: &str) -> bool {
        self.tools.iter().any(|n| n == name)
    }

    fn startup_setting(&self, _name: &str) -> Option<Value> {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
    fn tech_names(&self) -> Vec<String> {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
    fn tech_prereqs(&self, _name: &str) -> Vec<String> {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
    fn tech_unit(&self, _name: &str) -> Option<Value> {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
    fn tech_max_level(&self, _name: &str) -> Option<Value> {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
    fn tech_has_research_trigger(&self, _name: &str) -> bool {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
    fn tech_exists(&self, _name: &str) -> bool {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
    fn entity_exists(&self, _name: &str) -> bool {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
    fn recipe_exists(&self, _name: &str) -> bool {
        unreachable!("the ingredient list asks the World about presence and nothing else")
    }
}

/// One case as the file spells it, with everything a failure has to print.
struct Case {
    line: usize,
    section: String,
    kind: ListKind,
    category: String,
    setting: String,
    /// The stored value's BYTES, which is what a setting holds and what the
    /// language is handed.
    input: Vec<u8>,
    /// The expected canonical rendering, or `None` for a refusal.
    ok: Option<String>,
    refusal: Option<String>,
}

/// What every refusal this language builds begins with. A later caller strips
/// it to compose a log line, so the property has to be TOTAL over the corpus
/// rather than true of the messages somebody remembered.
const REFUSAL_PREFIX: &str = "fkrecipes: ";

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/ingredient-list/cases.txt")
}

/// The BYTES between the FIRST and the LAST bar, verbatim: the corpus says so,
/// which is how a case can carry leading and trailing spaces and how an empty
/// input is written.
fn between_bars(line: &str, no: usize) -> Vec<u8> {
    let first = line
        .find('|')
        .unwrap_or_else(|| panic!("line {} has no opening bar: {}", no, line));
    let last = line
        .rfind('|')
        .unwrap_or_else(|| panic!("line {} has no closing bar: {}", no, line));
    assert!(last > first, "line {} has only one bar: {}", no, line);
    unescape(&line[first + 1..last], no)
}

/// An EXPECTATION is text, and an `in:` line is not. A case's input is a
/// stored setting's bytes, which is exactly what may not be text; what comes
/// back out is a canonical rendering or a refusal, both of which this half
/// builds as a `String`. So a corpus line that expects something no half could
/// produce is a mistake in the FILE, and it is caught here rather than being
/// compared lossily against a message and passing for the wrong reason.
fn expectation(line: &str, no: usize) -> String {
    let bytes = between_bars(line, no);
    String::from_utf8(bytes).unwrap_or_else(|_| {
        panic!(
            "line {}: an ok or refuse line whose bytes are not UTF-8; every message a half can produce is text",
            no
        )
    })
}

/// The corpus's escapes, applied to `in:`, `ok:` and `refuse:` alike.
///
/// IT BUILDS RAW BYTES AND CONVERTS NOTHING. `\xNN` is one raw byte, so a case
/// can hold a stored value that is not valid UTF-8 at all, and what the library
/// gets is that value: fkdata hands a guest the engine's own bytes, so a
/// reader that decoded them first would be answering the not-text question on
/// the library's behalf and every such case would pass for the wrong reason.
/// The `\xff\xfe` case is the whole point of that: it now reaches the parser's
/// own `from_utf8` rather than this reader's conversion.
fn unescape(text: &str, no: usize) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::with_capacity(text.len());
    let mut cs = text.chars();
    while let Some(c) = cs.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        let what = cs
            .next()
            .unwrap_or_else(|| panic!("line {}: a backslash at the end of the text", no));
        match what {
            't' => bytes.push(b'\t'),
            'r' => bytes.push(b'\r'),
            'n' => bytes.push(b'\n'),
            '\\' => bytes.push(b'\\'),
            'x' => bytes.push(hex(&mut cs, 2, no) as u8),
            'u' | 'U' => {
                let digits = if what == 'u' { 4 } else { 8 };
                let point = hex(&mut cs, digits, no);
                let c = char::from_u32(point)
                    .unwrap_or_else(|| panic!("line {}: U+{:04X} is not a code point", no, point));
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            other => panic!("line {}: \\{} is not one of the corpus escapes", no, other),
        }
    }
    bytes
}

fn hex(cs: &mut std::str::Chars, digits: usize, no: usize) -> u32 {
    let mut value = 0u32;
    for _ in 0..digits {
        let c = cs
            .next()
            .unwrap_or_else(|| panic!("line {}: an escape with too few hex digits", no));
        let d = c
            .to_digit(16)
            .unwrap_or_else(|| panic!("line {}: {:?} is not a hex digit", no, c));
        value = value * 16 + d;
    }
    value
}

/// Reads the header's three name lists and every case.
///
/// The header is prose with three marked lines in it, so the reader takes a
/// marker to start a list and the continuation lines that follow it, and
/// stops at the blank comment line that ends the block. Every name it
/// collects is then held to the engine's prototype charset, which is what
/// catches a reader that has started swallowing the prose underneath.
fn read_corpus() -> (CorpusWorld, Vec<Case>) {
    let path = corpus_path();
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the corpus is the contract and it is not readable at {}: {}",
            path.display(),
            e
        )
    });

    let mut items: Vec<String> = Vec::new();
    let mut fluids: Vec<String> = Vec::new();
    let mut tools: Vec<String> = Vec::new();
    let mut collecting: Option<u8> = None;

    let mut cases: Vec<Case> = Vec::new();
    let mut section = String::new();
    let mut kind = ListKind::Recipe;
    let mut category = String::new();
    let mut setting = String::new();
    let mut pending: Option<(usize, Vec<u8>)> = None;

    for (i, raw) in text.lines().enumerate() {
        let no = i + 1;
        let line = raw.trim_end_matches('\r');

        if let Some(comment) = line.strip_prefix('#') {
            let body = comment.trim();
            let marked = |needle: &str, which: u8| -> Option<(u8, String)> {
                body.find(needle)
                    .map(|at| (which, String::from(&body[at + needle.len()..])))
            };
            let found = marked("(ItemExists):", 0)
                .or_else(|| marked("(FluidExists):", 1))
                .or_else(|| marked("(ToolExists):", 2));
            match found {
                Some((which, tail)) => {
                    collecting = Some(which);
                    push_names(&mut items, &mut fluids, &mut tools, which, &tail);
                }
                None => match collecting {
                    // The blank comment line under the fixture block ends it.
                    Some(_) if body.is_empty() => collecting = None,
                    Some(which) => {
                        push_names(&mut items, &mut fluids, &mut tools, which, body);
                    }
                    None => {}
                },
            }
            continue;
        }
        collecting = None;
        if line.trim().is_empty() {
            continue;
        }

        if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let words: Vec<&str> = inner.split_whitespace().collect();
            match words.as_slice() {
                ["recipe", cat, name] => {
                    kind = ListKind::Recipe;
                    category = String::from(*cat);
                    setting = String::from(*name);
                }
                ["packs", name] => {
                    kind = ListKind::Packs;
                    category = String::new();
                    setting = String::from(*name);
                }
                _ => panic!("line {}: a section is [recipe <category> <setting>] or [packs <setting>], got {}", no, line),
            }
            section = String::from(line);
            continue;
        }

        if line.starts_with("in:") {
            assert!(
                pending.is_none(),
                "line {}: an input with no ok or refuse under it",
                no
            );
            pending = Some((no, between_bars(line, no)));
            continue;
        }
        let (want_ok, body) = if line.starts_with("ok:") {
            (true, expectation(line, no))
        } else if line.starts_with("refuse:") {
            (false, expectation(line, no))
        } else {
            panic!(
                "line {}: not a section, an input or an expectation: {}",
                no, line
            );
        };
        let (at, input) = pending
            .take()
            .unwrap_or_else(|| panic!("line {}: an expectation with no input above it", no));
        assert!(
            !section.is_empty(),
            "line {}: a case before any section line",
            no
        );
        cases.push(Case {
            line: at,
            section: section.clone(),
            kind,
            category: category.clone(),
            setting: setting.clone(),
            input,
            ok: if want_ok { Some(body.clone()) } else { None },
            refusal: if want_ok { None } else { Some(body) },
        });
    }
    assert!(
        pending.is_none(),
        "the corpus ends with an input that has no expectation"
    );

    // Anti-vacuity, both halves of it: a reader that found no cases, or one
    // that has started reading the prose as names, must fail rather than pass
    // over nothing. The floor tracks the corpus the review left behind; it is
    // raised when the corpus grows, never lowered to fit a reader.
    assert!(
        cases.len() >= 242,
        "only {} cases were read from {}; the reader is not reaching them",
        cases.len(),
        corpus_path().display()
    );
    for name in items.iter().chain(fluids.iter()).chain(tools.iter()) {
        assert!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "the header reader collected {:?}, which is not a prototype name",
            name
        );
    }
    for (what, names, need) in [
        ("items", &items, "iron-plate"),
        ("fluids", &fluids, "water"),
        ("tools", &tools, "automation-science-pack"),
    ] {
        assert!(
            names.iter().any(|n| n == need),
            "the header's {} do not include {}; the fixture is not the one the corpus states",
            what,
            need
        );
    }
    // The awkward half of the fixture, named here so a header quietly trimmed
    // back to easy names cannot pass: each of these is a name that reads as
    // something else, and they are what give the render rule teeth.
    for need in [
        "default",
        "none",
        "42",
        "x",
        "X2",
        "2x4",
        "loader-1x1",
        "7",
        "Default",
        "None",
    ] {
        assert!(
            items.iter().any(|n| n == need),
            "the header's items do not include {}, which the render rule turns on",
            need
        );
    }
    // The fluid half of the same trap: a FLUID whose name reads as an amount
    // is pointed at the fluid tag, and a fixture without one cannot tell the
    // two hints apart.
    assert!(
        fluids.iter().any(|n| n == "2x2"),
        "the header's fluids do not include 2x2, which the tag hint turns on"
    );

    (
        CorpusWorld {
            items,
            fluids,
            tools,
        },
        cases,
    )
}

fn push_names(
    items: &mut Vec<String>,
    fluids: &mut Vec<String>,
    tools: &mut Vec<String>,
    which: u8,
    tail: &str,
) {
    let into = match which {
        0 => items,
        1 => fluids,
        _ => tools,
    };
    for name in tail.split_whitespace() {
        into.push(String::from(name));
    }
}

/// THE ESCAPES THEMSELVES, before any case rests on them. A reader that
/// dropped `\xff` on the floor, or decoded it before the library saw it, would
/// turn the not-text cases into ordinary text and they would pass for the
/// wrong reason.
#[test]
fn the_corpus_escapes_build_raw_bytes() {
    for (written, want) in [
        ("plain", b"plain".to_vec()),
        ("a\\tb", b"a\tb".to_vec()),
        ("a\\r\\nb", b"a\r\nb".to_vec()),
        ("a\\\\b", b"a\\b".to_vec()),
        ("\\x41", b"A".to_vec()),
        // A code point escape is its UTF-8 encoding, one byte per byte.
        ("\\u00D7", vec![0xc3, 0x97]),
        ("\\U0001F642", vec![0xf0, 0x9f, 0x99, 0x82]),
        // One raw byte that is not UTF-8 stays that one byte: nothing between
        // the file and the library rewrites it, which is what fkdata promises
        // between the engine and the library.
        ("a\\xffb", vec![b'a', 0xff, b'b']),
        // Two bytes of a truncated sequence, which is the shape a corpus case
        // uses.
        ("\\xff\\xfe", vec![0xff, 0xfe]),
    ] {
        assert_eq!(unescape(written, 0), want, "escaping {:?}", written);
    }

    // A CODE POINT ESCAPE NAMES A SCALAR VALUE, and the corpus header says so:
    // a surrogate or a point above U+10FFFF is a mistake in the FILE, and it is
    // a failure here rather than a substitution because a reader that quietly
    // wrote U+FFFD for one would make the case about a character nobody wrote,
    // and the bytes it was written for would never reach the parser at all. A
    // case that needs those bytes writes them with \x.
    for written in ["\\uD800", "\\U00110000"] {
        let got = panic::catch_unwind(|| unescape(written, 0));
        assert!(
            got.is_err(),
            "the reader accepted {}, which is not a scalar value",
            written
        );
    }

    // AND AN EXPECTATION IS TEXT, which is a separate rule about a separate
    // kind of line. An `in:` line carries a stored setting's bytes and need not
    // be text; an `ok:` or a `refuse:` line names something a half must
    // PRODUCE, and every message either half can produce is built as a String.
    // So an expectation whose bytes are not UTF-8 is a mistake in the FILE too,
    // and `expectation` fails on it rather than folding it to U+FFFD and
    // comparing that against a message.
    let got = panic::catch_unwind(|| expectation("refuse:|\\xff|", 0));
    assert!(
        got.is_err(),
        "the reader accepted an expectation whose bytes are not text"
    );
}

/// EVERY CASE, byte for byte. A mismatch prints the section, the input, what
/// the corpus says and what this half said, so the failure is readable
/// without opening the file.
#[test]
fn the_corpus_is_the_contract() {
    let (world, cases) = read_corpus();
    let mut failures: Vec<String> = Vec::new();
    for c in &cases {
        let got = parse(&c.input, c.kind, &c.category, &c.setting, &world);
        match (&c.ok, &c.refusal, got) {
            (Some(want), _, Ok(list)) => {
                let rendered = render(&list);
                if &rendered != want {
                    failures.push(report(c, want, &rendered));
                }
            }
            (Some(want), _, Err(got)) => failures.push(report(
                c,
                &format!("ok: {}", want),
                &format!("refuse: {}", got),
            )),
            (_, Some(want), Err(got)) => {
                if &got != want {
                    failures.push(report(c, want, &got));
                }
                // EVERY REFUSAL BEGINS WITH THE LIBRARY'S OWN PREFIX, over the
                // whole corpus rather than over the cases somebody thought to
                // check. A caller that strips it to build a log line needs the
                // property to be total, and a message written without it would
                // otherwise only show up wherever that caller is exercised.
                for (whose, text) in [("the corpus", want.as_str()), ("this half", got.as_str())] {
                    if !text.starts_with(REFUSAL_PREFIX) {
                        failures.push(report(
                            c,
                            &format!("{} to begin with {:?}", whose, REFUSAL_PREFIX),
                            text,
                        ));
                    }
                }
            }
            (_, Some(want), Ok(list)) => failures.push(report(
                c,
                &format!("refuse: {}", want),
                &format!("ok: {}", render(&list)),
            )),
            (None, None, _) => unreachable!("a case is either an ok or a refusal"),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} corpus cases disagree with this half:\n\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
}

fn report(c: &Case, want: &str, got: &str) -> String {
    format!(
        "{} case at line {}\n  in:   |{}|\n  want: |{}|\n  got:  |{}|",
        c.section,
        c.line,
        // FOR THE REPORT ONLY, AND NOT LOSSILY. An input's bytes need not be
        // text, and this is the one place they are rendered rather than
        // compared. A lossy reading prints a raw-byte case and a case that
        // really holds U+FFFD as replacement characters either way, with
        // nothing in the line saying which of the two the FILE wrote:
        // measured, the corpus case `2 iron-\xff\xfeplate` and the case
        // `2 iron-\uFFFDplate` came out as the same glyph repeated a
        // different number of times, which nobody reads as a difference in
        // kind. Escaped they are `2 iron-\xff\xfeplate` and
        // `2 iron-\xef\xbf\xbdplate`. The escaping is the transcript's, so a
        // byte string reads the same way everywhere in this suite.
        escape_bytes(&c.input),
        want,
        got
    )
}

/// THE CANONICAL FORM IS A FIXED POINT. Every "ok" expectation in the corpus,
/// fed back in, parses and renders to itself. That is the property every
/// rendering rests on: a list written out anywhere, in a log line or a
/// setting's description, must come back through the same parser as the same
/// list.
#[test]
fn every_canonical_rendering_parses_to_itself() {
    let (world, cases) = read_corpus();
    let mut checked = 0usize;
    for c in &cases {
        let want = match &c.ok {
            Some(w) => w,
            None => continue,
        };
        checked += 1;
        let list = match parse(want.as_bytes(), c.kind, &c.category, &c.setting, &world) {
            Ok(list) => list,
            Err(got) => panic!(
                "{} line {}: the canonical rendering |{}| is refused: {}",
                c.section, c.line, want, got
            ),
        };
        let again = render(&list);
        assert_eq!(
            &again, want,
            "{} line {}: the canonical rendering is not a fixed point",
            c.section, c.line
        );
    }
    assert!(
        checked >= 30,
        "only {} canonical renderings were checked; the corpus reader is not finding them",
        checked
    );
}

/// THE INVISIBLE SET AND THE WHITESPACE SET, EVERY MEMBER OF BOTH, which the
/// corpus can only carry one member of per range.
///
/// ONE POLICY: a character the player cannot see is refused by its code point
/// wherever it sits, and nothing is deleted from the text on their behalf. The
/// whitespace set is the exception and it is asked FIRST, which is why the six
/// members the two sets share (tab, LF, CR, U+2007, U+202F and U+3000)
/// separate words rather than refuse.
///
/// THE SET IS SPELLED HERE RATHER THAN READ OUT OF THE PREDICATE, and the Go
/// twin now carries the same table for the same reason. A walk that asks
/// `is_invisible` which points to test is a test that asks its own subject
/// what to ask about: a range NARROWED in the predicate simply stops being
/// tested. PROVED on the Go side, whose walk was shaped that way: narrowing
/// 0x115f..0x1160 to 0x115f and 0xfe00..0xfe0f to 0xfe0f dropped 15 code
/// points with the whole Go suite still green. This table is an INDEPENDENT
/// spelling of the same set.
///
/// ASKED THROUGH THE PARSER, because the predicate itself is out of reach and
/// on purpose: `is_invisible` is a private `fn` of the language module, and a
/// `pub(crate)` in front of it is exactly the widening
/// `nothing_reachable_in_the_language_module_is_unguarded` exists to refuse.
/// The parser is the stronger question anyway, since it pins the message as
/// well as the answer: every member of the table is put to it, and so is the
/// code point either side of every range, which must come back quoted as
/// itself. A range narrowed, widened or dropped in `ingredient_list.rs` fails
/// here by name.
///
/// The corpus pins one member of every range in both languages, and the Go
/// twin walks its own copy of this table member by member. Between the three,
/// every member of the set is witnessed in BOTH halves and no half's witness
/// is the predicate it is testing.
#[test]
fn every_invisible_character_is_refused_and_the_whitespace_set_separates() {
    let world = CorpusWorld {
        items: names(&["iron-plate"]),
        fluids: names(&["water"]),
        tools: names(&["automation-science-pack"]),
    };
    let read = |text: &str| {
        parse(
            text.as_bytes(),
            ListKind::Recipe,
            "crafting",
            "mymod-parts",
            &world,
        )
    };

    // The whitespace set, in the order the language's own constant spells it.
    const WHITESPACE: &[u32] = &[
        0x0009, 0x000a, 0x000d, 0x0020, 0x00a0, 0x2007, 0x202f, 0x3000,
    ];
    // The invisible set, range by range, low to high. Derived rather than
    // recalled; the predicate's own doc says from what and how.
    const INVISIBLE: &[(u32, u32)] = &[
        (0x0000, 0x001f),
        (0x007f, 0x009f),
        (0x00ad, 0x00ad),
        (0x034f, 0x034f),
        (0x0600, 0x0605),
        (0x061c, 0x061c),
        (0x06dd, 0x06dd),
        (0x070f, 0x070f),
        (0x0890, 0x0891),
        (0x08e2, 0x08e2),
        (0x115f, 0x1160),
        (0x17b4, 0x17b5),
        (0x180b, 0x180f),
        (0x2000, 0x200f),
        (0x2028, 0x202f),
        (0x205f, 0x206f),
        (0x3000, 0x3000),
        (0x3164, 0x3164),
        (0xfe00, 0xfe0f),
        (0xfeff, 0xfeff),
        (0xffa0, 0xffa0),
        (0xfff0, 0xfffb),
        (0x110bd, 0x110bd),
        (0x110cd, 0x110cd),
        (0x13430, 0x1343f),
        (0x1bca0, 0x1bca3),
        (0x1d173, 0x1d17a),
        (0xe0000, 0xe0fff),
    ];

    let mut members = 0usize;
    let mut refused = 0usize;
    for (low, high) in INVISIBLE {
        for point in *low..=*high {
            let c = char::from_u32(point).expect(
                "the table covers a point that is not a scalar value and can be in no text",
            );
            members += 1;
            // The six the whitespace set takes first are members of this set
            // and separate words anyway; the loop below is where they are put
            // to the parser.
            if WHITESPACE.contains(&point) {
                continue;
            }
            refused += 1;
            let token = alloc::format!("U+{:04X}", point);
            assert_eq!(
                read(&alloc::format!("2 iron{}-plate", c)).expect_err("an invisible character was accepted"),
                alloc::format!(
                    "fkrecipes: mymod-parts, entry 1 (\"2 iron{}-plate\"): an invisible character ({}) has no place here; retype the entry rather than pasting it",
                    token, token
                ),
                "{} inside a name",
                token
            );
        }
    }
    // EXACTLY, not a floor: a floor a deleted range still clears is a floor
    // that cannot notice. The tag block alone is 4096 of the 4287, and six of
    // them are whitespace first. The Go twin asserts the same two numbers.
    assert_eq!(
        (members, refused),
        (4287, 4281),
        "the table walked {} members and put {} of them to the parser; the set is 4287 and 4281, and a table that no longer says so is a set somebody changed on one side",
        members,
        refused
    );

    // THE CODE POINT EITHER SIDE OF EVERY RANGE is quoted as itself, which is
    // the half of the boundary a list of members cannot state: a range widened
    // by one would otherwise pass here.
    let mut neighbours = 0usize;
    for (low, high) in INVISIBLE {
        for point in [low.checked_sub(1), high.checked_add(1)]
            .into_iter()
            .flatten()
        {
            let c = match char::from_u32(point) {
                Some(c) => c,
                // A boundary that lands on a surrogate is not a scalar value
                // and no text can carry it.
                None => continue,
            };
            if WHITESPACE.contains(&point)
                || INVISIBLE.iter().any(|(l, h)| (*l..=*h).contains(&point))
            {
                continue;
            }
            neighbours += 1;
            assert_eq!(
                read(&alloc::format!("2 iron{}-plate", c)).expect_err("a strange character was accepted"),
                alloc::format!(
                    "fkrecipes: mymod-parts, entry 1 (\"2 iron{}-plate\"): \"{}\" has no place here; names use the letters a to z, digits, - and _, and an amount is plain digits, as in \"2 iron-plate\"",
                    c, c
                ),
                "U+{:04X} sits just outside the set and must be quoted as itself",
                point
            );
        }
    }
    assert_eq!(
        neighbours, 53,
        "{} neighbours were exercised, not the 53 the table has; a boundary went unasked",
        neighbours
    );

    // Every member of the whitespace set separates, at the edges and inside.
    for point in WHITESPACE {
        let c = char::from_u32(*point).expect("the whitespace set holds scalar values only");
        let text = alloc::format!("{}2{}iron-plate{}", c, c, c);
        match read(&text) {
            Ok(list) => assert_eq!(
                render(&list),
                "2 iron-plate",
                "U+{:04X} as whitespace",
                point
            ),
            Err(got) => panic!("U+{:04X} as whitespace was refused: {}", point, got),
        }
    }

    // THE FOUR THAT USED TO BE DELETED are members like any other, and this is
    // the row that says so: a BOM, a zero-width space and the two joiners are
    // answered by their code points rather than silently removed from a text
    // nobody typed that way.
    //
    // PUT TO THE PARSER, not to the table above. An earlier shape asked
    // whether INVISIBLE contained them, which is a table asserted against four
    // literals in the same file: it would have stayed green with the predicate
    // stripping all four again.
    for point in [0xfeffu32, 0x200b, 0x200c, 0x200d] {
        let c = char::from_u32(point).expect("the four are scalar values");
        let token = alloc::format!("U+{:04X}", point);
        assert_eq!(
            read(&alloc::format!("2 iron{}-plate", c)).expect_err("one of the four was accepted"),
            alloc::format!(
                "fkrecipes: mymod-parts, entry 1 (\"2 iron{}-plate\"): an invisible character ({}) has no place here; retype the entry rather than pasting it",
                token, token
            ),
            "{} is not refused by code point",
            token
        );
    }
}

/// A deterministic generator, seeded by a constant: a property that fails
/// only on some runs is a property nobody can bisect. xorshift64 star, four
/// lines, no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// RENDER THEN PARSE IS AN IDENTITY, over generated lists rather than over
/// the handful a corpus can hold.
///
/// The names are chosen to hit the awkward half of the rule, and the review
/// widened it: an item called `42`, `x`, `X2` or `2x4` can only be written in
/// its tag, `loader-1x1` is a NAME and must not be tagged, `none` and
/// `default` are the two reserved words, and `both` is an item AND a fluid,
/// so the tie rule and the fluid tag both have to survive the round trip. The
/// fluid amounts mix the values the corpus pins with random doubles taken
/// from raw bits, capped at the measured ceiling, which is where a renderer
/// that reached for exponent form or dropped a digit would show up.
#[test]
fn rendering_then_parsing_is_an_identity() {
    let items = [
        "iron-plate",
        "copper-cable",
        "steel-plate",
        "42",
        "x",
        "X",
        "X2",
        "2x4",
        "loader-1x1",
        "none",
        "default",
        // THE RESERVED WORDS ARE MATCHED WITHOUT ASCII CASE, so an item called
        // Default takes its tag exactly as one called default does, and one
        // called defaults is a name and stays plain: the fold is an equality
        // and not a prefix.
        "None",
        "Default",
        "defaults",
        "2x",
        "1e3",
        "both",
    ];
    let fluids = ["water", "steam", "both", "42"];
    let world = CorpusWorld {
        items: names(&items),
        fluids: names(&fluids),
        tools: names(&["automation-science-pack"]),
    };
    let pinned = [
        0.1_f64,
        2.5,
        1e-7,
        1e21,
        0.5,
        1.0,
        65535.0,
        1000000000.0,
        1e-3,
        3.0,
        5e-324,
        1e301,
    ];

    let mut rng = Rng(0x5eed_1eaf_c0ff_ee01);
    let mut rounds = 0usize;
    // The two lists that are words rather than entries: the empty list reads
    // back as nothing, and the marker reads back as itself without asking the
    // World anything at all.
    let empty = ListText::List(IngredientList {
        entries: Vec::new(),
    });
    assert_eq!(render(&empty), "none");
    assert_eq!(
        parse(b"none", ListKind::Recipe, "crafting", "mymod-parts", &world),
        Ok(empty)
    );
    assert_eq!(render(&ListText::Default), "default");
    assert_eq!(
        parse(
            b"default",
            ListKind::Recipe,
            "crafting",
            "mymod-parts",
            &world
        ),
        Ok(ListText::Default)
    );

    for _ in 0..2000 {
        let mut entries = Vec::new();
        let mut taken: Vec<(bool, &str)> = Vec::new();
        let want = 1 + rng.below(5);
        while entries.len() < want {
            let fluid = rng.below(3) > 0;
            let name = if fluid {
                fluids[rng.below(fluids.len())]
            } else {
                items[rng.below(items.len())]
            };
            if taken.contains(&(fluid, name)) {
                continue;
            }
            taken.push((fluid, name));
            let amount = if fluid {
                let v = if rng.below(2) == 0 {
                    pinned[rng.below(pinned.len())]
                } else {
                    random_amount(&mut rng)
                };
                Amount::Fluid(v)
            } else {
                Amount::Item(1 + rng.below(65535) as i64)
            };
            entries.push(crate::ingredient_list::ListEntry {
                name: String::from(name),
                amount,
            });
        }
        let list = ListText::List(IngredientList { entries });
        let text = render(&list);
        let back = parse(
            text.as_bytes(),
            ListKind::Recipe,
            "crafting-with-fluid",
            "mymod-parts",
            &world,
        );
        match back {
            Ok(again) => {
                assert_eq!(again, list, "rendered as |{}|", text);
                assert_eq!(render(&again), text, "the second rendering differs");
            }
            Err(got) => panic!("the rendering |{}| does not parse: {}", text, got),
        }
        rounds += 1;
    }
    assert_eq!(rounds, 2000);
}

/// A positive finite double at or below the measured fluid ceiling:
/// denormals, huge magnitudes and everything between, which is a wider net
/// than any table.
fn random_amount(rng: &mut Rng) -> f64 {
    loop {
        let v = f64::from_bits(rng.next() & 0x7fff_ffff_ffff_ffff);
        if v.is_finite() && v > 0.0 && v <= 1e301 {
            return v;
        }
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| String::from(*s)).collect()
}

/// THE DECIMAL RULE, over the values that break a shortest-round-trip
/// printer, and it is the SAME table the Go half runs.
///
/// The first two rows are exact ties at the seventeenth digit, where Go's
/// printer and Rust's Display disagree about the last digit; the corpus pins
/// them as sentences and this pins them as numbers. The rest are the shapes
/// the layout has to get right: a fraction, an integer with the zeros the
/// exponent asks for, the smallest denormal, an integer past 2^53, and the
/// largest double there is, where the even-digit tie rule chooses 158 over
/// the 157 both platforms print.
///
/// EVERY EXPECTATION IS ALSO ASSERTED TO READ BACK to the value it renders,
/// so a table row cannot be wrong in the same direction as the code.
//
// THE LITERALS ARE THE DOUBLES' OWN EXACT DECIMALS, which is what makes the
// two tie rows ties at all: clippy reads 1.00000762939453125 as excessive
// precision and offers to truncate it to 1.0000076293945313, which is Rust's
// shortest printer breaking exactly the tie this rule exists to settle. The
// value would not change (both spellings name the same double) but the row
// would stop saying what it is for. Every row is held to
// `want.parse() == v` below, so the literals cannot drift unnoticed.
#[allow(clippy::excessive_precision)]
#[test]
fn the_decimal_rule_writes_one_decimal_for_both_halves() {
    let cases: &[(f64, String)] = &[
        (0.1, String::from("0.1")),
        (0.25, String::from("0.25")),
        (0.5, String::from("0.5")),
        (2.5, String::from("2.5")),
        (1.0, String::from("1")),
        (3.0, String::from("3")),
        (1e-7, String::from("0.0000001")),
        (1e21, digits_then_zeros("1", 21)),
        (1e301, digits_then_zeros("1", 301)),
        (5e-324, zeros_then_digits(323, "4")),
        // 2^53 + 1 is not a double; the one it becomes is even, and 17 digits
        // is the first precision that reads back to it.
        (9007199254740993.0, String::from("9007199254740992")),
        (
            1.7976931348623157e308,
            digits_then_zeros("17976931348623158", 292),
        ),
        (1.00000762939453125, String::from("1.0000076293945312")),
        (1059438285926254.25, String::from("1059438285926254.2")),
    ];
    for (v, want) in cases {
        assert_eq!(
            want.parse::<f64>(),
            Ok(*v),
            "the expectation {} does not read back to the value it is written for",
            want
        );
        assert_eq!(&format_amount(*v), want, "rendering {:?}", v);
        // The rendering is a fixed point of itself, which is what the log
        // line and the setting description rest on.
        assert_eq!(
            &format_amount(want.parse::<f64>().expect("reads back")),
            want
        );
    }
}

/// Digits with trailing zeros behind them, which is how an expectation 300
/// characters long stays readable.
fn digits_then_zeros(digits: &str, zeros: usize) -> String {
    let mut out = String::from(digits);
    for _ in 0..zeros {
        out.push('0');
    }
    out
}

/// A zero, a dot, leading zeros and the digits.
fn zeros_then_digits(zeros: usize, digits: &str) -> String {
    let mut out = String::from("0.");
    for _ in 0..zeros {
        out.push('0');
    }
    out.push_str(digits);
    out
}

/// THE CATEGORY BEATS THE CEILING, and the order is the same in the player's
/// path and in the author's: a fluid a crafting recipe cannot take at all is
/// the larger mistake, whatever the amount is. The author's half of this pair
/// is plan_data_refuses_a_declared_fluid_a_recipe_cannot_take.
#[test]
fn a_crafting_recipe_hears_about_the_category_before_the_ceiling() {
    let world = CorpusWorld {
        items: names(&["iron-plate"]),
        fluids: names(&["water"]),
        tools: names(&["automation-science-pack"]),
    };
    // 1e302, written the way this language spells a number: the exponent form
    // is refused as a shape before anything looks at how large it is.
    let entry = format!("{} water", digits_then_zeros("1", 302));
    let refused = |category: &str| match parse(
        entry.as_bytes(),
        ListKind::Recipe,
        category,
        "mymod-parts",
        &world,
    ) {
        Ok(list) => panic!("the entry was accepted as {}", render(&list)),
        Err(got) => got,
    };
    assert_eq!(
        refused("crafting"),
        format!(
            "fkrecipes: mymod-parts, entry 1 (\"{}\"): water is a fluid, and a recipe in the crafting category takes items only",
            entry
        )
    );
    assert_eq!(
        refused(""),
        format!(
            "fkrecipes: mymod-parts, entry 1 (\"{}\"): water is a fluid, and a recipe in the crafting category takes items only",
            entry
        ),
        "an empty category IS crafting"
    );
    assert_eq!(
        refused("chemistry"),
        format!(
            "fkrecipes: mymod-parts, entry 1 (\"{}\"): the amount is too large; fluid amounts go up to 1e301",
            entry
        )
    );
}

/// A consumer's fixture, written before the World grew the two questions this
/// round adds: it implements what the trait REQUIRES and nothing more, which
/// is exactly the shape that used to stop compiling the day a method landed.
struct BareWorld;

impl Named for BareWorld {
    fn mod_name(&self) -> String {
        String::from("bare")
    }
}

impl World for BareWorld {
    fn startup_setting(&self, _name: &str) -> Option<Value> {
        None
    }
    fn tech_names(&self) -> Vec<String> {
        Vec::new()
    }
    fn tech_prereqs(&self, _name: &str) -> Vec<String> {
        Vec::new()
    }
    fn tech_unit(&self, _name: &str) -> Option<Value> {
        None
    }
    fn tech_max_level(&self, _name: &str) -> Option<Value> {
        None
    }
    fn tech_has_research_trigger(&self, _name: &str) -> bool {
        false
    }
    fn tech_exists(&self, _name: &str) -> bool {
        false
    }
    fn item_exists(&self, _name: &str) -> bool {
        false
    }
    fn entity_exists(&self, _name: &str) -> bool {
        false
    }
    fn recipe_exists(&self, _name: &str) -> bool {
        false
    }
}

/// The default's panic NAMES THE METHOD. A fixture that reaches a question it
/// never answered gets told which one to write, rather than a wrong answer or
/// a trait it can no longer implement.
#[test]
fn the_world_defaults_name_the_method_a_fixture_owes() {
    for (what, got) in [
        (
            "fkrecipes: World::fluid_exists is not implemented by this fixture",
            panic::catch_unwind(|| BareWorld.fluid_exists("water")),
        ),
        (
            "fkrecipes: World::tool_exists is not implemented by this fixture",
            panic::catch_unwind(|| BareWorld.tool_exists("automation-science-pack")),
        ),
    ] {
        let payload = got.expect_err("the default answered instead of panicking");
        let said = payload
            .downcast_ref::<&str>()
            .map(|s| String::from(*s))
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .expect("the panic carried no message");
        assert_eq!(said, what);
    }
}

/// THE TWO WHOLE-TEXT GUARDS, which a corpus case can only spell one way and
/// which have to hold at the boundary rather than at the one length and the
/// one byte sequence the file happens to carry.
///
/// Both are UNREACHABLE THROUGH THE SETTINGS SCREEN (measured: the engine
/// resets a stored value that is not a string, and the field itself refuses
/// nothing this long), which is exactly why they are tested here: they exist
/// for a mod-settings.dat somebody edited by hand, and nobody will find them
/// by playing.
#[test]
fn the_whole_text_guards_hold_at_their_boundaries() {
    let world = CorpusWorld {
        items: names(&["iron-plate"]),
        fluids: names(&["water"]),
        tools: names(&["automation-science-pack"]),
    };
    let read = |text: &[u8]| parse(text, ListKind::Recipe, "crafting", "mymod-parts", &world);

    // THE LENGTH IS COUNTED IN SCALARS, not in bytes: a list of 1000 two-byte
    // names is 2000 characters and is read, and one character more is not.
    // A byte count would refuse the first, which is a text a player could
    // plausibly have.
    let entry = "1 iron-plate, ";
    let mut long = String::new();
    while long.chars().count() + entry.chars().count() <= 2000 {
        long.push_str(entry);
    }
    long.push_str("1 iron-plate");
    assert_eq!(long.chars().count(), 2000);
    let at_the_limit =
        read(long.as_bytes()).expect_err("2000 characters of duplicates are read and refused");
    assert_eq!(
        at_the_limit, "fkrecipes: mymod-parts: entries 1 and 2 both name iron-plate",
        "a 2000-character text is READ; the length guard must not fire on it"
    );

    for (what, text) in [
        ("one character over", format!("{}x", long)),
        ("far over", "x".repeat(98_000)),
    ] {
        assert_eq!(
            read(text.as_bytes()).expect_err("the length guard did not fire"),
            "fkrecipes: mymod-parts is longer than 2000 characters; that is not an ingredient list",
            "{}",
            what
        );
    }

    // NOT TEXT, AND IT IS THE BYTES THAT SAY SO. fkdata hands both halves the
    // stored value unchanged, so what a hand-edited file's invalid sequence
    // reaches here as is the sequence itself: this half asks
    // core::str::from_utf8 where the Go half asks utf8.ValidString, and the
    // corpus is where the two are held to the same answer. The corpus pins a
    // bad sequence inside a name; these are the other positions it can sit in,
    // and the one a corpus case cannot spell at all, which is a value long
    // enough to reach the length guard.
    let mut before_the_length_guard = alloc::vec![b'x'; 98_001];
    before_the_length_guard[0] = 0xff;
    for (what, text) in [
        ("alone", alloc::vec![0xffu8]),
        ("inside a name", b"2 iron-\xffplate".to_vec()),
        ("after a valid list", b"2 iron-plate\xff".to_vec()),
        // A byte that is not a leading byte at all, rather than a truncated
        // sequence: from_utf8 refuses both and the message is one message.
        ("a continuation byte alone", alloc::vec![0x80u8]),
        // The length guard must not get there first: the not-text rule is
        // asked of the whole value before anything else, so a 98,000-byte
        // value that is also not text is refused as text rather than as
        // length.
        ("before the length guard", before_the_length_guard),
    ] {
        assert_eq!(
            read(&text).expect_err("the not-text guard did not fire"),
            "fkrecipes: mymod-parts contains characters that are not text; retype the list",
            "{}",
            what
        );
    }

    // AND A REPLACEMENT CHARACTER IS NOT THAT RULE. U+FFFD is three ordinary
    // UTF-8 bytes, so it is answered by the ordinary character rules, which
    // quote it: which rule answers depends on where it sits, and it is the
    // names rule only where the case below puts it. Measured, in the same
    // fixture: `[item=iron-\u{fffd}plate]` takes the TAG rule instead ("a tag
    // is [item=name] or [fluid=name]"), and `, \u{fffd}` takes the empty-entry
    // rule, because a text whose first problem is elsewhere takes that problem.
    // The corpus pins the names-rule position; this is here beside the not-text
    // cases, because the two used to be one rule and are not.
    assert_eq!(
        read("\u{fffd}".as_bytes()).expect_err("a replacement character was accepted"),
        "fkrecipes: mymod-parts, entry 1 (\"\u{fffd}\"): \"\u{fffd}\" has no place here; names use the letters a to z, digits, - and _, and an amount is plain digits, as in \"2 iron-plate\""
    );
}

/// THE ANSWERS THE CORPUS DOES NOT PIN, and both halves have to give the same
/// one anyway, so they are pinned here instead. Each is a place where two
/// orderings are defensible and only one can be the language.
#[test]
fn the_rules_the_corpus_leaves_to_the_implementation() {
    let world = CorpusWorld {
        items: names(&["iron-plate", "automation-science-pack"]),
        fluids: names(&["water"]),
        tools: names(&["automation-science-pack"]),
    };
    let cases: &[(&str, ListKind, &str, &str)] = &[
        // AN EMPTY ENTRY IS REPORTED WHEREVER IT SITS, ahead of every other
        // question about any entry, including the reserved words. The corpus
        // pins the word in entry 1 with the hole in entry 2; this is the same
        // text the other way round, and it must give the same KIND of answer
        // rather than swapping with the position.
        (
            ", none, 2 iron-plate",
            ListKind::Recipe,
            "mymod-parts",
            "fkrecipes: mymod-parts, entry 1 is empty; one comma separates two ingredients",
        ),
        // The second empty entry is not the one reported: the pass walks in
        // reading order and stops.
        (
            "2 iron-plate,, 3 copper-cable,, 4 steel-plate",
            ListKind::Recipe,
            "mymod-parts",
            "fkrecipes: mymod-parts, entry 2 is empty; one comma separates two ingredients",
        ),
        // TWO RESERVED WORDS TOGETHER are answered about the FIRST one, for
        // the same reason: position decides, so both halves name the same
        // word.
        (
            "none, default",
            ListKind::Recipe,
            "mymod-parts",
            "fkrecipes: mymod-parts: none stands alone; remove the other entries or the word",
        ),
        (
            "default, none",
            ListKind::Recipe,
            "mymod-parts",
            "fkrecipes: mymod-parts: default stands alone; remove the other entries or the word",
        ),
        // A TAGGED FLUID SKIPS THE ITEM QUESTIONS, and it keeps doing so with
        // NO AMOUNT WRITTEN, which is the arm the corpus does not spell: the
        // implicit 1 must not change which namespace answers. The name really
        // is a science pack, and the tag really does say fluid, so both the
        // lookup and the sentence come out of the fluid table.
        (
            "[fluid=automation-science-pack]",
            ListKind::Packs,
            "mymod-packs",
            "fkrecipes: mymod-packs, entry 1 (\"[fluid=automation-science-pack]\"): no fluid is named automation-science-pack",
        ),
        // The same tag over a name that IS a fluid gets the fluid sentence,
        // which is the arm above it.
        (
            "[fluid=water]",
            ListKind::Packs,
            "mymod-packs",
            "fkrecipes: mymod-packs, entry 1 (\"[fluid=water]\"): water is a fluid, and research takes science packs only",
        ),
        // And a tagged ITEM falls through the same three questions a bare
        // name does: this one stops at the item table, and the corpus pins
        // [item=water], which walks past it into the fluid table.
        (
            "[item=iron-plate]",
            ListKind::Packs,
            "mymod-packs",
            "fkrecipes: mymod-packs, entry 1 (\"[item=iron-plate]\"): iron-plate is an item, not a science pack",
        ),
        // A CASED RESERVED WORD IS THE WORD, so it stands alone by the same
        // rule, and the sentence names the word in the one spelling the
        // language has rather than echoing the player's capitals back.
        (
            "NoNe, 2 iron-plate",
            ListKind::Recipe,
            "mymod-parts",
            "fkrecipes: mymod-parts: none stands alone; remove the other entries or the word",
        ),
        (
            "2 iron-plate, DEFAULT",
            ListKind::Recipe,
            "mymod-parts",
            "fkrecipes: mymod-parts: default stands alone; remove the other entries or the word",
        ),
        // A HOMOGLYPH IS NOT THE WORD. The last letter here is a Cyrillic
        // capital Te (U+0422), so the entry reads as DEFAULT on screen and is
        // none of the language's words: the fold is over ASCII letters and a
        // name is what its code points are. The character rule answers, and it
        // quotes the character that is not what it looks like.
        (
            "DEFAUL\u{422}",
            ListKind::Recipe,
            "mymod-parts",
            "fkrecipes: mymod-parts, entry 1 (\"DEFAUL\u{422}\"): \"\u{422}\" has no place here; names use the letters a to z, digits, - and _, and an amount is plain digits, as in \"2 iron-plate\"",
        ),
    ];
    for (input, kind, setting, want) in cases {
        match parse(input.as_bytes(), *kind, "crafting", setting, &world) {
            Ok(list) => panic!("input {:?} was accepted as {}", input, render(&list)),
            Err(got) => assert_eq!(&got, want, "input {:?}", input),
        }
    }
}

/// The two probes are separate namespaces, and the parser asks the right one.
/// A world whose fluid list is empty cannot resolve `water` even though the
/// same name is an item nowhere: the message is about both families because
/// the name carried no tag.
#[test]
fn a_recipe_name_is_looked_up_in_both_families_and_a_pack_name_in_neither() {
    let world = CorpusWorld {
        items: names(&["iron-plate"]),
        fluids: names(&["water"]),
        tools: names(&["automation-science-pack"]),
    };
    let cases: &[(ListKind, &str, &str, Result<&str, &str>)] = &[
        (ListKind::Recipe, "chemistry", "2 water", Ok("2 [fluid=water]")),
        (
            ListKind::Recipe,
            "chemistry",
            "2 automation-science-pack",
            Err("fkrecipes: mymod-parts, entry 1 (\"2 automation-science-pack\"): no item or fluid is named automation-science-pack"),
        ),
        (
            ListKind::Packs,
            "",
            "2 automation-science-pack",
            Ok("2 automation-science-pack"),
        ),
    ];
    for (kind, category, input, want) in cases {
        let setting = if *kind == ListKind::Recipe {
            "mymod-parts"
        } else {
            "mymod-packs"
        };
        let got = parse(input.as_bytes(), *kind, category, setting, &world);
        match (want, got) {
            (Ok(want), Ok(list)) => assert_eq!(&render(&list), want, "input {:?}", input),
            (Err(want), Err(got)) => assert_eq!(&got, want, "input {:?}", input),
            (Ok(want), Err(got)) => {
                panic!("input {:?}: want {:?}, refused with {}", input, want, got)
            }
            (Err(want), Ok(list)) => panic!(
                "input {:?}: want the refusal {:?}, got {}",
                input,
                want,
                render(&list)
            ),
        }
    }
}
