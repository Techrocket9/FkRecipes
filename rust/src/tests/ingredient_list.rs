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
    input: String,
    /// The expected canonical rendering, or `None` for a refusal.
    ok: Option<String>,
    refusal: Option<String>,
}

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/ingredient-list/cases.txt")
}

/// The text between the FIRST and the LAST bar, verbatim: the corpus says so,
/// which is how a case can carry leading and trailing spaces and how an empty
/// input is written.
fn between_bars(line: &str, no: usize) -> String {
    let first = line
        .find('|')
        .unwrap_or_else(|| panic!("line {} has no opening bar: {}", no, line));
    let last = line
        .rfind('|')
        .unwrap_or_else(|| panic!("line {} has no closing bar: {}", no, line));
    assert!(last > first, "line {} has only one bar: {}", no, line);
    unescape(&line[first + 1..last], no)
}

/// The corpus's escapes, applied to `in:`, `ok:` and `refuse:` alike.
///
/// IT BUILDS BYTES AND CONVERTS THE WAY fkdata DOES ON THE WIRE. `\xNN` is one
/// RAW BYTE, so a case can hold a stored value that is not valid UTF-8 at all,
/// and the lossy conversion turns it into U+FFFD exactly as the guest sees it
/// coming out of a hand-edited mod-settings.dat. Doing this any other way
/// would make the not-text rule untestable from the file both halves share.
fn unescape(text: &str, no: usize) -> String {
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
    String::from_utf8_lossy(&bytes).into_owned()
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
    let mut pending: Option<(usize, String)> = None;

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
            (true, between_bars(line, no))
        } else if line.starts_with("refuse:") {
            (false, between_bars(line, no))
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
        cases.len() >= 203,
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
    for need in ["default", "none", "42", "x", "X2", "2x4", "loader-1x1", "7"] {
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
/// dropped `\xff` on the floor would turn the not-text cases into ordinary
/// text and they would pass for the wrong reason.
#[test]
fn the_corpus_escapes_build_bytes_and_decode_lossily() {
    for (written, want) in [
        ("plain", String::from("plain")),
        ("a\\tb", String::from("a\tb")),
        ("a\\r\\nb", String::from("a\r\nb")),
        ("a\\\\b", String::from("a\\b")),
        ("\\x41", String::from("A")),
        ("\\u00D7", String::from("\u{00d7}")),
        ("\\U0001F642", String::from("\u{1f642}")),
        // One raw byte that is not UTF-8 becomes exactly one U+FFFD, which is
        // what fkdata's lossy decode hands the guest.
        ("a\\xffb", String::from("a\u{fffd}b")),
        // Two bytes of a truncated sequence, which is the shape a corpus case
        // uses, and the parser must refuse whatever number of replacement
        // characters it lands on.
        ("\\xff\\xfe", String::from("\u{fffd}\u{fffd}")),
    ] {
        assert_eq!(unescape(written, 0), want, "escaping {:?}", written);
    }

    // A CODE POINT ESCAPE NAMES A SCALAR VALUE, and the corpus header says so:
    // a surrogate or a point above U+10FFFF is a mistake in the FILE, and a
    // reader that quietly substituted U+FFFD would turn such a case into a
    // not-text case that passes for the wrong reason. A case needing those
    // bytes writes them with \x.
    for written in ["\\uD800", "\\U00110000"] {
        let got = panic::catch_unwind(|| unescape(written, 0));
        assert!(
            got.is_err(),
            "the reader accepted {}, which is not a scalar value",
            written
        );
    }
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
        c.section, c.line, c.input, want, got
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
        let list = match parse(want, c.kind, &c.category, &c.setting, &world) {
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
        parse("none", ListKind::Recipe, "crafting", "mymod-parts", &world),
        Ok(empty)
    );
    assert_eq!(render(&ListText::Default), "default");
    assert_eq!(
        parse(
            "default",
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
            &text,
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
    let refused =
        |category: &str| match parse(&entry, ListKind::Recipe, category, "mymod-parts", &world) {
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
    let read = |text: &str| parse(text, ListKind::Recipe, "crafting", "mymod-parts", &world);

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
    let at_the_limit = read(&long).expect_err("2000 characters of duplicates are read and refused");
    assert_eq!(
        at_the_limit, "fkrecipes: mymod-parts: entries 1 and 2 both name iron-plate",
        "a 2000-character text is READ; the length guard must not fire on it"
    );

    for (what, text) in [
        ("one character over", format!("{}x", long)),
        ("far over", "x".repeat(98_000)),
    ] {
        assert_eq!(
            read(&text).expect_err("the length guard did not fire"),
            "fkrecipes: mymod-parts is longer than 2000 characters; that is not an ingredient list",
            "{}",
            what
        );
    }

    // NOT TEXT, AND THE REPLACEMENT CHARACTER IS THE ONLY FORM IT CAN TAKE
    // HERE: a Rust &str is valid UTF-8 by construction, so what a hand-edited
    // file's invalid bytes become on the way in through fkdata's lossy decode
    // is what this half has to recognise. The Go half asks utf8.ValidString
    // as well; the corpus is where the two are held to the same answer.
    for (what, text) in [
        ("alone", String::from("\u{fffd}")),
        ("inside a name", String::from("2 iron-\u{fffd}plate")),
        ("after a valid list", String::from("2 iron-plate\u{fffd}")),
        // The length guard must not get there first: the not-text rule is
        // asked of the whole text before anything else, so a 98,000-character
        // value that also holds a replacement character is refused as text
        // rather than as length.
        (
            "before the length guard",
            format!("\u{fffd}{}", "x".repeat(98_000)),
        ),
    ] {
        assert_eq!(
            read(&text).expect_err("the not-text guard did not fire"),
            "fkrecipes: mymod-parts contains characters that are not text; retype the list",
            "{}",
            what
        );
    }
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
    ];
    for (input, kind, setting, want) in cases {
        match parse(input, *kind, "crafting", setting, &world) {
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
        let got = parse(input, *kind, category, setting, &world);
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
