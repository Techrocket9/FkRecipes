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

/// What one walk over a module says about it: every string literal with its
/// 1-based line number, and the CODE with every comment and literal blanked
/// out, newlines kept so a line number still means something.
///
/// The two come from one walk because they are one question asked twice: what
/// in this file is text a player reads, and what in it is code. A second
/// scanner would be a second answer.
struct Scan {
    literals: Vec<(usize, String)>,
    code: String,
}

/// Every string literal in `text`, with its 1-based line number.
///
/// Comments are skipped (line, and block comments which nest in Rust), as are
/// escapes inside a literal. Raw strings are handled by hash count even though
/// this crate has none today, because the failure mode of not handling them is
/// a scanner that silently reads code as text the day somebody writes one.
fn string_literals(text: &str) -> Vec<(usize, String)> {
    scan(text).literals
}

fn scan(text: &str) -> Scan {
    let b: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut code = String::with_capacity(text.len());
    // What a comment or a literal leaves behind: a newline stays a newline so
    // the line numbers hold, and everything else becomes a space so two
    // identifiers either side of a blanked run cannot be read as one.
    let blank = |code: &mut String, c: char| code.push(if c == '\n' { '\n' } else { ' ' });
    let mut line = 1usize;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            code.push('\n');
            line += 1;
            i += 1;
            continue;
        }
        // Comments.
        if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                blank(&mut code, b[i]);
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            let mut depth = 1;
            blank(&mut code, b[i]);
            blank(&mut code, b[i + 1]);
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '\n' {
                    line += 1;
                } else if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                    depth += 1;
                    blank(&mut code, b[i]);
                    i += 1;
                } else if b[i] == '*' && i + 1 < b.len() && b[i + 1] == '/' {
                    depth -= 1;
                    blank(&mut code, b[i]);
                    i += 1;
                }
                blank(&mut code, b[i]);
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
                for c in &b[i..=j] {
                    blank(&mut code, *c);
                }
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
                            for c in &b[j..k] {
                                blank(&mut code, *c);
                            }
                            j = k;
                            break 'raw;
                        }
                    }
                    if b[j] == '\n' {
                        line += 1;
                    }
                    blank(&mut code, b[j]);
                    s.push(b[j]);
                    j += 1;
                }
                out.push((start, s));
                i = j;
                continue;
            }
        }
        // A CHARACTER LITERAL, AND NOT A LIFETIME. `'"'` is the case that
        // makes this a rule rather than a nicety: read as ordinary code its
        // quote opens a string that runs to the next quote in the file and
        // blanks real code out of the scan, so a call hidden behind one would
        // be invisible to every property below. A lifetime (`'a`, `'static`,
        // `'_`) has no closing quote, and the shape is what tells them apart:
        // one character and then a quote, or a backslash escape and then a
        // quote. A byte literal (`b'x'`) is the same shape with an identifier
        // character in front of it, which changes nothing here.
        if c == '\'' {
            let mut end = None;
            if i + 2 < b.len() && b[i + 1] != '\\' && b[i + 2] == '\'' {
                end = Some(i + 2);
            } else if i + 3 < b.len() && b[i + 1] == '\\' {
                // The character after the backslash cannot close the literal,
                // and `\x41` and `\u{1f600}` run further than one character,
                // so the closing quote is looked for rather than counted to.
                let mut j = i + 3;
                while j < b.len() && b[j] != '\'' && b[j] != '\n' {
                    j += 1;
                }
                if j < b.len() && b[j] == '\'' {
                    end = Some(j);
                }
            }
            if let Some(j) = end {
                for c in &b[i..=j] {
                    blank(&mut code, *c);
                }
                i = j + 1;
                continue;
            }
        }
        // An ordinary string.
        if c == '"' {
            let start = line;
            let mut s = String::new();
            blank(&mut code, c);
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' && i + 1 < b.len() {
                    // The escape's meaning does not matter here, only that the
                    // next character cannot end the literal.
                    if b[i + 1] == '\n' {
                        line += 1;
                    }
                    blank(&mut code, b[i]);
                    blank(&mut code, b[i + 1]);
                    s.push(b[i + 1]);
                    i += 2;
                    continue;
                }
                if b[i] == '\n' {
                    line += 1;
                }
                blank(&mut code, b[i]);
                s.push(b[i]);
                i += 1;
            }
            if i < b.len() {
                blank(&mut code, b[i]);
            }
            i += 1;
            out.push((start, s));
            continue;
        }
        code.push(c);
        i += 1;
    }
    Scan {
        literals: out,
        code,
    }
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

