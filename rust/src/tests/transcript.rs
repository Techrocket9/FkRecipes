use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::op::{Op, PathEl};
use crate::value::{Value, MAX_EXACT_INT};

// The transcript is how a test reads an Op stream: one line per op, values
// rendered in the order the planner built them. The Go mirror renders the
// same lines from the same plan, which is what "the two halves agree" means
// before the packaged mirror harness exists to say it in Lua.

/// The four lines this library composes onto EVERY text setting's
/// description, after the default list: that a list too long for the tooltip
/// is still one list, what to write and how much of it, which field decides
/// while this one says the reserved word, and what a text it cannot use costs.
///
/// THE FORMAT LINE IS TWO SENTENCES ON AN INGREDIENT SETTING AND ONE ON A
/// PACKS SETTING, which is why there is a `PACKS` tail beside every `TEXT`
/// one: the word `none` empties an ingredient list and is REFUSED on a pack
/// list, so naming it there would be telling a player to type a word the
/// library turns down.
///
/// SPELLED OUT HERE RATHER THAN TAKEN FROM THE SOURCE, which is the whole
/// point of a golden: `text_format_line` builds the number from `MAX_TEXT`, so
/// a test that asked it for the sentence would move with any edit to either.
/// These bytes are the contract, the Go twin carries the same ones, and the
/// mirror compares the two transcripts.
///
/// A golden writes [`TEXT_TAIL`] where these belong and `assert_composed`
/// expands it, because a `&[&str]` of raw strings cannot concatenate a
/// constant the way the Go twin's `+` does.
pub(crate) const WANT_TEXT_TAIL: &str = concat!(
    r#", "\nA list too long for one line continues on the next; the continuation is part of the same list.""#,
    r#", "\nWrite internal names, as the default line above does, in at most 2000 characters. The word none empties the list, so the recipe costs nothing to craft.""#,
    r#", "\nWhile this says default this mod's own list applies.""#,
    r#", "\nA text this mod cannot use is set aside and the field behaves as though it said default; the reason is in the log, or in the load error if the load stops anyway.""#
);

/// The same tail on a PACKS setting, whose format line stops at the ceiling.
pub(crate) const WANT_PACKS_TAIL: &str = concat!(
    r#", "\nA list too long for one line continues on the next; the continuation is part of the same list.""#,
    r#", "\nWrite internal names, as the default line above does, in at most 2000 characters.""#,
    r#", "\nWhile this says default this mod's own list applies.""#,
    r#", "\nA text this mod cannot use is set aside and the field behaves as though it said default; the reason is in the log, or in the load error if the load stops anyway.""#
);

/// The same tail on a text setting that has a DROPDOWN beside it: the switch
/// line names which way the settings screen sorts the two.
pub(crate) const WANT_TEXT_TAIL_ABOVE: &str = concat!(
    r#", "\nA list too long for one line continues on the next; the continuation is part of the same list.""#,
    r#", "\nWrite internal names, as the default line above does, in at most 2000 characters. The word none empties the list, so the recipe costs nothing to craft.""#,
    r#", "\nWhile this says default the option chosen above applies; anything else applies instead of it.""#,
    r#", "\nA text this mod cannot use is set aside and the field behaves as though it said default; the reason is in the log, or in the load error if the load stops anyway.""#
);

/// The same tail again with the dropdown sorting BELOW the text setting, which
/// is what a legacy dropdown ordered after a generated setting produces.
pub(crate) const WANT_TEXT_TAIL_BELOW: &str = concat!(
    r#", "\nA list too long for one line continues on the next; the continuation is part of the same list.""#,
    r#", "\nWrite internal names, as the default line above does, in at most 2000 characters. The word none empties the list, so the recipe costs nothing to craft.""#,
    r#", "\nWhile this says default the option chosen below applies; anything else applies instead of it.""#,
    r#", "\nA text this mod cannot use is set aside and the field behaves as though it said default; the reason is in the log, or in the load error if the load stops anyway.""#
);

