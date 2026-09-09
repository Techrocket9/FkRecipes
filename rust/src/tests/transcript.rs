use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::op::{Op, PathEl};
use crate::value::{Value, MAX_EXACT_INT};

// The transcript is how a test reads an Op stream: one line per op, values
// rendered in the order the planner built them. The Go mirror renders the
// same lines from the same plan, which is what "the two halves agree" means
// before the packaged mirror harness exists to say it in Lua.

/// The two lines this library composes onto EVERY text setting's description,
/// after the default list: what to write and how much of it, then what a text
/// it cannot use costs.
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
    r#", "\nWrite internal names, as the default line above does, in at most 2000 characters.""#,
    r#", "\nA text this mod cannot use is set aside and that default applies instead; the reason is in the log, or in the load error if the load stops anyway.""#
);

/// What a golden writes where [`WANT_TEXT_TAIL`] belongs.
pub(crate) const TEXT_TAIL: &str = "<text tail>";

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
        .map(|w| w.replace(TEXT_TAIL, WANT_TEXT_TAIL).replace("\\n", "\n"))
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