/// THE CEILING IS NEVER TYPED INTO A MESSAGE, which is what makes "one number
/// by construction" a fact rather than a hope.
///
/// [`text_format_line`] builds the sentence the player reads from `MAX_TEXT`,
/// and the parser's own too-long refusal does the same. Nothing in a value
/// test can see the difference between that and a typed 2000: both render the
/// same bytes while the constant happens to be 2000, so a hand-typed digit run
/// would sit there green until somebody changed the ceiling and shipped a
/// description that lied about it. Only a property over the source can see
/// it, so this forbids the ceiling's own decimal rendering in every string
/// literal the crate builds a message out of.
///
/// IT TRACKS THE CONSTANT. The needle is `MAX_TEXT` rendered at run time, so
/// raising the ceiling moves what is forbidden with it. A literal that merely
/// CONTAINS those digits is caught too (12000 would be), and that is the right
/// side to err on: a number near the ceiling in a player-facing sentence is
/// one somebody should have to re-read.
///
/// It is the Go twin's `TestTheCeilingIsNeverTypedIntoAMessage`, over the same
/// module list the stage property above reads.
#[test]
fn the_ceiling_is_never_typed_into_a_message() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources: Vec<PathBuf> = Vec::new();
    for entry in
        fs::read_dir(&root).expect("the source is the thing under test and it is not there")
    {
        let path = entry.expect("unreadable directory entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            sources.push(path);
        }
    }
    sources.sort();
    assert!(
        sources.len() >= 5,
        "only {} modules found under {}; the scan would prove nothing",
        sources.len(),
        root.display()
    );

    let needle = alloc::format!("{}", crate::ingredient_list::MAX_TEXT);
    let mut checked = 0usize;
    let mut problems: Vec<String> = Vec::new();
    for path in &sources {
        let text = fs::read_to_string(path).expect("a module is not readable");
        for (line, lit) in string_literals(&text) {
            checked += 1;
            if lit.contains(&needle) {
                problems.push(alloc::format!(
                    "{}:{}: a string literal types the ceiling {} instead of building it from MAX_TEXT\n  literal: {:?}",
                    path.file_name().unwrap().to_string_lossy(),
                    line,
                    needle,
                    lit
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "messages typing the ceiling instead of building it:\n{}",
        problems.join("\n")
    );
    assert!(
        checked >= 50,
        "only {} string literals were scanned across {} modules; the scanner is not reaching the messages",
        checked,
        sources.len()
    );
}

/// THE SEAM, AS A SOURCE PROPERTY.
///
/// The behavioural tests prove that a plan declaring no text setting installs
/// no language table. They cannot prove that no FUTURE line calls the parser
/// or the renderer by name, and one such line is all it takes: a direct call
/// is a reference link-time elimination has to keep, so the whole language
/// would ship again in every consumer's mod, silently and with every test
/// still green. That is a property over the source, so it is asserted over
/// the source, and it is the one tripwire here that needs no wasm toolchain
/// at all.
///
/// TWO RULES, because the seam has two halves, and they are the Go twin's
/// (`go/source_test.go`) in the one form Rust's standard library leaves
/// available. A guarded item may be NAMED only in the modules its row lists,
/// which is what makes `use crate::ingredient_list::render_list as show;` a
/// violation rather than the way around this; and it may be CALLED only in
/// the modules its row lists for calls, which for the custom-cost resolver is
/// nowhere at all, because the plan reaches it through the installed
/// pointer.
///
/// A CALL IS EVERY FORM THE LANGUAGE HAS: `name(`, `::name(`, `Self::name(`,
/// `Lib::name(` and, for a method, `.name(`. They are all one shape here, the
/// name with a `(` behind it and no identifier character in front, because
/// what precedes the name is a path and a path is what the naming rule
/// already covers. A DEFINITION (`fn name(` and `fn name<`, and `static
/// name:` or `const name:` for the two installation points) is neither, and
/// it is told apart rather than left to the substring search, or the line
/// that defines the resolver would report itself.
///
/// A DOT MEANS DIFFERENT THINGS EITHER SIDE OF THE SEAM, and `method` is that
/// fact rather than a convenience. This crate's table carries fields named
/// for the functions they hold, so `(lang.parse)(...)` reads as `parse` with
/// a dot in front of it: a dotted FREE FUNCTION is a field, or a method on
/// some other type, and can never be the item, so it is the seam working. A
/// dotted INHERENT METHOD is the item, so `self.resolve_custom_cost(...)` is
/// exactly the defect this catches. The Go twin needs no such flag only
/// because its table's fields are named differently from its functions.
struct SeamGuard {
    /// The function, static or constant guarded.
    name: &'static str,
    /// The module that defines it. A row whose name has no definition there
    /// is a table gone stale, and a stale table is a property that passes
    /// because it stopped asking.
    home: &'static str,
    /// The modules that may NAME it at all, in any position.
    namers: &'static [&'static str],
    /// The modules that may CALL it. Empty means nowhere may.
    callers: &'static [&'static str],
    /// Whether it is an inherent method rather than a free function. See the
    /// note on the dot above.
    method: bool,
}

/// The four functions the language table carries, the lexer helper the parser
/// reaches through, the one method the plan holds beside them, and the two
/// values that install all of it.
///
/// `plan.rs` names no function here at all: the table is built inside the
/// language's own module, so the constructors install it by naming `LANGUAGE`
/// and `CUSTOM_COST`, which is why those two rows list `plan.rs` and the
/// function rows do not. `data.rs` may NAME `resolve_custom_cost`, because
/// that is where the resolver is defined and where `CUSTOM_COST` holds it,
/// and it may not call it.
const SEAM: &[SeamGuard] = &[
    SeamGuard {
        name: "parse",
        home: "ingredient_list.rs",
        namers: &["ingredient_list.rs"],
        callers: &["ingredient_list.rs"],
        method: false,
    },
    SeamGuard {
        name: "render",
        home: "ingredient_list.rs",
        namers: &["ingredient_list.rs"],
        callers: &["ingredient_list.rs"],
        method: false,
    },
    SeamGuard {
        name: "render_list",
        home: "ingredient_list.rs",
        namers: &["ingredient_list.rs"],
        callers: &["ingredient_list.rs"],
        method: false,
    },
    SeamGuard {
        name: "format_amount",
        home: "ingredient_list.rs",
        namers: &["ingredient_list.rs"],
        callers: &["ingredient_list.rs"],
        method: false,
    },
    // The lexer's own helper, named nowhere but at home: no constructor
    // installs it, because the parser is what reaches it.
    SeamGuard {
        name: "classify",
        home: "ingredient_list.rs",
        namers: &["ingredient_list.rs"],
        callers: &["ingredient_list.rs"],
        method: false,
    },
    SeamGuard {
        name: "resolve_custom_cost",
        home: "data.rs",
        namers: &["data.rs"],
        callers: &[],
        method: true,
    },
    // THE TWO INSTALLATION POINTS, which are the last way round the rows
    // above: `LANGUAGE.parse` and `CUSTOM_COST(...)` reach the functions
    // without naming one, and reaching them from a planner would link the
    // language into every consumer exactly as a direct call does. Naming
    // them is `plan.rs`'s job and nobody else's.
    SeamGuard {
        name: "LANGUAGE",
        home: "ingredient_list.rs",
        namers: &["ingredient_list.rs", "plan.rs"],
        callers: &[],
        method: false,
    },
    SeamGuard {
        name: "CUSTOM_COST",
        home: "data.rs",
        namers: &["data.rs", "plan.rs"],
        callers: &[],
        method: false,
    },
];

#[test]
fn only_the_language_module_names_the_language() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources: Vec<PathBuf> = Vec::new();
    for entry in
        fs::read_dir(&root).expect("the source is the thing under test and it is not there")
    {
        let path = entry.expect("unreadable directory entry").path();
        // Every module, the language's own included: which of them may name
        // what is the table's answer and not the walk's.
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            sources.push(path);
        }
    }
    sources.sort();
    assert!(
        sources.len() >= 5,
        "only {} modules found under {}; the scan would prove nothing",
        sources.len(),
        root.display()
    );

    let mut problems: Vec<String> = Vec::new();
    // Counted so a rename cannot turn this into a walk over nothing.
    let mut defined = vec![0usize; SEAM.len()];
    for path in &sources {
        let text = fs::read_to_string(path).expect("a module is not readable");
        let code: Vec<char> = scan(&text).code.chars().collect();
        let module = path.file_name().unwrap().to_string_lossy().to_string();
        for (g, seen) in SEAM.iter().zip(defined.iter_mut()) {
            let needle: Vec<char> = g.name.chars().collect();
            if code.len() < needle.len() {
                continue;
            }
            for i in 0..=code.len() - needle.len() {
                if code[i..i + needle.len()] != needle[..] {
                    continue;
                }
                let before = if i == 0 { ' ' } else { code[i - 1] };
                let after = code.get(i + needle.len()).copied().unwrap_or(' ');
                // The name has to match whole: `render_value` is not
                // `render`, and `resolved_pack_list` is nothing here.
                if is_name_char(before) || is_name_char(after) {
                    continue;
                }
                if before == '.' && !g.method {
                    continue;
                }
                let defines = is_a_definition(&code, i, after);
                if defines && module == g.home {
                    *seen += 1;
                }
                let line = code[..i].iter().filter(|c| **c == '\n').count() + 1;
                if !g.namers.contains(&module.as_str()) {
                    problems.push(format!(
                        "{}:{}: names {}, which links the ingredient language into every consumer; a `use` alias is a name like any other, and only {} may name it",
                        module,
                        line,
                        g.name,
                        g.namers.join(" and ")
                    ));
                    continue;
                }
                if !defines && after == '(' && !g.callers.contains(&module.as_str()) {
                    problems.push(format!(
                        "{}:{}: calls {} directly, which links the ingredient language into every consumer; reach it through the value the text-setting constructors install",
                        module, line, g.name
                    ));
                }
            }
        }
    }
    for (g, seen) in SEAM.iter().zip(defined.iter()) {
        if *seen == 0 {
            problems.push(format!(
                "{} is defined nowhere in {}; the seam table guards a name this crate no longer has",
                g.name, g.home
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "the ingredient language is reached by name, which links it into every consumer:\n{}",
        problems.join("\n")
    );
}

/// What may sit inside a Rust identifier, which is what makes the search
/// above match whole names rather than prefixes.
fn is_name_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

/// Whether the name starting at `i` is the one an item declares: `fn name(`
/// or `fn name<` for a function, and `static name:` or `const name:` for the
/// two installation points. A definition is not a call, and without this the
/// line that defines the resolver would report itself.
fn is_a_definition(code: &[char], i: usize, after: char) -> bool {
    let keyword = |word: &str| {
        let w: Vec<char> = word.chars().collect();
        let mut k = i;
        while k > 0 && (code[k - 1] == ' ' || code[k - 1] == '\n' || code[k - 1] == '\t') {
            k -= 1;
        }
        k >= w.len()
            && code[k - w.len()..k] == w[..]
            && (k == w.len() || !is_name_char(code[k - w.len() - 1]))
    };
    (keyword("fn") && (after == '(' || after == '<'))
        || ((keyword("static") || keyword("const")) && after == ':')
}

/// EVERY REACHABLE ITEM OF THE LANGUAGE MODULE, AND NOT A HAND-KEPT LIST OF
/// THEM.
///
/// The rule above guards the names the table lists. The names it does NOT
/// list are the hole in it: a planner that calls some OTHER function of the
/// language module links that part of the language into every consumer with
/// the whole suite still green, and the Go half measured the packaged module
/// growing back through exactly one such call. Go closes the hole by walking
/// `ingredientlist.go` and guarding every top-level function the file
/// declares.
///
/// RUST CLOSES MOST OF IT WITHOUT A TEST, which is why this rule is shaped
/// differently from the Go one rather than copied from it. A private `fn`
/// cannot be called from another module at all, so the language's helpers sit
/// outside a planner's reach by the compiler's own rule. What is left is the
/// WIDENING: a `pub(crate)` written in front of one of them is the whole step
/// that opens the hole, and it is the kind of step a reviewer reads as a
/// formality. So the module's own declarations are held to one rule: whatever
/// faces outward is a name the seam table guards, and everything else stays
/// private.
///
/// TOLD BY KEYWORD, NOT BY NAME. `ListKind`, `ListText`, `IngredientList`,
/// `ListEntry`, `DEFAULT` and the rest of the module's data are free to face
/// outward, because a type or a constant is not a function and no call links
/// through one. `fn`, and the `static` that holds the table of them, are the
/// two shapes that carry code, so those two keywords are what the walk looks
/// for and it asks nothing about the name it finds.
///
/// AT ANY DEPTH, AND STRICTER THAN REACHABILITY ON PURPOSE. A `pub(crate) fn`
/// inside a private nested module is not in fact reachable, and this reports
/// it anyway: the walk reads visibility keywords rather than resolving a
/// module tree, and the price of that is a `pub` somebody deletes once,
/// against a scanner everybody would have to trust twice.
#[test]
fn nothing_reachable_in_the_language_module_is_unguarded() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("ingredient_list.rs");
    let module = path.file_name().unwrap().to_string_lossy().to_string();
    let text = fs::read_to_string(&path)
        .expect("the language module is the thing under test and it is not there");
    let found = exported_code_items(&scan(&text).code);
    // A walk that came back empty would be a property that passes because it
    // stopped asking. `LANGUAGE` is the one item here that cannot be made
    // private, because `plan.rs` names it to install the table, so it is the
    // floor the walk is held to.
    assert!(
        found.iter().any(|d| d.name == "LANGUAGE"),
        "the walk over {} found no LANGUAGE among the {} outward-facing items it saw; it is not reading the module",
        module,
        found.len()
    );

    let guarded: Vec<&str> = SEAM
        .iter()
        .filter(|g| g.home == module)
        .map(|g| g.name)
        .collect();
    let mut problems: Vec<String> = Vec::new();
    for d in &found {
        if !guarded.contains(&d.name.as_str()) {
            problems.push(format!(
                "{}:{}: {} is reachable from outside the language module and the seam table does not guard it; keep it private or guard it",
                module, d.line, d.name
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "the ingredient language is reachable past its own seam table:\n{}",
        problems.join("\n")
    );
}

/// One declaration the walk found: the name it declares, and its 1-based
/// line.
struct Decl {
    name: String,
    line: usize,
}

/// Every `fn` and `static` in `code` whose visibility faces outward, at any
/// depth: `pub`, and the narrowed forms `pub(crate)`, `pub(super)` and
/// `pub(in ...)`, all of which still cross the module boundary this rule is
/// about.
///
/// `code` is the blanked half of [`scan`], so a `pub fn` written inside a
/// comment or quoted in a message is already gone and only Rust is left. The
/// walk is over WORDS rather than characters, because what may sit between
/// the visibility and the keyword (`unsafe`, `const`, `async`, `extern "C"`)
/// is a run of words, and a character search would have to spell out every
/// ordering of them.
fn exported_code_items(code: &str) -> Vec<Decl> {
    let b: Vec<char> = code.chars().collect();
    // Words, plus single punctuation characters so a `pub(...)` restriction
    // can be stepped over.
    let mut toks: Vec<(usize, String)> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if is_name_char(b[i]) {
            let start = i;
            while i < b.len() && is_name_char(b[i]) {
                i += 1;
            }
            toks.push((start, b[start..i].iter().collect()));
            continue;
        }
        if !b[i].is_whitespace() {
            toks.push((i, b[i].to_string()));
        }
        i += 1;
    }

    let mut out: Vec<Decl> = Vec::new();
    for (t, tok) in toks.iter().enumerate() {
        if tok.1 != "pub" {
            continue;
        }
        let mut k = t + 1;
        // The restriction is stepped over rather than read: every form of it
        // still faces out of this module.
        if k < toks.len() && toks[k].1 == "(" {
            let mut depth = 0usize;
            while k < toks.len() {
                if toks[k].1 == "(" {
                    depth += 1;
                } else if toks[k].1 == ")" {
                    depth -= 1;
                    if depth == 0 {
                        k += 1;
                        break;
                    }
                }
                k += 1;
            }
        }
        while k < toks.len()
            && matches!(toks[k].1.as_str(), "unsafe" | "const" | "async" | "extern")
        {
            k += 1;
        }
        // `pub(crate) const DEFAULT`, `pub(crate) type ParseFn`, a `pub`
        // field: data, and the keyword is what says so.
        if k >= toks.len() || (toks[k].1 != "fn" && toks[k].1 != "static") {
            continue;
        }
        let mut n = k + 1;
        if toks[k].1 == "static" && n < toks.len() && toks[n].1 == "mut" {
            n += 1;
        }
        if n < toks.len() && toks[n].1.chars().all(is_name_char) {
            out.push(Decl {
                name: toks[n].1.clone(),
                line: b[..toks[n].0].iter().filter(|c| **c == '\n').count() + 1,
            });
        }
    }
    out
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

/// The other half of the same walk, and the half the seam property stands on.
/// A `code` that came back blank would make that property pass over an empty
/// file forever, and a `code` that kept comments and literals would fail it
/// over a sentence nobody executes.
#[test]
fn the_scanner_blanks_comments_and_literals_out_of_the_code() {
    let cases: &[(&str, bool)] = &[
        ("let a = parse(x);", true),
        // What a message about the language may say without being one.
        (r#"let a = "call parse(x) instead";"#, false),
        ("// never write parse(x) here\nlet a = 1;", false),
        ("/* parse(x) */ let a = 1;", false),
        ("let a = r#\"parse(x)\"#;", false),
        // THE CHARACTER LITERAL THAT HOLDS A QUOTE. Without a case for it the
        // quote opens a string that runs to the next one, and everything
        // between is blanked out of the code, call included.
        ("let q = '\"'; let a = parse(x);", true),
        ("let q = b'\"'; let a = parse(x);", true),
        ("let q = '\\''; let a = parse(x);", true),
        // A lifetime is not a literal and opens nothing: its quote has no
        // partner, and a scanner that thought it did would blank the rest of
        // the file.
        ("fn f<'a>(s: &'a str) { parse(s); }", true),
        ("let a: &'static str = \"x\"; parse(a);", true),
    ];
    for (src, want) in cases {
        let code = scan(src).code;
        assert_eq!(
            code.contains("parse("),
            *want,
            "scanning {:?} gave code {:?}",
            src,
            code
        );
    }
    // A character literal is not a string literal, so the message property
    // must not be handed one to check.
    assert!(string_literals("let q = '\"';").is_empty());
    // The line numbers have to survive the blanking, or every report points at
    // the wrong line.
    assert_eq!(
        scan("/* one\ntwo */\nlet a = parse(x);")
            .code
            .lines()
            .count(),
        3
    );
}

/// The declaration walk, held up on the shapes the language module does not
/// have today. The shapes it does have are proved by the property itself,
/// which runs over the real file; these are the ones that would go wrong on
/// the day somebody writes one.
#[test]
fn the_walk_reads_outward_facing_code_only() {
    let cases: &[(&str, &[&str])] = &[
        ("pub(crate) fn parse(x: u8) {}", &["parse"]),
        ("fn private_helper(x: u8) {}", &[]),
        ("pub fn wide() {}", &["wide"]),
        ("pub(super) fn up() {}", &["up"]),
        ("pub(in crate::plan) fn narrowed() {}", &["narrowed"]),
        ("pub(crate) unsafe fn risky() {}", &["risky"]),
        ("pub const fn folded() {}", &["folded"]),
        ("pub extern \"C\" fn abi() {}", &["abi"]),
        (
            "pub(crate) static LANGUAGE: Language = Language {};",
            &["LANGUAGE"],
        ),
        ("pub(crate) static mut COUNT: u8 = 0;", &["COUNT"]),
        // Data is not code: a constant, a type alias whose right-hand side is
        // spelled `fn`, and a field named for the function it holds all carry
        // a visibility, and none of them links a function in.
        ("pub(crate) const DEFAULT: &str = \"default\";", &[]),
        ("pub(crate) type ParseFn = fn(&str) -> u8;", &[]),
        ("pub(crate) struct L { pub(crate) parse: ParseFn }", &[]),
        ("pub(crate) enum ListKind { Recipe }", &[]),
        // Depth. A method on an impl and a function inside a nested module are
        // both declarations, and indentation says nothing about either.
        ("impl L { pub(crate) fn method(&self) {} }", &["method"]),
        ("mod inner { pub(crate) fn buried() {} }", &["buried"]),
        // A comment and a message are not declarations, which is the whole
        // reason the walk is handed the blanked half of the scan.
        ("// pub(crate) fn commented() {}", &[]),
        ("let a = \"pub(crate) fn quoted() {}\";", &[]),
    ];
    for (src, want) in cases {
        let got: Vec<String> = exported_code_items(&scan(src).code)
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert_eq!(&got, want, "walking {:?}", src);
    }
    // The line number is what a report points at, so it is asked for rather
    // than assumed.
    let found = exported_code_items(&scan("fn a() {}\n\npub(crate) fn b() {}").code);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 3);
}

// ---------------------------------------------------------------------------
// The resolved-recipes hand-over, as a source property.
// ---------------------------------------------------------------------------

/// ONE DOOR INTO `Resolution::recipes`, AND THIS IS WHAT KEEPS IT ONE.
///
/// `resolve` answers a recipe's ingredients through four arms: a text that
/// takes a dropdown's choice over, a dropdown on a preset, a text with no
/// dropdown beside it, and a plain declared list. A check written into one of
/// them is missing from three, and from whichever arm is added next;
/// `Resolution::add_recipe` exists so there is one place that sees every
/// resolved list, and the self-product line lives there.
///
/// A FIFTH ARM THAT PUSHED DIRECTLY WOULD PASS EVERY BEHAVIOURAL TEST, because
/// a test can only assert about the arms it happens to build a plan through.
/// The defect is a property of the source, so it is asserted over the source,
/// and it costs no toolchain at all.
///
/// A COUNT ALONE IS NOT THE RULE, BECAUSE A COUNT ALONE IS GREEN FOR TOO MUCH.
/// `recipes.extend(`, `recipes.append(`, `recipes.insert(`, a `&mut
/// res.recipes` bound to a local, `Vec::push(&mut res.recipes, list)` and an
/// accessor returning `&mut Vec<..>` all reach the vector while leaving the
/// count at one, and every one of them compiles, passes `cargo fmt --check`
/// and passes clippy with warnings denied. Sharpest of all is the plausible
/// refactor: lift the push out of the hand-over into a `push_resolved` helper
/// and let a fifth arm call the helper, and the module still carries exactly
/// one push while `add_recipe` no longer sees every list.
///
/// SO THE RULE IS THREE THINGS AT ONCE. The module carries exactly one
/// `recipes.push(`; that push sits INSIDE the body of the one function named
/// here, brace-matched over the blanked code so a brace in a comment or a
/// string cannot move the span; and none of the other reaches appears outside
/// that body. Reads are untouched, because reading one is what the prototype
/// loop does on every plan.
///
/// ONE SHAPE STILL ESCAPES IT, RECORDED RATHER THAN PAPERED OVER:
/// `res = Resolution { recipes: lists, ..Default::default() }`, a wholesale
/// reconstruction, compiles, formats, lints and leaves this green. The Go twin
/// catches its equivalent because a composite literal with a `recipes:` key is
/// one of its five shapes. Closing it here needs a `Resolution {` or
/// `recipes: ` needle, and both have honest occurrences today (the return type
/// of `Lib::resolve`, three `impl Resolution` blocks, the field declaration,
/// and `Lib::new`'s own `recipes: Vec::new()`), so it is a false-positive
/// trade rather than an oversight.
struct RecipesWriter {
    /// The module's path under `src`, so a future `src/<dir>/data.rs` is a
    /// different module from this one rather than the same row twice.
    module: &'static str,
    /// The item whose body is the ONLY place in that module a push may sit.
    func: &'static str,
    why: &'static str,
}

const RECIPES_WRITERS: &[RecipesWriter] = &[
    RecipesWriter {
        module: "data.rs",
        func: "fn add_recipe(",
        why: "the one hand-over every resolved list goes through, so a check written there covers every arm",
    },
    RecipesWriter {
        module: "plan.rs",
        // `recipe_decl` and not `recipe`: the public `recipe` and
        // `legacy_recipe` both hand down to it, and it is the one that pushes.
        // The Go twin's row is lib.go's private `recipe`, which is the same
        // function under the name that half gave it.
        func: "fn recipe_decl(",
        why: "the declaration constructor, which is the plan's own list and not the resolution's",
    },
];

/// The reaches that are refused outside an allowed body. Each was injected,
/// each left the push count at one, and each is red here.
const REFUSED_REACHES: &[&str] = &[
    "recipes.extend(",
    "recipes.append(",
    "recipes.insert(",
    "recipes.resize(",
    "recipes = ",
    "&mut self.recipes",
    "&mut res.recipes",
    "Vec::push(&mut",
];

const NO_ROW: &str =
    "no module but the hand-over and the declaration constructor may write a recipes vector at all";

#[test]
fn only_the_hand_over_writes_the_resolved_recipes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let sources = crate_modules(&root);
    assert!(
        sources.len() >= 5,
        "only {} modules found under {}; the scan would prove nothing",
        sources.len(),
        root.display()
    );

    let mut problems: Vec<String> = Vec::new();
    for path in &sources {
        let text = fs::read_to_string(path).expect("a module is not readable");
        let code: Vec<char> = scan(&text).code.chars().collect();
        let module = path
            .strip_prefix(&root)
            .expect("a module outside the tree that produced it")
            .to_string_lossy()
            .to_string();
        let row = RECIPES_WRITERS.iter().find(|wr| wr.module == module);
        let want = if row.is_some() { 1 } else { 0 };
        let why = row.map(|wr| wr.why).unwrap_or(NO_ROW);

        let pushes = needle_hits(&code, "recipes.push(");
        if pushes.len() != want {
            problems.push(format!(
                "{}: pushes onto .recipes {} times at lines {:?}; it is meant to do so {} times, being {}",
                module,
                pushes.len(),
                lines_of(&code, &pushes),
                want,
                why
            ));
        }

        // The one body a write may sit in. A row whose function has gone is a
        // rule with a stale allowance, which is a rule that has stopped
        // guarding something, so it is a problem in its own right.
        let mut span = None;
        if let Some(wr) = row {
            match body_span(&code, wr.func) {
                Some(s) => span = Some(s),
                None => problems.push(format!(
                    "{}: has no `{}` for the writes to sit in; it is meant to hold {}",
                    module, wr.func, wr.why
                )),
            }
        }
        let inside = |i: usize| span.is_some_and(|(from, to)| i > from && i < to);

        // A module with no row has already been reported by the count above,
        // and "outside" means nothing there: every line of it is outside.
        if let Some(wr) = row {
            for i in &pushes {
                if inside(*i) {
                    continue;
                }
                problems.push(format!(
                    "{}:{}: pushes onto .recipes outside `{}`; only that body may, being {}",
                    module,
                    line_of(&code, *i),
                    wr.func,
                    wr.why
                ));
            }
        }
        for needle in REFUSED_REACHES {
            for i in needle_hits(&code, needle) {
                if inside(i) {
                    continue;
                }
                problems.push(format!(
                    "{}:{}: reaches a recipes vector as `{}`, which {}",
                    module,
                    line_of(&code, i),
                    needle,
                    match row {
                        Some(wr) => alloc::format!("is outside `{}`, the only body that may", wr.func),
                        None => String::from("no module outside the hand-over and the declaration constructor may do at all"),
                    }
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "a resolved list reaches Resolution::recipes without going through Resolution::add_recipe, so a check written there would not see it:\n{}",
        problems.join("\n")
    );
}

/// Every module of the crate itself, walked INTO subdirectories so a writer in
/// a module added under `src/<dir>/` is scanned on the day it is written
/// rather than never.
///
/// `src/tests/` is left out, and that is the one exclusion: it is the crate's
/// own scaffolding, and its fixture world keeps a `recipes` vector of its own
/// that it pushes onto. The Go twin makes the same exclusion when
/// packageSources drops every `_test.go`.
fn crate_modules(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack = alloc::vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            fs::read_dir(&dir).expect("the source is the thing under test and it is not there");
        for entry in entries {
            let path = entry.expect("unreadable directory entry").path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) == Some("tests") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Every char index at which `needle` occurs as a whole token: an identifier
/// character may not abut an end that is one itself, so `sub_recipes.push(` is
/// not `recipes.push(` and `&mut res.recipes_by_name` is not `&mut
/// res.recipes`.
fn needle_hits(code: &[char], needle: &str) -> Vec<usize> {
    let n: Vec<char> = needle.chars().collect();
    let mut out = Vec::new();
    if code.len() < n.len() {
        return out;
    }
    for i in 0..=code.len() - n.len() {
        if code[i..i + n.len()] != n[..] {
            continue;
        }
        if is_name_char(n[0]) && i > 0 && is_name_char(code[i - 1]) {
            continue;
        }
        let after = code.get(i + n.len()).copied().unwrap_or(' ');
        if is_name_char(n[n.len() - 1]) && is_name_char(after) {
            continue;
        }
        out.push(i);
    }
    out
}

/// The half-open char range of the body of the first item written as `needle`,
/// brace-matched over the BLANKED code so a brace inside a comment or a string
/// literal cannot move it. `None` when the item is not there at all.
fn body_span(code: &[char], needle: &str) -> Option<(usize, usize)> {
    let at = *needle_hits(code, needle).first()?;
    let mut i = at;
    while i < code.len() && code[i] != '{' {
        i += 1;
    }
    let from = i;
    let mut depth = 0usize;
    while i < code.len() {
        match code[i] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((from, i));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The 1-based line a char index sits on.
fn line_of(code: &[char], i: usize) -> usize {
    code[..i].iter().filter(|c| **c == '\n').count() + 1
}

fn lines_of(code: &[char], at: &[usize]) -> Vec<usize> {
    at.iter().map(|i| line_of(code, *i)).collect()
}

// ---------------------------------------------------------------------------
// Every refusal site, classified, as a source property.
// ---------------------------------------------------------------------------

/// THE TWO-SIDED RULE, MADE MECHANICAL.
///
/// A CHECK THAT ASKS THE `World` ANYTHING IS ENVIRONMENTAL AND DEGRADES LOUDLY;
/// A CHECK THAT READS ONLY THE DECLARATION IS AN AUTHOR BUG AND REFUSES BY
/// NAME. An environmental check stays a refusal only where degrading would
/// overwrite another mod's prototype, hand the engine something it refuses
/// anyway, or invent a value the author never declared; and one more, which the
/// threat model rather than the rule supplies: a site the game cannot reach at
/// all.
///
/// THE CLASSIFICATION KEYS ON WHAT THE CHECK ASKS, not on where an author would
/// have noticed it. A fixture `World` that answers `tool_exists` true for a pack
/// the shipped mod set demotes is green in development and red in play, so
/// "visible in development" is a property of the author's fixture and this
/// crate cannot see a fixture.
///
/// WHY A SOURCE PROPERTY. Nothing in a behavioural test can see a refusal ADDED
/// to a World-asking function: the new sentence has its own new test, every old
/// test stays green, and the policy has changed with no reader.
///
/// THE COUNT IS PART OF THE RULE. Every row pins how many refusals its item
/// builds, and every hit in a scanned module has to sit inside one of them, so
/// a refusal added anywhere goes red until somebody classifies it.
///
/// ONE ROW THIS FILE HAS AND THE GO TWIN DOES NOT. This crate's ingredient
/// language answers with `Result<_, String>`, where the `String` is a PROBLEM
/// that becomes a player's fallback line and never stops a load; the Go half
/// returns a plain string there and builds no `errors.New` at all, so its
/// census never sees the module. The row below says so rather than leaving the
/// module unscanned.
const CLASS_DECLARATION: &str = "reads only the declaration";
const CLASS_ENVIRONMENTAL: &str = "asks the game";
const CLASS_MIXED: &str = "holds both kinds";
const CLASS_NOT_A_REFUSAL: &str = "builds no refusal at all";
/// The item takes a `World` and every refusal in it still reads only the
/// declaration. It exists because `CLASS_DECLARATION` is ENFORCED by the World
/// check and this shape would fail it honestly: `resolve_custom_cost` walks pack
/// ladders, so a `World` is in its signature, and the two sentences it builds
/// are both about a DECLARED default the settings stage would already have
/// refused. Saying that is not the same as `CLASS_MIXED` with no sites, which
/// asserts nothing at all.
const CLASS_DECLARATION_IN_WORLD: &str = "names a World and still reads only the declaration";

/// The enumerated exceptions, and nothing else may appear in a row. Each is the
/// reason degrading would be WORSE than refusing.
const EXCEPT_OVERWRITES: &str = "degrading silently clobbers a stranger's prototype";
const EXCEPT_ENGINE_REFUSES: &str =
    "degrading hands the engine something it refuses with a worse message";
const EXCEPT_INVENTS: &str =
    "there is nothing declared to degrade to, so the library would have to invent a value";
const EXCEPT_UNREACHABLE: &str = "the game cannot reach it: the engine resets an unoffered stored value before any stage runs (measured), so only a hand-edited file arrives here, which the threat model puts out of scope";

const ALLOWED_EXCEPTIONS: &[&str] = &[
    EXCEPT_OVERWRITES,
    EXCEPT_ENGINE_REFUSES,
    EXCEPT_INVENTS,
    EXCEPT_UNREACHABLE,
];

/// One environmental refusal inside a mixed item: a fragment of the sentence
/// distinctive enough to name exactly one site, and the exception it claims.
struct EnvironmentalSite {
    fragment: &'static str,
    except: &'static str,
}

struct RefusalClass {
    /// The module's path under `src`.
    module: &'static str,
    /// The item whose body holds the refusals, written as its `fn` needle. The
    /// EMPTY string is the whole module, which only `CLASS_NOT_A_REFUSAL` uses.
    func: &'static str,
    class: &'static str,
    /// The exception an all-environmental row claims; empty for every other
    /// class.
    except: &'static str,
    sites: &'static [EnvironmentalSite],
    /// How many refusals the item builds.
    n: usize,
    why: &'static str,
}

const REFUSAL_CLASSES: &[RefusalClass] = &[
    RefusalClass {
        module: "cycle.rs",
        func: "fn check_cycles(",
        class: CLASS_ENVIRONMENTAL,
        except: EXCEPT_ENGINE_REFUSES,
        sites: &[],
        n: 1,
        why: "the one ring it still refuses holds NO edge this plan made: every edge of ours is dropped with a line and a tooltip first, so what is left is the game's own tree looping without this mod in it, which the engine refuses on its own and which the threat model puts out of scope",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn plan_data(",
        class: CLASS_MIXED,
        except: "",
        sites: &[EnvironmentalSite {
            fragment: "the mod name is empty",
            except: EXCEPT_INVENTS,
        }],
        n: 2,
        why: "one host-wiring fault that asks the World for the prefix everything else derives from, and one declaration fault (a Lib built without New)",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn validate(",
        class: CLASS_MIXED,
        except: "",
        sites: &[
            EnvironmentalSite {
                fragment: "the item {} already exists in data.raw",
                except: EXCEPT_OVERWRITES,
            },
            EnvironmentalSite {
                fragment: "the recipe {} already exists in data.raw",
                except: EXCEPT_OVERWRITES,
            },
            EnvironmentalSite {
                fragment: "the technology {} already exists in data.raw",
                except: EXCEPT_OVERWRITES,
            },
            EnvironmentalSite {
                fragment: "names a place_result {} that does not exist",
                except: EXCEPT_INVENTS,
            },
            EnvironmentalSite {
                fragment: "produces {}, which does not exist",
                except: EXCEPT_INVENTS,
            },
            EnvironmentalSite {
                fragment: "no technology of that name exists",
                except: EXCEPT_INVENTS,
            },
            EnvironmentalSite {
                fragment: "is a research_trigger technology",
                except: EXCEPT_INVENTS,
            },
            EnvironmentalSite {
                fragment: "carries no unit to copy",
                except: EXCEPT_INVENTS,
            },
            EnvironmentalSite {
                fragment: "has a unit that is not a dictionary",
                except: EXCEPT_ENGINE_REFUSES,
            },
            EnvironmentalSite {
                fragment: "the unit of {} holds a table",
                except: EXCEPT_ENGINE_REFUSES,
            },
            EnvironmentalSite {
                fragment: "the max_level of {} holds a table",
                except: EXCEPT_ENGINE_REFUSES,
            },
        ],
        n: 47,
        why: "the three data.raw collisions, the two named-prototype probes and the six CostOf sentences are what it asks the game; the other thirty-six read the declaration",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn check_resolved_craft_times(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 1,
        why: "the stored value it is about is gone by the time it runs, so the number left is the plan's own declared default",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn resolve_custom_cost(",
        class: CLASS_DECLARATION_IN_WORLD,
        except: "",
        sites: &[],
        n: 2,
        why: "it names a World for the packs it walks, and both refusals it builds are about a DECLARED default the settings stage would already have refused; the Go mirror keeps the pair in a resolution method of its own, which names no World at all",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn read_dropdown(",
        class: CLASS_ENVIRONMENTAL,
        except: EXCEPT_UNREACHABLE,
        sites: &[],
        n: 1,
        why: "a stored value the dropdown does not offer",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn validate_ingredients(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 9,
        why: "the declared ingredient list",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn validate_no_duplicates(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 1,
        why: "the declared ingredient list",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn validate_custom_cost(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 7,
        why: "the declared settings a custom cost names",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn validate_unit(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 9,
        why: "the declared unit",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn validate_no_duplicate_packs(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 1,
        why: "the declared pack list",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn research_number_maximum(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 1,
        why: "the declared range of a research number",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn matches_allowed_values(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 3,
        why: "the declared values of a dropdown against the choices declared beside it",
    },
    RefusalClass {
        module: "data.rs",
        func: "fn check_extra(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 3,
        why: "the declared Extra fields",
    },
    RefusalClass {
        module: "settings.rs",
        func: "fn plan_settings(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 2,
        why: "the settings stage is handed a Named and not a World: data.raw does not exist yet, so there is no game here to ask",
    },
    RefusalClass {
        module: "settings.rs",
        func: "fn validate_settings(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 15,
        why: "the declared settings",
    },
    RefusalClass {
        module: "settings.rs",
        func: "fn validate_bindings(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 9,
        why: "the declared bindings",
    },
    RefusalClass {
        module: "settings.rs",
        func: "fn validate_text_settings(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 2,
        why: "the declared text settings",
    },
    RefusalClass {
        module: "settings.rs",
        func: "fn validate_declared_packs(",
        class: CLASS_DECLARATION,
        except: "",
        sites: &[],
        n: 3,
        why: "the declared pack list of a packs setting",
    },
    RefusalClass {
        module: "ingredient_list.rs",
        func: "",
        class: CLASS_NOT_A_REFUSAL,
        except: "",
        sites: &[],
        n: 26,
        why: "every Err here carries a PROBLEM string, which becomes a player's fallback line and never stops a load; the Go half returns a plain string in the same places and builds no refusal at all",
    },
];

/// The same table, written the other way round: every row's CLASS and every
/// exception it claims, grouped BY CLASS.
///
/// WHY A SECOND LIST AT ALL. `REFUSAL_CLASSES` is a table a reader edits while
/// looking at the function they just changed, and the only thing stopping a row
/// being moved from "asks the game" to "reads only the declaration" was the
/// World check, which every item that takes a `Resolution` rather than a
/// `World` slips past: `check_resolved_craft_times` takes `res` alone, so one
/// word could make the whole rule stop guarding such an item with the suite
/// green. Now a relabel needs TWO edits in two
/// places, and the second place is grouped by CLASS, so the row has to be
/// carried out of one group and into another where a reviewer reading the diff
/// sees it.
///
/// IT PINS THE EXCEPTIONS TOO, both the row's own and its sites', in order. The
/// table above only ever asked whether an exception was ONE OF the enumerated
/// four; which of the four a site claims is the whole content of the
/// classification, and it was unpinned.
struct PolicyRow {
    module: &'static str,
    func: &'static str,
    class: &'static str,
    except: &'static str,
    /// The row's environmental site exceptions, in the order the table above
    /// lists them, joined by " | ". Empty where there are none.
    sites: &'static str,
}

const REFUSAL_POLICY: &[PolicyRow] = &[
    // Reads only the declaration. None of these may name a World at all.
    PolicyRow { module: "data.rs", func: "fn check_resolved_craft_times(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "data.rs", func: "fn validate_ingredients(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "data.rs", func: "fn validate_no_duplicates(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "data.rs", func: "fn validate_custom_cost(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "data.rs", func: "fn validate_unit(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "data.rs", func: "fn validate_no_duplicate_packs(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "data.rs", func: "fn research_number_maximum(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "data.rs", func: "fn matches_allowed_values(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "data.rs", func: "fn check_extra(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "settings.rs", func: "fn plan_settings(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "settings.rs", func: "fn validate_settings(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "settings.rs", func: "fn validate_bindings(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "settings.rs", func: "fn validate_text_settings(", class: CLASS_DECLARATION, except: "", sites: "" },
    PolicyRow { module: "settings.rs", func: "fn validate_declared_packs(", class: CLASS_DECLARATION, except: "", sites: "" },
    // Names a World and still reads only the declaration, which is enforced
    // both ways: the World has to be there, and no site may claim an exception.
    PolicyRow { module: "data.rs", func: "fn resolve_custom_cost(", class: CLASS_DECLARATION_IN_WORLD, except: "", sites: "" },
    // Asks the game, and every refusal in it is kept by the one exception named
    // here.
    PolicyRow { module: "cycle.rs", func: "fn check_cycles(", class: CLASS_ENVIRONMENTAL, except: EXCEPT_ENGINE_REFUSES, sites: "" },
    PolicyRow { module: "data.rs", func: "fn read_dropdown(", class: CLASS_ENVIRONMENTAL, except: EXCEPT_UNREACHABLE, sites: "" },
    // Holds both, and the exceptions are its sites', in the order the table
    // above lists them.
    PolicyRow { module: "data.rs", func: "fn plan_data(", class: CLASS_MIXED, except: "", sites: "there is nothing declared to degrade to, so the library would have to invent a value" },
    PolicyRow {
        module: "data.rs",
        func: "fn validate(",
        class: CLASS_MIXED,
        except: "",
        sites: "degrading silently clobbers a stranger's prototype | degrading silently clobbers a stranger's prototype | degrading silently clobbers a stranger's prototype | there is nothing declared to degrade to, so the library would have to invent a value | there is nothing declared to degrade to, so the library would have to invent a value | there is nothing declared to degrade to, so the library would have to invent a value | there is nothing declared to degrade to, so the library would have to invent a value | there is nothing declared to degrade to, so the library would have to invent a value | degrading hands the engine something it refuses with a worse message | degrading hands the engine something it refuses with a worse message | degrading hands the engine something it refuses with a worse message",
    },
    // Builds no refusal at all.
    PolicyRow { module: "ingredient_list.rs", func: "", class: CLASS_NOT_A_REFUSAL, except: "", sites: "" },
];

/// The relabel guard. It says nothing about the source; it holds the two tables
/// to each other, which is what makes a one-word class change a red suite.
#[test]
fn the_two_classification_lists_agree() {
    let mut problems: Vec<String> = Vec::new();
    for rc in REFUSAL_CLASSES {
        let Some(pr) = REFUSAL_POLICY
            .iter()
            .find(|pr| pr.module == rc.module && pr.func == rc.func)
        else {
            problems.push(format!(
                "{} in {} is classified \"{}\" and REFUSAL_POLICY does not carry it; every row is pinned in both lists so a relabel cannot be one edit",
                rc.func, rc.module, rc.class
            ));
            continue;
        };
        if pr.class != rc.class {
            problems.push(format!(
                "{} in {}: REFUSAL_CLASSES says \"{}\" and REFUSAL_POLICY says \"{}\"; one of the two is a relabel",
                rc.func, rc.module, rc.class, pr.class
            ));
        }
        if pr.except != rc.except {
            problems.push(format!(
                "{} in {}: REFUSAL_CLASSES keeps it a refusal because \"{}\" and REFUSAL_POLICY says \"{}\"",
                rc.func, rc.module, rc.except, pr.except
            ));
        }
        let listed = rc
            .sites
            .iter()
            .map(|s| s.except)
            .collect::<Vec<&str>>()
            .join(" | ");
        if listed != pr.sites {
            problems.push(format!(
                "{} in {}: the site exceptions are\n {}\nand REFUSAL_POLICY pins\n {}",
                rc.func, rc.module, listed, pr.sites
            ));
        }
    }
    for pr in REFUSAL_POLICY {
        if !REFUSAL_CLASSES
            .iter()
            .any(|rc| rc.module == pr.module && rc.func == pr.func)
        {
            problems.push(format!(
                "{} in {} is pinned in REFUSAL_POLICY and REFUSAL_CLASSES does not carry it; a row deleted from one list is deleted from both",
                pr.func, pr.module
            ));
        }
    }
    if REFUSAL_POLICY.len() != REFUSAL_CLASSES.len() {
        problems.push(format!(
            "the two classification lists hold {} and {} rows; they are the same rows written twice",
            REFUSAL_POLICY.len(),
            REFUSAL_CLASSES.len()
        ));
    }
    assert!(
        problems.is_empty(),
        "the two classification lists disagree, so a relabel could ship unseen:\n{}",
        problems.join("\n")
    );
}

#[test]
fn every_refusal_site_is_classified() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let sources = crate_modules(&root);
    assert!(
        sources.len() >= 5,
        "only {} modules found under {}; the scan would prove nothing",
        sources.len(),
        root.display()
    );

    let mut problems: Vec<String> = Vec::new();
    let mut used = alloc::vec![0usize; REFUSAL_CLASSES.len()];
    for path in &sources {
        let text = fs::read_to_string(path).expect("a module is not readable");
        let walked = scan(&text);
        let literals = walked.literals;
        let code: Vec<char> = walked.code.chars().collect();
        let module = path
            .strip_prefix(&root)
            .expect("a module outside the tree that produced it")
            .to_string_lossy()
            .to_string();

        let mut hits: Vec<usize> = Vec::new();
        for needle in REFUSAL_NEEDLES {
            hits.extend(needle_hits(&code, needle));
        }
        if hits.is_empty() {
            continue;
        }
        hits.sort_unstable();

        let rows: Vec<(usize, &RefusalClass)> = REFUSAL_CLASSES
            .iter()
            .enumerate()
            .filter(|(_, rc)| rc.module == module)
            .collect();
        if rows.is_empty() {
            problems.push(format!(
                "{}: builds {} refusal(s) at lines {:?} and no row classifies the module",
                module,
                hits.len(),
                lines_of(&code, &hits)
            ));
            continue;
        }

        for (i, rc) in &rows {
            // A WHOLE-MODULE ROW OWNS EVERY HIT IN IT, which is the only way
            // CLASS_NOT_A_REFUSAL can be stated at all.
            if rc.func.is_empty() {
                used[*i] = hits.len();
                continue;
            }
            let Some(span) = body_span(&code, rc.func) else {
                problems.push(format!(
                    "{}: has no `{}` for its refusals to sit in; it is meant to hold {}",
                    module, rc.func, rc.why
                ));
                continue;
            };
            let mine: Vec<usize> = hits
                .iter()
                .copied()
                .filter(|h| *h > span.0 && *h < span.1)
                .collect();
            used[*i] = mine.len();

            if rc.class == CLASS_DECLARATION && signature_of(&code, rc.func).contains("World") {
                problems.push(format!(
                    "{}: `{}` is classified \"{}\" and names a World; a check that can ask the game is environmental and needs an exception",
                    module, rc.func, CLASS_DECLARATION
                ));
            }
            // AND THE OTHER WAY ROUND FOR THE ONE CLASS THAT CLAIMS BOTH. A row
            // that says "names a World and still reads only the declaration"
            // has to name one; without the World it is plain
            // `CLASS_DECLARATION` and should be held to that check instead.
            if rc.class == CLASS_DECLARATION_IN_WORLD
                && !signature_of(&code, rc.func).contains("World")
            {
                problems.push(format!(
                    "{}: `{}` is classified \"{}\" and names no World; say \"{}\" instead, which is the enforced one",
                    module, rc.func, CLASS_DECLARATION_IN_WORLD, CLASS_DECLARATION
                ));
            }
            if rc.class != CLASS_MIXED {
                continue;
            }
            for j in 0..rc.sites.len() {
                let f = rc.sites[j].fragment;
                let found = mine
                    .iter()
                    .filter(|h| sentence_at(&code, &literals, **h).contains(f))
                    .count();
                if found != 1 {
                    problems.push(format!(
                        "{}: `{}`: the environmental fragment {:?} names {} refusals; it has to name exactly one",
                        module, rc.func, f, found
                    ));
                }
            }
        }

        // EVERY HIT INSIDE SOME ROW'S BODY, which is what makes a refusal added
        // to a function nobody listed a red suite rather than a silent policy
        // change.
        let whole_module = rows.iter().any(|(_, rc)| rc.func.is_empty());
        if whole_module {
            continue;
        }
        for h in &hits {
            let inside = rows.iter().any(|(_, rc)| {
                body_span(&code, rc.func).is_some_and(|(from, to)| *h > from && *h < to)
            });
            if !inside {
                problems.push(format!(
                    "{}:{}: builds a refusal inside no classified item; add a row to REFUSAL_CLASSES saying whether it reads only the declaration or asks the game, and if it asks the game which of the enumerated exceptions keeps it a refusal",
                    module,
                    line_of(&code, *h)
                ));
            }
        }
    }

    for (i, rc) in REFUSAL_CLASSES.iter().enumerate() {
        if used[i] != rc.n {
            problems.push(format!(
                "{} in {} builds {} refusals and the row pins {}; classify the one that moved ({})",
                rc.func, rc.module, used[i], rc.n, rc.why
            ));
        }
        if (rc.class == CLASS_ENVIRONMENTAL) != !rc.except.is_empty() {
            problems.push(format!(
                "{} in {} is classified \"{}\" and its exception does not agree with that",
                rc.func, rc.module, rc.class
            ));
        }
        if !rc.except.is_empty() && !ALLOWED_EXCEPTIONS.contains(&rc.except) {
            problems.push(format!(
                "{} in {} claims an exception that is not one of the enumerated ones",
                rc.func, rc.module
            ));
        }
        for s in rc.sites {
            if !ALLOWED_EXCEPTIONS.contains(&s.except) {
                problems.push(format!(
                    "{} in {}: the site {:?} claims an exception that is not one of the enumerated ones",
                    rc.func, rc.module, s.fragment
                ));
            }
        }
        // A MIXED ROW WITH NO SITES ASSERTS NOTHING, which is the third way a
        // relabel could have got past this test: CLASS_MIXED skips the World
        // check and the site walk both, so an all-environmental item moved into
        // it with an empty list would be unconstrained. A mixed row names the
        // refusals that ask the game, one by one, and by construction it holds
        // at least one that does not.
        if rc.class == CLASS_MIXED && rc.sites.is_empty() {
            problems.push(format!(
                "{} in {} is classified \"{}\" and lists no environmental site; a mixed row names the refusals that ask the game, one by one",
                rc.func, rc.module, CLASS_MIXED
            ));
        }
        if rc.class == CLASS_MIXED && rc.n <= rc.sites.len() {
            problems.push(format!(
                "{} in {} is classified \"{}\" with {} refusals and {} of them environmental; a row whose refusals ALL ask the game is \"{}\" with one exception, not mixed",
                rc.func, rc.module, CLASS_MIXED, rc.n, rc.sites.len(), CLASS_ENVIRONMENTAL
            ));
        }
        if rc.func.is_empty() && rc.class != CLASS_NOT_A_REFUSAL {
            problems.push(format!(
                "{}: a whole-module row may only say \"{}\"",
                rc.module, CLASS_NOT_A_REFUSAL
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "a refusal site is unclassified, so a policy change could ship with every behavioural test green:\n{}",
        problems.join("\n")
    );
}

/// What builds a refusal in this crate: the two `Err` constructors the planner
/// uses, the one it borrows from the crafting-time rule, and
/// `Resolution::refuse`. A re-raise of a message another site composed is not
/// one of them: it carries no sentence of its own.
const REFUSAL_NEEDLES: &[&str] = &[
    "Err(format!(",
    "Err(String::from(",
    "Err(declared_craft_time_problem(",
    ".refuse(",
];

/// The signature of the item written as `needle`: everything from the needle to
/// the brace its body opens with, which is where a `World` parameter is spelled
/// if there is one.
fn signature_of(code: &[char], needle: &str) -> String {
    let Some(at) = needle_hits(code, needle).first().copied() else {
        return String::new();
    };
    let mut i = at;
    while i < code.len() && code[i] != '{' {
        i += 1;
    }
    code[at..i].iter().collect()
}

/// The string literals of the refusal that starts at `at`, joined: a message
/// built from six pieces is six literals in one call, and joining them is what
/// lets a fragment name a site without this test knowing how the sentence was
/// assembled.
///
/// THE LITERALS COME FROM THE OTHER HALF OF THE SAME SCAN, because the blanked
/// code is exactly the half with the literals taken out of it. The call's span
/// is its own parentheses, matched the way `body_span` matches braces, and the
/// literals are the ones whose LINES fall inside it.
fn sentence_at(code: &[char], literals: &[(usize, String)], at: usize) -> String {
    let mut i = at;
    while i < code.len() && code[i] != '(' {
        i += 1;
    }
    let mut depth = 0usize;
    let mut end = code.len();
    while i < code.len() {
        match code[i] {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let (first, last) = (line_of(code, at), line_of(code, end));
    let mut out = String::new();
    for (line, text) in literals {
        if *line >= first && *line <= last {
            out.push_str(text);
        }
    }
    out
}