/// What a golden writes where [`WANT_TEXT_TAIL`] belongs.
pub(crate) const TEXT_TAIL: &str = "<text tail>";

/// What a golden writes where [`WANT_PACKS_TAIL`] belongs.
pub(crate) const PACKS_TAIL: &str = "<packs tail>";

/// What a golden writes where [`WANT_TEXT_TAIL_ABOVE`] belongs.
pub(crate) const TEXT_TAIL_ABOVE: &str = "<text tail above>";

/// What a golden writes where [`WANT_TEXT_TAIL_BELOW`] belongs.
pub(crate) const TEXT_TAIL_BELOW: &str = "<text tail below>";

pub(crate) fn transcript(ops: &[Op]) -> Vec<String> {
    let mut lines = Vec::with_capacity(ops.len());
    for op in ops {
        match op {
            Op::Extend(proto) => lines.push(format!("extend {}", render_value(proto))),
            Op::Set(path, val) => {
                lines.push(format!("set {} = {}", render_path(path), render_value(val)))
            }
            Op::Log(line) => lines.push(format!("log {}", line)),
        }
    }
    lines
}

pub(crate) fn render_path(path: &[PathEl]) -> String {
    let mut out = String::new();
    for (i, el) in path.iter().enumerate() {
        match el {
            PathEl::Num(n) => {
                out.push('[');
                out.push_str(&format_num(*n));
                out.push(']');
            }
            PathEl::Str(s) => {
                if i > 0 {
                    out.push('.');
                }
                out.push_str(s);
            }
        }
    }
    out
}

/// A byte string's bytes, made readable WITHOUT being made lossy: printable
/// ASCII stays itself, and `"`, `\` and everything outside that range become
/// `\xNN`. Every reader of bytes in this suite shares it, which is what keeps a
/// value holding the raw byte 0xff and a value holding a real U+FFFD from
/// printing the same way in a failure report.
pub(crate) fn escape_bytes(b: &[u8]) -> String {
    let mut out = String::new();
    for x in b {
        match x {
            0x20..=0x7e if *x != b'"' && *x != b'\\' => out.push(*x as char),
            _ => out.push_str(&format!("\\x{:02x}", x)),
        }
    }
    out
}

