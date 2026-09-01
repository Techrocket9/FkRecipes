//! A SOURCE-TEXT PROPERTY OVER EVERY MESSAGE THIS CRATE CAN BUILD.
//!
//! The behavioural tests compose real refusals through real plans, which is
//! what proves the composition is right, but each one covers the templates its
//! own six plans happen to reach. This crate has some eighty message chunks
//! and a stage put back into any single one of them ships a sentence the
//! player reads twice:
//!
//! ```text
//! fklua: at the data stage, fkrecipes: at the data stage, two technologies ...
//! ```
//!
//! A sample cannot catch that; only a property over the whole source can. So
//! this reads the crate's own modules and holds every string literal in them
//! to the rule, which makes a template added next year covered on the day it
//! is written rather than on the day somebody remembers to extend a table.
//!
//! SCOPED TO STRING LITERALS, and that scoping is load-bearing rather than
//! tidiness: " stage, " occurs in ordinary wrapped prose all over the doc
//! comments in this crate, so a raw scan of the source text would be all false
//! positives and would be deleted within the week. The scanner below is
//! therefore a small state machine over comments and literals rather than a
//! substring search, and it is the Go half's `go/ast` walk in the one form
//! Rust's standard library leaves available.
//!
//! THE MODULE LIST IS READ FROM DISK rather than fixed with `include_str!`.
//! A fixed list is a list a new module can be left out of, and a message
//! property that silently stops covering a file is worse than none; `std` is
//! available under `cfg(test)` (the locale golden already reads its fixture
//! that way), so the directory itself is the list.

use std::fs;
use std::path::PathBuf;

/// What a refusal may not carry, and why.
///
/// `fkdata::raise` routes a message through the host's own failure path, and
/// `fk_data.lua`'s `fail()` prefixes "fklua: at the <stage> stage, " to it.
/// The stage therefore belongs to the host at every stage, and a message built
/// here carries the crate's attribution and goes straight into the diagnosis.
const FORBIDDEN: &[(&str, &str)] = &[
    (
        "fkrecipes: at the",
        "the old stage-prefixed refusal shape; the host prefixes the stage now",
    ),
    (
        " stage, ",
        "a stage the host will name again, giving the player one sentence with two of them",
    ),
];

/// Every string literal in `text`, with its 1-based line number.
///
/// Comments are skipped (line, and block comments which nest in Rust), as are
/// escapes inside a literal. Raw strings are handled by hash count even though
/// this crate has none today, because the failure mode of not handling them is
/// a scanner that silently reads code as text the day somebody writes one.
fn string_literals(text: &str) -> Vec<(usize, String)> {
    let b: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        // Comments.
        if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '\n' {
                    line += 1;
                } else if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                    depth += 1;
                    i += 1;
                } else if b[i] == '*' && i + 1 < b.len() && b[i + 1] == '/' {
                    depth -= 1;
                    i += 1;
                }
                i += 1;
            }
            continue;
        }
        // A raw string: r, some hashes, then the quote.
        if c == 'r' {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < b.len() && b[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == '"' {
                let start = line;
                let mut s = String::new();
                j += 1;
                'raw: while j < b.len() {
                    if b[j] == '"' {
                        let mut k = j + 1;
                        let mut seen = 0;
                        while k < b.len() && b[k] == '#' && seen < hashes {
                            seen += 1;
                            k += 1;
                        }
                        if seen == hashes {
                            j = k;
                            break 'raw;
                        }
                    }
                    if b[j] == '\n' {
                        line += 1;
                    }
                    s.push(b[j]);
                    j += 1;
                }
                out.push((start, s));
                i = j;
                continue;
            }
        }
        // An ordinary string.
        if c == '"' {
            let start = line;
            let mut s = String::new();
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' && i + 1 < b.len() {
                    // The escape's meaning does not matter here, only that the
                    // next character cannot end the literal.
                    if b[i + 1] == '\n' {
                        line += 1;
                    }
                    s.push(b[i + 1]);
                    i += 2;
                    continue;
                }
                if b[i] == '\n' {
                    line += 1;
                }
                s.push(b[i]);
                i += 1;
            }
            i += 1;
            out.push((start, s));
            continue;
        }
        i += 1;
    }
    out
}

#[test]
fn no_message_carries_its_own_stage() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources: Vec<PathBuf> = Vec::new();
    for entry in
        fs::read_dir(&root).expect("the source is the thing under test and it is not there")
    {
        let path = entry.expect("unreadable directory entry").path();
        // Only the crate's own modules. src/tests/ is test scaffolding, and
        // this file quotes the forbidden shapes twice over.
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            sources.push(path);
        }
    }
    sources.sort();
    // A read that matched nothing would be a test that passes by scanning an
    // empty set, which is the one outcome worse than failing.
    assert!(
        sources.len() >= 5,
        "only {} modules found under {}; the scan would prove nothing",
        sources.len(),
        root.display()
    );

    let mut checked = 0usize;
    let mut problems: Vec<String> = Vec::new();
    for path in &sources {
        let text = fs::read_to_string(path).expect("a module is not readable");
        for (line, lit) in string_literals(&text) {
            checked += 1;
            for (needle, why) in FORBIDDEN {
                if lit.contains(needle) {
                    problems.push(format!(
                        "{}:{}: a string literal contains {:?}, {}\n  literal: {:?}",
                        path.file_name().unwrap().to_string_lossy(),
                        line,
                        needle,
                        why,
                        lit
                    ));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "messages carrying a stage of their own:\n{}",
        problems.join("\n")
    );
    assert!(
        checked >= 50,
        "only {} string literals were scanned across {} modules; the scanner is not reaching the messages",
        checked,
        sources.len()
    );
}

/// The scanner is the thing the property stands on, so it is held up on inputs
/// whose answer is known. A scanner that quietly returned nothing would make
/// the property above pass forever.
#[test]
fn the_literal_scanner_reads_only_literals() {
    let cases: &[(&str, &[&str])] = &[
        (r#"let a = "plain";"#, &["plain"]),
        // The case the scoping exists for.
        (
            "// at the data stage, a comment\nlet a = \"kept\";",
            &["kept"],
        ),
        ("/* at the data stage, */ let a = \"kept\";", &["kept"]),
        // Rust block comments nest, so a naive scan would end this one early
        // and read the rest of the file as code.
        (
            "/* /* nested */ still comment */ let a = \"kept\";",
            &["kept"],
        ),
        // An escaped quote does not end the literal.
        (r#"let a = "say \"hi\" now";"#, &["say \"hi\" now"]),
        // A raw string keeps its backslashes and its inner quotes.
        (
            "let a = r#\"raw \" and \\ here\"#;",
            &["raw \" and \\ here"],
        ),
        // Concatenation-shaped code: each piece is its own literal, which is
        // what makes a message built from six pieces six checks.
        (r#"let a = "one".to_owned() + "two";"#, &["one", "two"]),
    ];
    for (src, want) in cases {
        let got: Vec<String> = string_literals(src).into_iter().map(|(_, s)| s).collect();
        assert_eq!(&got, want, "scanning {:?}", src);
    }
}