pub(crate) fn render_value(v: &Value) -> String {
    match v {
        Value::Nil => String::from("nil"),
        Value::Bool(b) => String::from(if *b { "true" } else { "false" }),
        Value::Num(n) => format_num(*n),
        Value::Str(s) => format!("\"{}\"", s),
        // A BYTE STRING IS MARKED, so no expectation can read it as text: the
        // `b` prefix and the `\xNN` escapes say which arm the value is on, and
        // a rendering that quietly printed the bytes would let a `Str` and a
        // `Bytes` holding the same ASCII produce the same line.
        //
        // THIS ONE RENDERING IS NOT MIRRORED, and it cannot be: the Go model
        // has no Bytes arm at all, so its renderer prints the bytes raw inside
        // the quotes a `Str` gets. No cross-language expectation may carry one,
        // which costs nothing, because the two example guests emit no such
        // value and the mirror harness never sees this function.
        Value::Bytes(b) => format!("b\"{}\"", escape_bytes(b)),
        Value::Arr(items) => {
            let parts: Vec<String> = items.iter().map(render_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Map(pairs) => {
            let parts: Vec<String> = pairs
                .iter()
                .map(|(k, val)| format!("{}={}", k, render_value(val)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// The ONE rendering rule the two halves share. Shortest-round-trip printing
/// was the obvious choice and it was wrong: Rust's Display and Go's strconv
/// break ties differently, so real values print as 3109032256237.6563 on one
/// side and 3109032256237.6562 on the other, and the transcripts they are
/// compared through diverge on numbers neither planner rejected. This rule
/// has no ties to break.
///
/// - NaN and the infinities keep the Go spellings.
/// - An integral value the double holds exactly prints as plain digits.
/// - Everything else prints at seventeen significant digits in scientific
///   form. Both languages round the same way at a fixed precision, and this
///   half's exponent shape (no plus, no leading zeros) is the one the Go half
///   normalises onto.
pub(crate) fn format_num(n: f64) -> String {
    if n.is_nan() {
        return String::from("NaN");
    }
    if n.is_infinite() {
        return String::from(if n > 0.0 { "+Inf" } else { "-Inf" });
    }
    if n == n.trunc() && n.abs() <= MAX_EXACT_INT as f64 {
        return format!("{}", n as i64);
    }
    format!("{:.16e}", n)
}

pub(crate) fn field(v: &Value, key: &str) -> Option<Value> {
    if let Value::Map(pairs) = v {
        for (k, val) in pairs {
            if k.as_str() == key {
                return Some(val.clone());
            }
        }
    }
    None
}

/// A composed description carries REAL newlines, which a raw string literal
/// cannot hold and a quoted one would drown in backslashes: the expectations
/// written against it use `\n` and this puts the character back before
/// comparing.
pub(crate) fn assert_composed(got: &[String], want: &[&str]) {
    let want: Vec<String> = want
        .iter()
        .map(|w| {
            w.replace(TEXT_TAIL_ABOVE, WANT_TEXT_TAIL_ABOVE)
                .replace(TEXT_TAIL_BELOW, WANT_TEXT_TAIL_BELOW)
                .replace(TEXT_TAIL, WANT_TEXT_TAIL)
                .replace(PACKS_TAIL, WANT_PACKS_TAIL)
                .replace("\\n", "\n")
        })
        .collect();
    let refs: Vec<&str> = want.iter().map(|s| s.as_str()).collect();
    assert_lines(got, &refs);
}

pub(crate) fn assert_lines(got: &[String], want: &[&str]) {
    assert_lines_named(got, want, "");
}

/// `assert_lines` for a table-driven test: the case's own name is printed with
/// the mismatch, because a table of six worlds reports six identical failures
/// otherwise and none of them says which world produced it.
pub(crate) fn assert_lines_named(got: &[String], want: &[&str], case: &str) {
    let mut bad = false;
    if !case.is_empty() {
        // Printed only on a failure, and before the lines, so the case name
        // reads as a heading over its own diff rather than as another line.
        if got.len() != want.len() || got.iter().zip(want).any(|(g, w)| g != w) {
            println!("case: {}", case);
        }
    }
    let shared = got.len().min(want.len());
    for i in 0..shared {
        if got[i] != want[i] {
            println!("line {}\n got: {}\nwant: {}", i, got[i], want[i]);
            bad = true;
        }
    }
    if got.len() != want.len() {
        println!("got {} lines, want {}", got.len(), want.len());
        for line in got.iter().skip(shared) {
            println!("extra line: {}", line);
        }
        for line in want.iter().skip(shared) {
            println!("missing line: {}", line);
        }
        bad = true;
    }
    assert!(!bad, "the transcript does not match");
}

/// The two halves must render a number the same way or the transcripts they
/// are compared through diverge on a value neither planner rejected. The Go
/// mirror runs this table verbatim; the two must come out byte for byte
/// alike.
#[test]
fn format_num_matches_the_go_mirror() {
    let cases = [
        // Integers, including the negative zero the integer path flattens.
        (0.0, "0"),
        (-0.0, "0"),
        (2.0, "2"),
        (-30.0, "-30"),
        (1e15, "1000000000000000"),
        // The exact-integer boundary the validator blesses, and the first
        // value past it, which no longer holds exactly.
        (9007199254740991.0, "9007199254740991"),
        (9007199254740992.0, "9007199254740992"),
        (-9007199254740992.0, "-9007199254740992"),
        (9007199254740994.0, "9.0071992547409940e15"),
        // Fractions, and the samples a shortest-round-trip renderer split
        // between the two languages.
        (2.5, "2.5000000000000000e0"),
        (-0.75, "-7.5000000000000000e-1"),
        (3109032256237.6562, "3.1090322562376562e12"),
        (2766224557440.7812, "2.7662245574407812e12"),
        (-791902786.7695312, "-7.9190278676953125e8"),
        // Small magnitudes, where the exponent carries its own sign.
        (1e-7, "9.9999999999999995e-8"),
        (2.5e-10, "2.5000000000000002e-10"),
        (5e-324, "4.9406564584124654e-324"),
        (1.7976931348623157e308, "1.7976931348623157e308"),
        (2.2250738585072014e-308, "2.2250738585072014e-308"),
        (0.1, "1.0000000000000001e-1"),
        (6.02214076e23, "6.0221407599999999e23"),
        // The non-finite values, which the planner refuses long before an op
        // carries one; this pins the spelling anyway.
        (f64::INFINITY, "+Inf"),
        (f64::NEG_INFINITY, "-Inf"),
        (f64::NAN, "NaN"),
    ];
    for (input, want) in cases {
        assert_eq!(format_num(input), want, "format_num({})", input);
    }
}

/// The `localised_description` one fallen-back prototype carries, in the shape
/// a transcript shows it.
///
/// EVERY TRANSCRIPT COMPOSES IT RATHER THAN RETYPING THE SENTENCE, exactly as
/// the fallback LINES are composed through `player_fallback`. The sentence
/// itself is pinned by `fallback_note_shape` and the line by
/// `player_fallback_line_shape`, so a drift in either is one failure with the
/// whole text in it rather than thirty.
/// THE KIND AND THE EMITTED NAME ARE THE FIRST TWO ARGUMENTS because a note
/// with no declared `description` opens with `description_ref`'s wrapper, which
/// carries the prototype's own `[<kind>-description]` key: the shape is the
/// prototype's and not the note's, so a transcript that hard-coded one shape
/// would pass a recipe's key onto a technology.
pub(crate) fn note_in(kind: &str, name: &str, setting: &str, destroys_inputs: bool) -> String {
    format!(
        r#"localised_description=["", {}, {}], "#,
        description_ref_in(kind, name),
        chunked_params(&crate::data::fallback_note(setting, destroys_inputs))
    )
}

/// `description_ref`'s wrapper as a transcript prints it, which is one spelling
/// shared by every expectation carrying a note with no declared `description`.
pub(crate) fn description_ref_in(kind: &str, name: &str) -> String {
    format!(
        "[\"?\", [\"\", [\"{}-description.{}\"], \"\n\"], \"\"]",
        kind, name
    )
}

/// One sentence as the PARAMETERS `append_localised` splits it into, quoted and
/// comma separated the way a transcript prints them.
///
/// IT ASKS THE CHUNKER RATHER THAN SPELLING THE CUT, for the reason `note_in`
/// gives about the sentence itself: a transcript here is pinning WHICH SENTENCE
/// a prototype carries, and the split it is carried in is pinned once, with the
/// pieces written out by hand, by `the_chunker_splits_on_spaces_within_the_budget`.
/// Thirty transcripts carrying a hand-copied cut point would be thirty failures
/// the day the budget moves, and none of them would be about what they test.
pub(crate) fn chunked_params(text: &str) -> String {
    crate::value::chunk_localised(text)
        .into_iter()
        .map(|p| format!(r#""{}""#, p))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One ERROR line with the tail a RECIPE'S INGREDIENT TEXT carries and no other
/// fallback does.
pub(crate) fn with_recipe_tail(line: &str) -> String {
    format!("{} {}", line, crate::data::RECIPE_CHANGE_SENTENCE)
}

/// The ONE sentence a technology left with no science pack earns, composed here
/// so the tests that assert it cannot drift apart from each other, exactly as
/// `note_in` composes the tooltip note.
///
/// IT NAMES THE NAMES, which is the whole of what the sentence gained: an author
/// reading it is one whose ladders all missed, and the rungs they wrote are the
/// one thing that says which mod set this is.
/// The ONE sentence a refusal raised after resolution carries when a stored
/// value fell back on the way to it, composed here so the tests that assert it
/// cannot drift apart from each other.
///
/// IT IS A FACT AND NAMES NO SCREEN. An earlier round appended a route to
/// Settings > Mod settings > Startup and the client cannot reach it from an
/// "Error loading mods" dialog; what survives is the half that was true, which
/// is that the player's stored value was set aside. See
/// `Resolution::fallback_fact`.
pub(crate) fn with_fallback_fact(message: &str, setting: &str) -> String {
    format!(
        "{}. The stored value of {} could not be used and was set aside, so what applied is what that field gives when it is left alone.",
        message, setting
    )
}

/// The ONE transcript line a technology left with no science pack earns,
/// composed here so the tests that assert it cannot drift apart from each
/// other, exactly as `note_in` composes the tooltip note.
///
/// IT NAMES THE NAMES, which is the whole of what the sentence carries for an
/// author: the one reading it is an author whose ladders all missed, and the
/// rungs they wrote are the one thing that says which mod set this is.
///
/// IT WAS A REFUSAL AND IS A LINE NOW. The load is not stopped any more: a mod
/// set that demotes one science pack must not be able to lock a player out of a
/// game whose error dialog cannot reach the Mod Settings screen (measured on
/// 2.0.77). See `Resolution::packless_at`.
pub(crate) fn packless_log(tech: &str, tried: &[&str]) -> String {
    format!(
        "log fkrecipes: ERROR: {}: none of {} is a science pack this game has, so the research is emitted with no science pack and completes for free",
        tech,
        tried.join(", ")
    )
}

/// The trailing line the same technology's own description carries, which is
/// where a player who never reads a log finds out that the research is free.
pub(crate) const PACKLESS_TOOLTIP: &str = "This game has none of the science packs this research names, so it takes no science pack at all. The reason is in the log.";

/// What a technology carries when not one source in the chosen tier's ladder
/// handed the library a cost it could copy. It states the environmental fact
/// and stops there: it names no dropdown value, because the value is a raw
/// setting string and this line is prose a player reads, and it names no price,
/// because a `cost_from` beside the tier can put the player's own count or
/// seconds into the unit beside it. See `unpriced_source_note`.
pub(crate) const UNPRICED_TOOLTIP: &str = "No technology this research takes its cost from carries a cost this mod can use here, so this research has no prerequisite and no copied cost. The reason is in the log.";

/// The OTHER emptied-unit tooltip, and the difference between the two is a fact
/// the library has against one it does not. Both technologies are emitted with
/// an empty unit; `PACKLESS_TOOLTIP`'s walk PUT every pack to the game and the
/// game had none of them, and this one never decoded the list at all, so it says
/// what it could not do rather than what the game does not have. See
/// `unreadable_copy_note`.
pub(crate) fn unreadable_copy_tooltip(source: &str) -> String {
    format!(
        "The {} cost this research copies cannot be read in this game, so it takes no science pack at all. The reason is in the log.",
        source
    )
}

/// The line a `cost_of` whose copied pack list could not be decoded logs, and
/// the line a TIER logs for the same fact: the two differ in what the library
/// did next, which is the whole of what a declared cost behind the copy
/// changes.
pub(crate) fn unreadable_copy_log(tech: &str, source: &str) -> String {
    format!(
        "log fkrecipes: ERROR: {}: the unit of {} holds a table this library cannot copy faithfully, so the research is emitted with no science pack and completes for free",
        tech, source
    )
}

pub(crate) fn unreadable_source_log(tech: &str, source: &str) -> String {
    format!(
        "log fkrecipes: ERROR: {}: the unit of {} holds a table this library cannot copy faithfully, so this mod's own declared cost applies instead",
        tech, source
    )
}

/// For the witnesses that are about ONE line's exact text in a stream whose
/// other lines another test already pins whole. Pinning the whole transcript in
/// all five would repeat four drop lines five times and make a change to the
/// drop line a five-test edit.
pub(crate) fn assert_has_line(got: &[String], want: &str) {
    assert!(
        got.iter().any(|line| line == want),
        "the transcript holds no line\nwant: {}\n got: {}",
        want,
        got.join("\n      ")
    );
}
