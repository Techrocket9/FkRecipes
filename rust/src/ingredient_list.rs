//! THE INGREDIENT LIST: the little language a player types into a startup
//! setting, and the only place a name a PLAYER wrote turns into an
//! ingredient.
//!
//! The player-facing reference is docs/ingredient-list.md and the contract is
//! testdata/ingredient-list/cases.txt, which both language halves read and
//! run case for case. This module is the Rust side of that contract: what is
//! written here is a message the corpus already spells, so a sentence changed
//! here without the corpus is a red suite in two languages.
//!
//! REFUSAL, NEVER FALLBACK, AND IT IS A RULE ABOUT THE LANGUAGE. A typed name
//! the game does not have is answered by naming the setting, the entry and the
//! problem, with the case-folded or dash-folded name offered when the game has
//! THAT. Nothing here substitutes a name, picks a nearest match or drops an
//! entry it cannot resolve: a silent substitute would hide the player's typo
//! behind a recipe they did not ask for. The author's own declared lists keep
//! their presence ladders; those are a modpack tolerance the author chose, and
//! this language is not that path.
//!
//! WHAT THE CALLER DOES WITH THE REFUSAL IS NOT THIS MODULE'S RULE. A refused
//! text no longer stops the load: `data.rs` logs the sentence written here
//! inside one ERROR line and takes the author's declared list instead, because
//! a refusal on a field the player types into locks them out of their save
//! (the client measurement is in `player_fallback`). That changes nothing
//! above: this module still refuses rather than guessing, which is what makes
//! the line the player reads name the real problem.
//!
//! THE RESERVED WORD `default` IS WHAT THE SETTING SHIPS WITH, and it means
//! the mod's own list with its ladders, in this release and in every later
//! one. That is why the parse result is a two-armed thing rather than a list:
//! a player who never opened the settings screen has the word stored (the
//! engine writes every setting's current value into mod-settings.dat,
//! untouched defaults included), and reading it as a list of names would turn
//! them into an edited-text player the day the author changes the list.
//!
//! THE DIAGNOSIS ORDER IS FIXED, and it is the reason this reads as a
//! sequence of small checks rather than one pass: two halves in two languages
//! must say the SAME thing about an entry with three problems in it. The
//! whole text first (not text, too long, empty, the reserved words, the empty
//! entries), then per entry: the pieces left to right with the first bad one
//! quoted, then the shape of the entry (no name, two amounts, two names, the
//! signs), then the amount's own range, then what the name resolves to, then
//! the rules that need both (a fluid in a crafting recipe, a fraction on an
//! item, the fluid ceiling), then duplicates across entries.
//!
//! ONE POLICY FOR A CHARACTER THE PLAYER CANNOT SEE, and it is refusal.
//! Whitespace separates; every other member of the invisible set is refused by
//! its code point wherever it sits, and nothing is deleted from a stored text
//! on the player's behalf. Where a message quotes a piece of that text, every
//! invisible character in it is written as `U+XXXX`, so a quoted word is the
//! word on the player's screen or it names what is not there.
//!
//! NOTHING HERE READS THE ENGINE except through [`World`], and every question
//! it asks is a presence probe. The whole module is host-testable for exactly
//! that reason.

use alloc::borrow::Cow;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

use crate::data::takes_items_only;
use crate::plan::Amount;
use crate::value::{MAX_FLUID_AMOUNT, MAX_ITEM_AMOUNT};
use crate::world::World;

/// The word that means a list with nothing in it. A recipe may be free to
/// craft, so it says so on purpose; research may not, because whether a unit
/// with no packs can be completed in a game was not measured and a headless
/// probe cannot measure it.
const NONE: &str = "none";

/// The word that means the mod's own declared list. It is the setting's
/// default value and the rendering of [`ListText::Default`].
pub(crate) const DEFAULT: &str = "default";

/// The longest text this language will look at, in Unicode scalars.
///
/// MEASURED: a stored value of 98,000 characters reaches the guest intact, so
/// without this a paste of somebody's log file would be quoted back whole in
/// a refusal. Nothing an ingredient list has to say needs more than this.
///
/// WHAT THE CEILING BOUNDS IS THE REFUSAL, and [`quotable`] now sets the rate:
/// an invisible character costs its `U+XXXX` token rather than its own bytes,
/// so the bound had to be re-measured when the escaping landed. MEASURED at
/// exactly 2000 characters, which is the widest text this rule lets through:
/// 2000 x U+E0001 refuses in 14129 bytes and 2000 x U+200B in 12128. Both
/// halves build the same message, byte for byte, which is what the corpus
/// pins, and the Go twin is where the two numbers are re-taken:
///
/// ```text
/// cd go && go test -run 'TestWholeTextRules/length' -v
/// ```
///
/// A dozen kilobytes is a load failure a player can still read; the
/// 98,000-character paste this rule turns away would have been most of a
/// megabyte.
///
/// THE QUOTED ENTRY IS NOT TRUNCATED, deliberately. A truncation rule would be
/// written here to be unwound: this round moves a refused text out of the
/// error dialog and into the log, and the dialog is the only place the size of
/// the quotation was ever the problem.
///
/// IT IS `pub(crate)` BECAUSE THE DESCRIPTION QUOTES IT. `text_format_line`
/// tells the player the ceiling in words, and a digit typed there instead
/// would be a promise the parser could stop keeping. A constant is not a
/// function: naming it outside this module links no parser and no renderer, so
/// it is not part of the seam `SEAM` guards.
pub(crate) const MAX_TEXT: usize = 2000;

/// U+00D7 MULTIPLICATION SIGN. A player whose keyboard or autocorrect
/// produces it means what `x` means, and it cannot appear in a prototype name
/// (measured: the engine allows A-Z a-z 0-9 _- and nothing else), so it can
/// always split.
const TIMES: char = '\u{00d7}';

/// Which list is being read, which decides what a name may resolve to and
/// what `none` means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ListKind {
    /// A recipe's ingredients: items and fluids, and `none` is the empty
    /// list.
    Recipe,
    /// A research unit's science packs: tool-type items only, and `none` is
    /// refused.
    Packs,
}

/// What a player's text says.
///
/// TWO ARMS, NOT A LIST WITH A FLAG: the word `default` names no ingredient at
/// all, and a caller that has to bind one or the other cannot forget which it
/// is holding.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum ListText {
    /// The reserved word `default`: the mod's own declared list, ladders and
    /// all, whatever the author's list becomes in a later release.
    Default,
    /// A list the player wrote out, resolved. Empty is the word `none`.
    List(IngredientList),
}

/// A parsed list, in the order it was typed.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct IngredientList {
    pub(crate) entries: Vec<ListEntry>,
}

/// One resolved entry. The name exists in the game as loaded, the amount is
/// legal for its kind, and the kind is the amount's own variant.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct ListEntry {
    pub(crate) name: String,
    pub(crate) amount: Amount,
}

/// THE LANGUAGE AS A TABLE OF POINTERS, and the reason this module has one.
///
/// A PLAN THAT DECLARES NO TEXT SETTING MUST NOT SHIP THE LANGUAGE. This crate
/// is compiled into the CONSUMER's wasm and packaged into Lua the player
/// downloads, so every function that survives link-time elimination is weight
/// in somebody's mod, and a direct call is a reference elimination has to keep
/// whether or not any declaration in the plan could ever reach it.
///
/// MEASURED, on `rust/examples/notext`, the fixture kept for exactly this
/// question: a plan shaped like the pilot's before its customizer round, two
/// dropdowns driving `ingredients_by` and `cost_by` over legacy prototypes and
/// no text setting anywhere, built for wasm32-unknown-unknown at
/// `opt-level = "s"` with LTO and packaged by `fklua mod`. With the planners
/// naming these functions it packaged 84,959 lines of fk_data_module.lua;
/// with this table in front of them, 59,491, the language's own functions
/// gone from the packaged module's headers. That pair was measured at c7a806e
/// against an `fklua` at a1fcd04 and belongs to that head: the CURRENT figures
/// for all four guests are the table under "The claim gets its adjective" in
/// Fix round 1b of `agents/implementation-notes.md`, with a runnable recipe
/// beside them. LINES rather than bytes, because rustc writes a source path
/// into the wasm's panic locations and the packaged module's byte total moves
/// by a few hundred bytes with wherever the checkout sits; both subsections
/// carry the byte totals with the path they were taken under.
///
/// The guest that DOES declare text settings paid for the indirection, and at
/// that same head the whole bill was 0.6%: `rust/examples/datastage` went from
/// 3,656,235 bytes of fk_data_module.lua to 3,679,378, and from 89,943 lines
/// to 90,520. The question came from the pilot, which measured its own
/// packaged data module at 4,904,124 bytes at its round-three head, WITH its
/// text settings declared; that is a different plan from the fixture above and
/// not the same measurement.
///
/// So the planners reach the language through this table and never by name.
/// It is installed by the two constructors that declare a text setting,
/// `Lib::ingredients_decl` and `Lib::packs_decl`, which name `LANGUAGE` and
/// are the only places in the crate outside this module that name anything
/// here at all; a plan that calls neither carries `None`, refers to nothing
/// here, and links none of it. `Lib::validate_text_settings` is the
/// guard: a text setting that arrived without its table is refused rather than
/// dereferenced, and `tests::source` holds the naming rule over the source
/// itself, because one direct call put back next year would ship all of this
/// again with every other test still green.
pub(crate) struct Language {
    pub(crate) parse: ParseFn,
    pub(crate) is_edited: fn(&[u8], ListKind, &str, &str, &dyn World) -> bool,
    pub(crate) render: fn(&ListText) -> String,
    pub(crate) render_list: fn(&IngredientList) -> String,
    pub(crate) format_amount: fn(f64) -> String,
}

/// The parser's own signature, named rather than spelled in the field above
/// because it is the one signature here long enough that clippy calls it
/// complex.
pub(crate) type ParseFn = fn(&[u8], ListKind, &str, &str, &dyn World) -> Result<ListText, String>;

/// The one table there is. See [`Language`] for why it is reached by pointer.
pub(crate) static LANGUAGE: Language = Language {
    parse,
    is_edited,
    render,
    render_list,
    format_amount,
};

/// Which tag, if any, the player wrote around a name. The game's own rich
/// text is the form that disambiguates an item from a fluid of the same name
/// and the form that reaches a name which would otherwise lex as an amount.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tag {
    Untagged,
    Item,
    Fluid,
}

/// One lexed piece of an entry.
#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Amount(String),
    /// The sign AS WRITTEN, because the refusal quotes it back: a player who
    /// typed `*` should not be told about an `x` they never wrote.
    Sign(String),
    Name(String, Tag),
}

/// Reads a player's text into a resolved list or the default marker, or
/// refuses with the whole message.
///
/// IT TAKES BYTES, because that is what a stored setting IS: fkdata hands both
/// halves the engine's own bytes and rewrites nothing, so the decision about
/// whether they are text belongs to the reader, and this is the reader.
///
/// `setting` is the setting's FULL name, which every message carries;
/// `category` is the recipe's declared category, which decides whether a
/// fluid may appear at all and is ignored for [`ListKind::Packs`].
///
/// The Err is the complete sentence, ready for `fkdata::raise`, and carries
/// NO stage of its own: the host prefixes the stage.
pub(crate) fn parse(
    text: &[u8],
    kind: ListKind,
    category: &str,
    setting: &str,
    w: &dyn World,
) -> Result<ListText, String> {
    // NOT TEXT, FIRST AND WHOLE, AND THE BYTES ARE WHAT SAY SO. This is the
    // exact mirror of the Go half's utf8.ValidString: fkdata delivers what the
    // engine holds, so a mod-settings.dat somebody edited by hand arrives here
    // as the invalid sequence itself and the two halves answer one question
    // rather than two. It costs a player nothing, because the settings screen
    // cannot produce such a value.
    //
    // U+FFFD IS AN ORDINARY CHARACTER HERE and no longer a proxy for one. The
    // old check refused any text containing the replacement character, because
    // the lossy decode this half used to sit behind was the only shape invalid
    // bytes could take; now that nothing rewrites them, a player who pasted a
    // real U+FFFD is answered by the ordinary character rules, which quote it.
    // WHICH of those rules answers depends on where it sits, exactly as it does
    // for any other character: bare in an entry it is the names rule (measured:
    // `"\u{fffd}" has no place here; names use the letters a to z, ...`), inside
    // a tag it is the tag rule (`a tag is [item=name] or [fluid=name]`), and a
    // text whose first problem is elsewhere takes that problem instead. This is
    // the answer the Go half always gave and which the corpus pins.
    let text = match core::str::from_utf8(text) {
        Ok(t) => t,
        Err(_) => {
            return Err(format!(
                "fkrecipes: {} contains characters that are not text; retype the list",
                setting
            ))
        }
    };
    if text.chars().count() > MAX_TEXT {
        return Err(format!(
            "fkrecipes: {} is longer than {} characters; that is not an ingredient list",
            setting, MAX_TEXT
        ));
    }

    // THE PARSER TRIMS FOR ITSELF, here and again around every entry. A
    // stored text arrives verbatim (measured: a setting with auto_trim = true
    // still reads back "  3 iron-plate , 0.5 [fluid=water]  " with both space
    // runs, because auto_trim is a GUI behaviour and does not touch what
    // mod-settings.dat holds), so there is nowhere else this can happen.
    //
    // NOTHING IS REMOVED FROM THE TEXT, only trimmed off its ends. A BOM or a
    // zero-width joiner carried in by a copy from a wiki page used to be
    // deleted here; it is refused by its code point now, with every other
    // character the player cannot see, because a deletion answered a text
    // nobody typed and a refusal quoting the result showed them a word that
    // was not on their screen.
    let whole = trim_ws(text);
    if whole.is_empty() {
        return Err(match kind {
            ListKind::Recipe => format!(
                "fkrecipes: {} is empty; write the ingredients as \"2 iron-plate, 3 copper-cable\", the word default for the mod's own list, or the word none for a recipe with no ingredients",
                setting
            ),
            ListKind::Packs => format!(
                "fkrecipes: {} is empty; write the science packs as \"1 automation-science-pack, 1 logistic-science-pack\", or the word default for the mod's own list",
                setting
            ),
        });
    }
    if whole.eq_ignore_ascii_case(DEFAULT) {
        return Ok(ListText::Default);
    }

    // ONE TRAILING COMMA IS TOLERATED, because a list a player is still
    // editing ends in one and there is nothing ambiguous about it. A second
    // one is an empty entry like any other.
    //
    // THE SPLIT COMES FIRST, and that order is the rule rather than an
    // accident of writing: a comma separates entries only OUTSIDE a tag, so
    // the tolerated one cannot be recognised until the entries exist. Taking
    // it off the whole text beforehand would pull the comma out of an
    // unclosed "[item=iron-plate," and answer about a tag nobody wrote.
    let mut raw = split_entries(whole);
    if raw.len() > 1 && raw[raw.len() - 1].is_empty() {
        raw.pop();
    }

    // EMPTY ENTRIES FIRST, ALL OF THEM, before any other question about any
    // entry: two commas in a row is a typing accident, and a text that also
    // holds a reserved word or a misspelled name should say so about the
    // accident rather than about whatever the halves happen to check next.
    for (i, e) in raw.iter().enumerate() {
        if e.is_empty() {
            return Err(format!(
                "fkrecipes: {}, entry {} is empty; one comma separates two ingredients",
                setting,
                i + 1
            ));
        }
    }

    // THE RESERVED WORDS, positionally, before per-entry diagnosis: "none, 2
    // iron.plate" is answered about the word rather than about the dot,
    // because the word is what makes the rest of the list meaningless.
    //
    // MATCHED WITHOUT ASCII CASE, because they are this language's own English
    // keywords and not names the game has to carry: a player typing into a
    // settings field types None as readily as none. The sentence names the
    // word in the one spelling the language has rather than echoing the
    // player's capitals, so both halves say it the same way.
    for e in &raw {
        let word = if e.eq_ignore_ascii_case(NONE) {
            NONE
        } else if e.eq_ignore_ascii_case(DEFAULT) {
            DEFAULT
        } else {
            continue;
        };
        if raw.len() > 1 {
            return Err(format!(
                "fkrecipes: {}: {} stands alone; remove the other entries or the word",
                setting, word
            ));
        }
        if word == DEFAULT {
            return Ok(ListText::Default);
        }
        return match kind {
            ListKind::Recipe => Ok(ListText::List(IngredientList {
                entries: Vec::new(),
            })),
            ListKind::Packs => Err(format!(
                "fkrecipes: {}: research takes at least one science pack",
                setting
            )),
        };
    }

    let mut entries: Vec<ListEntry> = Vec::with_capacity(raw.len());
    for (i, text) in raw.iter().enumerate() {
        // The NEXT entry is passed in for one rule and one only: an amount
        // alone followed by an entry starting with a digit is a decimal or
        // thousands comma, and saying so needs to see across the comma.
        let next = raw.get(i + 1).copied();
        match one_entry(text, next, kind, category, w) {
            Ok(entry) => entries.push(entry),
            // The entry is quoted AS TYPED, trimmed, and numbered from 1.
            // Character offsets are fragile across two languages' UTF-8
            // handling; an entry number with its text is exact and stable.
            Err(problem) => {
                return Err(format!(
                    "fkrecipes: {}, entry {} (\"{}\"): {}",
                    setting,
                    i + 1,
                    quotable(text),
                    problem
                ))
            }
        }
    }

    // Duplicates last, and by KIND AND NAME both: the engine refuses a
    // repeated ingredient (measured: "Duplicate item ingredients are not
    // allowed (iron-plate exists 2 or more times).") but an item and a fluid
    // that share a name are two different ingredients. The pair reported is
    // the first in reading order, which is what makes it the same pair in
    // both halves.
    for j in 1..entries.len() {
        for i in 0..j {
            if entries[i].name == entries[j].name
                && entries[i].amount.is_fluid() == entries[j].amount.is_fluid()
            {
                return Err(format!(
                    "fkrecipes: {}: entries {} and {} both name {}",
                    setting,
                    i + 1,
                    j + 1,
                    entries[j].name
                ));
            }
        }
    }

    Ok(ListText::List(IngredientList { entries }))
}

/// Whether a stored text says anything other than "the mod's own list".
///
/// IT IS THE PARSER'S OWN ANSWER, and that is the point: the planner is about
/// to IGNORE this text, because the dropdown beside it sits on a preset, and
/// the line it writes has to agree with the reading the data path would have
/// given the same bytes. A comparison against the bare word is a second answer
/// to that question and drifts from it: `default,` carries the tolerated
/// trailing comma this language accepts everywhere else, and telling the
/// player their untouched field is edited sends them looking for an edit they
/// never made.
///
/// THE REFUSAL IS DISCARDED, deliberately: a text that does not parse is not
/// the word, so it is an edit, and it stays one log line rather than a load
/// failure over a list nothing was going to read.
pub(crate) fn is_edited(
    text: &[u8],
    kind: ListKind,
    category: &str,
    setting: &str,
    w: &dyn World,
) -> bool {
    !matches!(
        parse(text, kind, category, setting, w),
        Ok(ListText::Default)
    )
}

/// Writes a parse result back out in the canonical form, which is what a log
/// line prints and what a setting's description carries.
///
/// It is the INVERSE of [`parse`]: rendering and parsing the result gives the
/// same thing back, and both halves test that as a property over generated
/// lists.
pub(crate) fn render(text: &ListText) -> String {
    match text {
        ListText::Default => String::from(DEFAULT),
        ListText::List(list) => render_list(list),
    }
}

/// The list arm of [`render`].
pub(crate) fn render_list(list: &IngredientList) -> String {
    if list.entries.is_empty() {
        return String::from(NONE);
    }
    let mut out = String::new();
    for (i, e) in list.entries.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        match e.amount {
            Amount::Item(n) => {
                out.push_str(&format!("{} {}", n, item_name(&e.name)));
            }
            // A fluid ALWAYS takes its tag, so the reader never has to know
            // which of the two namespaces a bare name would have hit.
            //
            // The amount goes through this language's own decimal rule and
            // not through Display: see [`format_amount`] for why neither
            // platform's shortest printer could be the rule.
            Amount::Fluid(v) => {
                out.push_str(&format!("{} [fluid={}]", format_amount(v), e.name));
            }
        }
    }
    out
}

/// An item's name as it can be written: plain where the language would read
/// it back as a name, tagged where it would not. `[item=42]`, `[item=x]`,
/// `[item=2x4]`, `[item=none]` and `[item=default]` are all names the engine
/// allows (measured: item("42"), item("x") and item("none") all load) and all
/// unreachable without the tag.
fn item_name(name: &str) -> String {
    if lexes_as_a_plain_name(name) {
        return String::from(name);
    }
    format!("[item={}]", name)
}

/// THE DECIMAL RULE: how this language writes a fluid amount, and the only
/// decimal rule it has. The research log line's seconds join it next.
///
/// NEITHER PLATFORM'S SHORTEST PRINTER, because the two disagree. Go's
/// `strconv.FormatFloat(v, 'f', -1, 64)` and Rust's `Display` both print a
/// shortest decimal that reads back to the same double, and on an exact tie
/// they break the last digit differently: the review found 129 divergences in
/// 204,105 doubles, the smallest at 1.0000076293945312, and the corpus pins
/// that one and 1059438285926254.2 for exactly this reason. Two halves that
/// print one amount two ways are two languages, so both halves walk this rule
/// instead: widen the precision until a candidate reads back exactly, then
/// break the tie towards the EVEN digit, a choice no platform's rounding gets
/// a vote in.
///
/// THE AMOUNT IS FINITE AND POSITIVE, and every path that reaches here has
/// already said so: the typed path refuses a non-finite amount at parse and
/// the declared path refuses one in the validators. No rendering of an
/// infinity is specified and none may be reachable; the guard below is what
/// keeps the routine total rather than trapping a consumer's wasm if one ever
/// were.
pub(crate) fn format_amount(v: f64) -> String {
    let mut digits = String::new();
    let mut exponent = 0i32;
    for p in 0..=MAX_PRECISION {
        // Correctly rounded at a FIXED precision, which is the one thing the
        // two platforms' formatters do the same way.
        let sci = format!("{:.*e}", p, v);
        let at = match sci.find('e') {
            Some(i) => i,
            // A value with no scientific form is a non-finite one. See the
            // note above: unreachable, and total anyway.
            None => return sci,
        };
        let d: String = sci[..at].chars().filter(|c| *c != '.').collect();
        exponent = sci[at + 1..].parse().unwrap_or(0);
        match qualifying_digits(&d, exponent, v) {
            Some(chosen) => {
                digits = chosen;
                break;
            }
            // Seventeen significant digits read back exactly for every finite
            // double, so the last pass always chooses. Taking D there anyway
            // makes that fact a fallback rather than a panic.
            None if p == MAX_PRECISION => {
                digits = d;
                break;
            }
            None => {}
        }
    }
    fixed_notation(&digits, exponent)
}

/// The precision the walk stops at, counted the way the formatters count it:
/// digits AFTER the leading one. Seventeen significant digits read back
/// exactly for every finite double.
const MAX_PRECISION: usize = 16;

/// The three candidates at one precision, and the tie rule that picks among
/// the ones that read back exactly.
///
/// The candidates are D with its last digit decremented, D, and D with it
/// incremented, in ascending order, and a neighbour that would borrow or
/// carry is not taken: every candidate is D's own digits with one digit
/// changed, never a different length. The choice is the first qualifying one
/// whose last digit is EVEN, D when none of them is even, and otherwise the
/// first that qualifies. `None` says this precision has nothing to offer and
/// the caller should widen it.
fn qualifying_digits(d: &str, exponent: i32, v: f64) -> Option<String> {
    let last = d.chars().next_back()?;
    let mut candidates: Vec<String> = Vec::with_capacity(3);
    if last != '0' {
        candidates.push(with_last(d, (last as u8 - 1) as char));
    }
    candidates.push(String::from(d));
    if last != '9' {
        candidates.push(with_last(d, (last as u8 + 1) as char));
    }
    let qualifying: Vec<String> = candidates
        .into_iter()
        .filter(|c| reads_back(c, exponent, v))
        .collect();
    for c in &qualifying {
        if last_digit_is_even(c) {
            return Some(c.clone());
        }
    }
    if qualifying.iter().any(|c| c == d) {
        return Some(String::from(d));
    }
    qualifying.first().cloned()
}

/// These digits with the last one replaced. The digits are ASCII, so the
/// byte split is the character split.
fn with_last(digits: &str, last: char) -> String {
    let mut out = String::from(&digits[..digits.len() - 1]);
    out.push(last);
    out
}

fn last_digit_is_even(digits: &str) -> bool {
    matches!(
        digits.chars().next_back(),
        Some('0' | '2' | '4' | '6' | '8')
    )
}

/// Whether these digits, read at this exponent, ARE v: the whole definition
/// of a candidate that qualifies.
fn reads_back(digits: &str, exponent: i32, v: f64) -> bool {
    scientific(digits, exponent).parse::<f64>() == Ok(v)
}

/// The digits as one number in scientific form: the leading digit, the rest
/// behind a dot, and the exponent.
fn scientific(digits: &str, exponent: i32) -> String {
    let mut out = String::from(&digits[..1]);
    if digits.len() > 1 {
        out.push('.');
        out.push_str(&digits[1..]);
    }
    out.push('e');
    out.push_str(&exponent.to_string());
    out
}

/// The chosen digits laid out in FIXED notation, which is the one shape this
/// language's amounts may take: a rendering has to parse back, and the parser
/// refuses an exponent.
fn fixed_notation(digits: &str, exponent: i32) -> String {
    let n = digits.len() as i32;
    let mut out = String::new();
    // The point sits at or past the last digit: an integer, with the zeros
    // the exponent asks for behind it.
    if exponent >= n - 1 {
        out.push_str(digits);
        for _ in 0..(exponent - (n - 1)) {
            out.push('0');
        }
        return out;
    }
    // The point sits inside the digits: an integer part and a fraction, with
    // a fraction's trailing zeros trimmed and an empty fraction taking its
    // dot with it.
    if exponent >= 0 {
        let at = (exponent + 1) as usize;
        out.push_str(&digits[..at]);
        let fraction = digits[at..].trim_end_matches('0');
        if !fraction.is_empty() {
            out.push('.');
            out.push_str(fraction);
        }
        return out;
    }
    // The point sits before them: a zero, a dot, and the leading zeros the
    // exponent asks for.
    out.push_str("0.");
    for _ in 0..(-exponent - 1) {
        out.push('0');
    }
    out.push_str(digits.trim_end_matches('0'));
    out
}

/// Whether writing this name bare would read back as the same name and
/// nothing else. Asked of the LEXER rather than of a list of shapes, so the
/// answer cannot drift from what the parser does: `loader-1x1` is a name,
/// `X2` is a sign and an amount, and neither fact is written down twice.
fn lexes_as_a_plain_name(name: &str) -> bool {
    // FOLDED, LIKE THE PARSER'S OWN TEST, and this is the third of the three
    // sites: an item called Default written bare would be read back as the
    // marker, so the tag is what keeps the round trip an identity. The fold is
    // an equality and not a prefix, so `defaults` is a name and stays plain.
    if name.eq_ignore_ascii_case(NONE) || name.eq_ignore_ascii_case(DEFAULT) {
        return false;
    }
    let mut toks: Vec<Tok> = Vec::new();
    match classify(name, &mut toks) {
        Ok(()) => matches!(toks.as_slice(), [Tok::Name(n, Tag::Untagged)] if n == name),
        Err(_) => false,
    }
}

/// One entry, from its text to a resolved line, or to the PROBLEM CLAUSE
/// alone: the caller owns the prefix that names the setting and the entry, so
/// nothing in here has to carry it.
fn one_entry(
    text: &str,
    next: Option<&str>,
    kind: ListKind,
    category: &str,
    w: &dyn World,
) -> Result<ListEntry, String> {
    let toks = tokens(text)?;

    let mut names: Vec<(&str, Tag)> = Vec::new();
    let mut amounts: Vec<&str> = Vec::new();
    let mut signs: Vec<&str> = Vec::new();
    for t in &toks {
        match t {
            Tok::Name(s, tag) => names.push((s.as_str(), *tag)),
            Tok::Amount(s) => amounts.push(s.as_str()),
            Tok::Sign(s) => signs.push(s.as_str()),
        }
    }

    // THE ORDER IS THE CONTRACT. Two amounts is reported before two names so
    // that "2 iron-plate 3 copper-cable" is one answer and not two.
    if names.is_empty() {
        return Err(no_name(text, next, &toks, kind, w));
    }
    if amounts.len() > 1 {
        return Err(String::from("has two amounts"));
    }
    if names.len() > 1 {
        return Err(two_names(&names, kind, w));
    }
    if signs.len() > 1 {
        return Err(format!("has more than one \"{}\"", quotable(signs[0])));
    }
    if signs.len() == 1 {
        if amounts.is_empty() {
            return Err(format!(
                "has \"{}\" with no amount beside it",
                quotable(signs[0])
            ));
        }
        if !sign_sits_between(&toks) {
            return Err(format!(
                "\"{}\" goes between the amount and the name",
                quotable(signs[0])
            ));
        }
    }

    // A missing amount means 1, which is what makes "iron-plate" a whole
    // entry. The lexer already proved the shape is digits with at most one
    // dot, so the only thing left that can surprise a parse is a number too
    // big for a double, which comes back as an infinity and is answered by
    // the fluid ceiling below.
    let value: f64 = match amounts.first() {
        None => 1.0,
        Some(written) => written.parse().unwrap_or(f64::INFINITY),
    };
    if value == 0.0 {
        return Err(String::from("the amount must be more than 0"));
    }

    let (name, tag) = names[0];
    let fluid = match kind {
        ListKind::Recipe => resolve_for_recipe(name, tag, w)?,
        ListKind::Packs => resolve_for_packs(name, tag, w)?,
    };

    if fluid {
        if takes_items_only(category) {
            return Err(format!(
                "{} is a fluid, and a recipe in the crafting category takes items only",
                name
            ));
        }
        // THE CEILING IS MEASURED, not a type's limit: above 1.0715e301 (the
        // wall is DBL_MAX / 2^24, see MAX_FLUID_AMOUNT for the bracket) the
        // engine does not refuse the load, it aborts inside
        // FixedPointNumber with "double value not in range for fixed point
        // number: inf" and takes the crash handler with it. A number too big
        // for a double lands here as an infinity and gets the same sentence,
        // because it is the same mistake one digit further on.
        if !value.is_finite() || value > MAX_FLUID_AMOUNT {
            return Err(String::from(
                "the amount is too large; fluid amounts go up to 1e301",
            ));
        }
        return Ok(ListEntry {
            name: name.to_string(),
            amount: Amount::Fluid(value),
        });
    }
    // 3.0 IS WHOLE. The engine loads a fractional item amount and dumps it as
    // written, so nothing but this refuses it, and a player who wrote a
    // trailing zero wrote a whole number.
    if !is_whole(value) {
        return Err(format!("{} is an item, and items take whole amounts", name));
    }
    if value > MAX_ITEM_AMOUNT as f64 {
        return Err(format!("{} takes at most {}", name, MAX_ITEM_AMOUNT));
    }
    Ok(ListEntry {
        name: name.to_string(),
        amount: Amount::Item(value as i64),
    })
}

/// An entry with no name in it, which is three different mistakes.
///
/// The first is the one the review found: `42`, `x`, `2x4` and `X2` are names
/// base Factorio and its mods really carry (measured: item("42"), item("x")
/// and loader-1x1 all load), and a player who typed one gets told the tag
/// that reaches it rather than a sentence about amounts. The second is the
/// decimal comma, which needs to see the NEXT entry to be sure. The third is
/// an amount with nothing after it.
fn no_name(text: &str, next: Option<&str>, toks: &[Tok], kind: ListKind, w: &dyn World) -> String {
    if let Some(tag) = reads_as_a_name(text, kind, w) {
        // The tag at the end is what the player types, and it is written as
        // typed: this arm was reached because the World has the text AS a
        // name, so the engine's own charset already says it is visible.
        return format!(
            "\"{}\" is a name that reads as an amount; write it in its tag, as [{}={}]",
            quotable(text),
            tag,
            text
        );
    }
    let one_amount = matches!(toks, [Tok::Amount(_)]);
    let next_is_digits = next.is_some_and(|n| n.starts_with(|c: char| c.is_ascii_digit()));
    if one_amount && next_is_digits {
        return String::from(
            "a comma separates two ingredients, not the digits of one number; write a dot for a fraction, as in \"0.5 water\"",
        );
    }
    String::from("has no name; write the amount before the name, as in \"2 iron-plate\"")
}

/// The tag that would reach this text as a name, for a text the game has
/// under the kind's own lookup.
fn reads_as_a_name(text: &str, kind: ListKind, w: &dyn World) -> Option<&'static str> {
    match kind {
        ListKind::Recipe => {
            if w.item_exists(text) {
                return Some("item");
            }
            if w.fluid_exists(text) {
                return Some("fluid");
            }
            None
        }
        // A science pack is an item, so the tag that reaches one is the item
        // tag; the fluid arm has nothing to answer here.
        ListKind::Packs => {
            if w.tool_exists(text) {
                Some("item")
            } else {
                None
            }
        }
    }
}

/// Two or more names in one entry, which is a missing comma or a display
/// name.
///
/// THE FOLD IS THE POINT. "2 iron plates" is what a player who knows the game
/// in English types, and telling them twice to add a comma teaches them
/// nothing; joining the words the way a prototype name is spelled, and
/// dropping the plural, lands on `iron-plate` and says so. Only when the fold
/// finds nothing does the entry get read as two ingredients, and only when
/// BOTH of those fail does it say what a name actually looks like.
fn two_names(names: &[(&str, Tag)], kind: ListKind, w: &dyn World) -> String {
    let mut spaced = String::new();
    let mut joined = String::new();
    for (i, (n, _)) in names.iter().enumerate() {
        if i > 0 {
            spaced.push(' ');
            joined.push('-');
        }
        spaced.push_str(n);
        for c in n.chars() {
            joined.push(if c == '_' {
                '-'
            } else {
                c.to_ascii_lowercase()
            });
        }
    }
    let (no_such, example) = match kind {
        ListKind::Recipe => ("no item or fluid is named", "iron-plate"),
        ListKind::Packs => ("no science pack is named", "automation-science-pack"),
    };
    let singular = joined.strip_suffix('s').map(String::from);
    for candidate in [Some(joined.clone()), singular].into_iter().flatten() {
        if exists_for(&candidate, kind, w) {
            return format!(
                "{} \"{}\"; did you mean {}",
                no_such,
                quotable(&spaced),
                candidate
            );
        }
    }
    if names.iter().all(|(n, _)| exists_for(n, kind, w)) {
        return String::from("names two ingredients; a comma separates them");
    }
    format!(
        "{} \"{}\"; names are the game's internal names, such as {}, and a comma separates two ingredients",
        no_such,
        quotable(&spaced),
        example
    )
}

/// The kind's own lookup, asked of a name that carries no tag.
fn exists_for(name: &str, kind: ListKind, w: &dyn World) -> bool {
    match kind {
        ListKind::Recipe => w.item_exists(name) || w.fluid_exists(name),
        ListKind::Packs => w.tool_exists(name),
    }
}

/// A recipe's name lookup, returning whether the name resolved to a FLUID.
///
/// AN ITEM WINS A TIE, and the tag is how a player says otherwise. Untagged
/// names hit the item family first because that is what the overwhelming
/// majority of recipes name; a name that is both is two prototypes, and
/// guessing the fluid would be a silent substitution.
fn resolve_for_recipe(name: &str, tag: Tag, w: &dyn World) -> Result<bool, String> {
    match tag {
        Tag::Untagged => {
            if w.item_exists(name) {
                return Ok(false);
            }
            if w.fluid_exists(name) {
                return Ok(true);
            }
            Err(format!(
                "no item or fluid is named {}{}",
                name,
                suggestion(name, |n| w.item_exists(n) || w.fluid_exists(n))
            ))
        }
        Tag::Item => {
            if w.item_exists(name) {
                return Ok(false);
            }
            Err(format!(
                "no item is named {}{}",
                name,
                suggestion(name, |n| w.item_exists(n))
            ))
        }
        Tag::Fluid => {
            if w.fluid_exists(name) {
                return Ok(true);
            }
            Err(format!(
                "no fluid is named {}{}",
                name,
                suggestion(name, |n| w.fluid_exists(n))
            ))
        }
    }
}

/// A pack list's name lookup. Tools only, and the two near misses get their
/// own sentences: the engine's own messages for both name neither the setting
/// nor the entry ("Invalid research unit (iron-plate). Research unit(s) can
/// only be tool type items at the moment." and, for a fluid, an assignID
/// abort about an item that does not exist).
///
/// The Ok is always false: a pack is an item, and a research unit has no
/// fluid form to be.
fn resolve_for_packs(name: &str, tag: Tag, w: &dyn World) -> Result<bool, String> {
    let no_pack = |name: &str| {
        format!(
            "no science pack is named {}{}",
            name,
            suggestion(name, |n| w.tool_exists(n))
        )
    };
    let is_fluid =
        |name: &str| format!("{} is a fluid, and research takes science packs only", name);
    // A TAGGED FLUID SKIPS THE ITEM QUESTIONS ENTIRELY: the player said which
    // namespace they meant, and answering out of the other one would be the
    // guess this language does not make. That goes for the MESSAGE as well as
    // for the lookup: a name the fluid table does not have is answered by the
    // fluid table, with a suggestion drawn from the same place, because
    // "no science pack is named water" would be an answer to a question
    // nobody asked.
    if tag == Tag::Fluid {
        if w.fluid_exists(name) {
            return Err(is_fluid(name));
        }
        return Err(format!(
            "no fluid is named {}{}",
            name,
            suggestion(name, |n| w.fluid_exists(n))
        ));
    }
    // THE ITEM TAG FALLS THROUGH LIKE A BARE NAME, tool then item then
    // fluid. `[item=water]` says which namespace the player MEANT, and the
    // game does not have the name there; the answer worth giving is the one
    // that says where the name actually lives. Only the fluid tag
    // short-circuits, above, because there the player named the namespace
    // that answers.
    if w.tool_exists(name) {
        return Ok(false);
    }
    if w.item_exists(name) {
        return Err(format!("{} is an item, not a science pack", name));
    }
    if w.fluid_exists(name) {
        return Err(is_fluid(name));
    }
    Err(no_pack(name))
}

/// The one guess this language makes, and it guesses at nothing: it offers a
/// name the game ACTUALLY HAS, reached by the two mistakes a prototype name
/// invites. Lowercasing catches a display name typed as written
/// ("Iron-Plate"); the underscore catches the other convention a player may
/// carry from another game's data. A fold that changes nothing, or that lands
/// on a name the game does not have either, offers nothing.
fn suggestion(name: &str, exists: impl Fn(&str) -> bool) -> String {
    let folded: String = name
        .chars()
        .map(|c| {
            if c == '_' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    if folded == name || !exists(&folded) {
        return String::new();
    }
    format!("; did you mean {}", folded)
}

/// The token sequence a single sign is allowed to make: it multiplies the
/// amount by the name, in either reading order, and stands between the two.
fn sign_sits_between(toks: &[Tok]) -> bool {
    matches!(
        toks,
        [Tok::Amount(_), Tok::Sign(_), Tok::Name(_, _)]
            | [Tok::Name(_, _), Tok::Sign(_), Tok::Amount(_)]
    )
}

/// What separates words and surrounds the text, measured rather than taken
/// from a Unicode class: the ASCII four that reach the guest intact, plus the
/// spaces a word processor or a phone keyboard produces where a player meant
/// a space. Everything else that looks blank is refused by its code point,
/// because a name is not allowed to hide one.
///
/// SIX OF THE EIGHT ARE IN THE INVISIBLE SET AS WELL (tab, LF, CR, U+2007,
/// U+202F and U+3000), and they separate rather than refuse because this
/// question is asked FIRST. That order is the whole difference between the two
/// sets: a character the player used as a space is one, and a character hiding
/// inside a name is the other.
fn is_ws(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'
            | '\u{000a}'
            | '\u{000d}'
            | '\u{0020}'
            | '\u{00a0}'
            | '\u{2007}'
            | '\u{202f}'
            | '\u{3000}'
    )
}

/// The characters a refusal names by CODE POINT rather than by quoting, and
/// the ones [`quotable`] rewrites inside quotation marks. It is the ONE answer
/// this language has to a character the player cannot see; nothing is deleted
/// from a text on their behalf.
///
/// CLOSED RANGES AND NO UNICODE TABLE, in both halves. This crate is compiled
/// into a consumer's wasm and its size is a measured property, so every
/// table-backed `char` method is out (and `unicode.IsPrint` with it, on the Go
/// side); the ranges below are written by hand and the corpus pins one member
/// of each. The ranges were DERIVED rather than recalled, from two local
/// Unicode tables that agree: Python 3.14's `unicodedata` at Unicode 16.0.0
/// and the Go standard library's `unicode` package at 15.0.0, which carry the
/// same 170 format characters.
///
/// WHAT IS IN IT, exactly, in three parts:
///
/// - EVERY FORMAT CHARACTER, general category `Cf`, all 170 of them. That is
///   what makes the reference's claim true rather than nearly true: the Arabic
///   number signs and letter mark, the end-of-ayah pair, the Syriac
///   abbreviation mark, the Arabic pound and piastre marks, the two Kaithi
///   number signs, the joiners and bidirectional marks, the invisible
///   operators, the byte-order mark, the interlinear annotation marks, the
///   Egyptian hieroglyph format controls, the shorthand format controls, the
///   musical beams, ties, slurs and phrases, and the language tags.
/// - EVERY POINT THE STANDARD DERIVES AS DEFAULT-IGNORABLE, which adds the
///   combining grapheme joiner, the Hangul choseong, jungseong and halfwidth
///   fillers, the two Khmer inherent vowels, the Mongolian free variation
///   selectors, the variation selectors and their supplement, and the reserved
///   runs (U+2065, U+FFF0 to U+FFF8, and most of the tag block).
/// - THE CHARACTERS THAT SHOW AS BLANK BUT ARE NEITHER, which is the C0 and C1
///   controls, the general-punctuation spaces U+2000 to U+200A, the line and
///   paragraph separators, the narrow no-break space, the medium mathematical
///   space and the ideographic space.
///
/// PRIVATE-USE POINTS STAY OUT: a font may draw a glyph for one, so refusing it
/// as invisible would be a lie. UNASSIGNED POINTS ARE IN ONLY WHERE THE
/// STANDARD RESERVES THE RUN AS DEFAULT-IGNORABLE, which is the three runs
/// named above; every one of the 3769 unassigned points this predicate takes
/// carries `Other_Default_Ignorable_Code_Point`, so a conforming renderer draws
/// nothing for it and a character assigned there later will be invisible by
/// construction. Taking those runs whole is also what lets the tag block be one
/// closed range instead of a table.
///
/// The whitespace set is checked before this one, so the six members the two
/// share (tab, LF, CR, U+2007, U+202F, U+3000) separate words rather than
/// refusing.
fn is_invisible(c: char) -> bool {
    matches!(c as u32,
        0x0000..=0x001f
            | 0x007f..=0x009f
            | 0x00ad
            | 0x034f
            | 0x0600..=0x0605
            | 0x061c
            | 0x06dd
            | 0x070f
            | 0x0890..=0x0891
            | 0x08e2
            | 0x115f..=0x1160
            | 0x17b4..=0x17b5
            | 0x180b..=0x180f
            | 0x2000..=0x200f
            | 0x2028..=0x202f
            | 0x205f..=0x206f
            | 0x3000
            | 0x3164
            | 0xfe00..=0xfe0f
            | 0xfeff
            | 0xffa0
            | 0xfff0..=0xfffb
            | 0x110bd
            | 0x110cd
            | 0x13430..=0x1343f
            | 0x1bca0..=0x1bca3
            | 0x1d173..=0x1d17a
            | 0xe0000..=0xe0fff)
}

/// The one way a piece of the PLAYER's text reaches a message that puts it
/// between quotation marks: every member of the invisible set becomes its
/// `U+XXXX` token, and a text with none of them is borrowed back untouched.
///
/// A QUOTED WORD MUST BE THE WORD ON THE PLAYER'S SCREEN, or it must name what
/// is not there. Quoting the raw text failed that both ways, one half measured
/// and the other half not.
///
/// MEASURED, on 2.0.77: a zero-width space between two letters quoted a word
/// that reads exactly as the name the player thinks they typed, so the refusal
/// looked like it was arguing with itself. That is this rule's whole reason on
/// its own.
///
/// MEASURED, but about a different path: a NUL truncates a STORED SETTING
/// VALUE inside the engine and the truncation is persisted (the row in
/// agents/customizer-design.md). That is the value on its way IN, not a
/// refusal on its way out, and what it actually implies is that a NUL typed on
/// the settings screen never reaches the guest at all, because the value is
/// cut at the NUL before it is stored. INFERRED, and asserted nowhere: what
/// the engine would do with a NUL inside a load-failure message. A hand-edited
/// mod-settings.dat is the only path that would ask, and this rule is why it
/// never has to be asked, because an entry carrying a NUL is quoted as
/// `U+0000` and the message is printable whichever way the answer would have
/// gone.
///
/// THE TOKEN CANNOT PRODUCE A MESSAGE A RAW TEXT COULD HAVE PRODUCED, which is
/// the precise claim; the token itself IS typeable. A player who types the six
/// characters `U+0000` gets an entry quoted as `U+0000`, byte for byte what
/// this rule writes for a real NUL. The two WHOLE messages still differ,
/// because `+` is not a name character and is not part of an amount: the typed
/// text is refused FOR the `+` and the real NUL is refused as an invisible
/// character. So a reader who sees `U+200B` in a quoted entry beside the
/// invisible-character sentence is looking at this rule and at nothing else.
///
/// EVERY QUOTING SITE GOES THROUGH IT, which is what makes the property total
/// rather than spot-applied: the entry, the sign, the multi-word fold, the
/// pieces the lexer refuses, and the single strange character. The one place a
/// player's text is written WITHOUT quotation marks is the tag a refusal tells
/// them to type, and that text is a name the World answered to, so the
/// engine's own charset is what keeps it visible.
///
/// AT LEAST FOUR HEX DIGITS, uppercase, the same token the invisible-character
/// sentence prints, so one form of a code point appears in this language and
/// not two.
fn quotable(s: &str) -> Cow<'_, str> {
    if !s.chars().any(is_invisible) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_invisible(c) {
            // WRITTEN INTO THE BUFFER THAT IS ALREADY THERE, rather than
            // formatted into a String of its own and copied in: a text of 2000
            // invisible characters would otherwise be 2000 allocations to
            // build one message. A write into a String cannot fail, which is
            // what the discarded Result says.
            let _ = write!(out, "U+{:04X}", c as u32);
            continue;
        }
        out.push(c);
    }
    Cow::Owned(out)
}

fn trim_ws(s: &str) -> &str {
    s.trim_matches(is_ws)
}

/// A prototype name's charset, measured: item("a.b"), item("a b"),
/// item("a:b"), item("a,b"), item("a/b"), item("a*b"), item("[a]") and
/// item("ae" with an umlaut) each refuse with "Invalid prototype name. Only
/// characters A-Z a-z 0-9 _- are allowed."
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Splits the whole text into entries on commas OUTSIDE a rich-text tag.
///
/// A `[` opens a span that ends at the next `]` or at the end of the text,
/// which is what lets `[item=iron-plate,quality=rare]` reach the parser as
/// ONE entry and be refused with the sentence about quality rather than split
/// into two halves neither of which is a tag.
fn split_entries(text: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let mut in_tag = false;
    for (i, c) in text.char_indices() {
        match c {
            '[' if !in_tag => in_tag = true,
            ']' if in_tag => in_tag = false,
            ',' if !in_tag => {
                parts.push(trim_ws(&text[start..i]));
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(trim_ws(&text[start..]));
    parts
}

/// Lexes one entry, left to right, and stops at the FIRST piece-level
/// problem: a bracket that is not a tag, a thousands separator, a signed or
/// exponent number, a character the language has no place for.
///
/// Words split on whitespace; `*` and the multiplication sign split further
/// and always become signs; everything else is classified whole.
fn tokens(text: &str) -> Result<Vec<Tok>, String> {
    let mut out: Vec<Tok> = Vec::new();
    for word in text.split(is_ws) {
        if word.is_empty() {
            continue;
        }
        let mut start = 0usize;
        for (i, c) in word.char_indices() {
            if c != '*' && c != TIMES {
                continue;
            }
            if i > start {
                classify(&word[start..i], &mut out)?;
            }
            out.push(Tok::Sign(String::from(&word[i..i + c.len_utf8()])));
            start = i + c.len_utf8();
        }
        if start < word.len() {
            classify(&word[start..], &mut out)?;
        }
    }
    Ok(out)
}

/// The tag shape refusal, which every malformed bracket lands on: a player
/// who wrote one bracket meant a tag, and the answer is the two forms there
/// are.
fn tag_shape() -> String {
    String::from("a tag is [item=name] or [fluid=name]")
}

/// Classifies one piece of a word: a tag, then whatever is glued after its
/// closing bracket, or the piece as a whole.
fn classify(piece: &str, out: &mut Vec<Tok>) -> Result<(), String> {
    let mut rest = piece;
    while rest.starts_with('[') {
        // An unclosed bracket is the tag shape refusal and not a name with a
        // bracket in it: the bracket is what the player got wrong.
        let close = match rest.find(']') {
            Some(i) => i,
            None => return Err(tag_shape()),
        };
        out.push(tag_token(&rest[1..close])?);
        rest = &rest[close + 1..];
    }
    if rest.is_empty() {
        return Ok(());
    }
    // Glued text after a tag continues as further pieces, which is what makes
    // "[item=iron-plate]2" a tag and an amount.
    classify_plain(rest, out)
}

/// Reads the inside of a `[...]`, which is `item=` or `fluid=` and a name.
fn tag_token(inner: &str) -> Result<Tok, String> {
    let eq = match inner.find('=') {
        Some(i) => i,
        None => return Err(tag_shape()),
    };
    let kind = &inner[..eq];
    let tag = match kind {
        "item" => Tag::Item,
        "fluid" => Tag::Fluid,
        _ => return Err(tag_shape()),
    };
    let rest = &inner[eq + 1..];
    let (name, params) = match rest.find(',') {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    if name.is_empty() || !name.chars().all(is_name_char) {
        return Err(tag_shape());
    }
    if let Some(params) = params {
        // The one parameter worth naming: shift-clicking a quality item into
        // a text field produces it, and a recipe ingredient has no quality to
        // carry (the engine's ingredient has no such field), so the answer is
        // the plain tag to write instead.
        if params.starts_with("quality=") {
            return Err(format!(
                "ingredients carry no quality; write [{}={}]",
                kind, name
            ));
        }
        return Err(tag_shape());
    }
    Ok(Tok::Name(String::from(name), tag))
}

/// Classifies a piece with no bracket in front of it.
///
/// THE ORDER IS THE LANGUAGE'S WHOLE AMBIGUITY ANSWER, and the review moved
/// two things into it. The thousands shape is caught BEFORE it can be read as
/// the number 1, because "1.000 iron-plate" silently becoming one plate is
/// the worst answer available. The glued sign shapes apply to the WHOLE piece
/// only: `2x`, `x2`, `2x3` and their decimal forms are an amount and a sign,
/// and `loader-1x1` (a real base prototype, measured) is a name, which is the
/// case the first specification got wrong.
fn classify_plain(piece: &str, out: &mut Vec<Tok>) -> Result<(), String> {
    if is_thousands(piece) {
        return Err(format!(
            "\"{}\" is not an amount here; a dot marks a fraction, and a thousand is written 1000",
            quotable(piece)
        ));
    }
    if is_amount(piece) {
        out.push(Tok::Amount(String::from(piece)));
        return Ok(());
    }
    if let Some(toks) = glued(piece) {
        out.extend(toks);
        return Ok(());
    }
    // A sign in front of a number and an exponent are both numbers a player
    // meant as numbers, so neither gets the character message: the minus is a
    // name character and would otherwise be looked up as a prototype, and
    // "1e3" is all name characters and would otherwise be told to add a
    // comma.
    if is_signed(piece) || is_exponent(piece) {
        return Err(format!(
            "\"{}\" is not an amount; amounts are plain digits such as 2 or 0.5",
            quotable(piece)
        ));
    }
    if piece.chars().all(is_name_char) {
        out.push(Tok::Name(String::from(piece), Tag::Untagged));
        return Ok(());
    }
    // The FIRST character outside the syntax, as a whole Unicode scalar: a
    // player who typed an accented letter or an emoji gets that character
    // back, not the first byte of it.
    let bad = piece
        .chars()
        .find(|c| !is_name_char(*c))
        .expect("a piece that is not all name characters has one that is not");
    if is_invisible(bad) {
        // AT LEAST FOUR HEX DIGITS, uppercase, which is how Unicode writes a
        // code point and how the player can search for it.
        return Err(format!(
            "an invisible character (U+{:04X}) has no place here; retype the entry rather than pasting it",
            bad as u32
        ));
    }
    // quotable is a no-op on this one by construction, the arm above having
    // taken every invisible character. It is written anyway, because the
    // property is that EVERY quoting site goes through it and an audit reads
    // the quotation marks rather than the reachability.
    let mut buf = [0u8; 4];
    Err(format!(
        "\"{}\" has no place here; names use the letters a to z, digits, - and _, and an amount is plain digits, as in \"2 iron-plate\"",
        quotable(bad.encode_utf8(&mut buf))
    ))
}

/// The four glued shapes, and only over a WHOLE piece: `x`, `2x`, `x2` and
/// `2x3`, with a decimal allowed on either number. Two sign characters, or a
/// side that is not an amount, is not one of them, which is what leaves
/// `loader-1x1` and `2x3x4` to the name rule.
fn glued(piece: &str) -> Option<Vec<Tok>> {
    // EXACTLY ONE MARK FALLS OUT OF THE SPLIT and is not tested for: the head
    // is the text BEFORE the first mark and so holds none, and a second mark
    // anywhere after it sits in the tail, where `is_amount` refuses it. A
    // guard for two marks would be a branch nothing can reach.
    let (at, sign) = piece.char_indices().find(|(_, c)| *c == 'x' || *c == 'X')?;
    let head = &piece[..at];
    let tail = &piece[at + sign.len_utf8()..];
    if !(head.is_empty() || is_amount(head)) || !(tail.is_empty() || is_amount(tail)) {
        return None;
    }
    let mut toks: Vec<Tok> = Vec::with_capacity(3);
    if !head.is_empty() {
        toks.push(Tok::Amount(String::from(head)));
    }
    toks.push(Tok::Sign(String::from(&piece[at..at + sign.len_utf8()])));
    if !tail.is_empty() {
        toks.push(Tok::Amount(String::from(tail)));
    }
    Some(toks)
}

/// Whether a finite double is an integer, in `core` alone.
///
/// f64::trunc and f64::fract live in `std`, and this crate compiles into
/// somebody else's no_std wasm module, so the question is answered by the two
/// facts that make it answerable without them: every double at or above 2^52
/// is already an integer, and below it a cast to i64 is exact and truncates
/// toward zero. The answer is the same one `v == v.trunc()` gives, which is
/// what the Go half asks with math.Trunc.
fn is_whole(v: f64) -> bool {
    const NO_FRACTION_ABOVE: f64 = 4503599627370496.0;
    if v >= NO_FRACTION_ABOVE || v <= -NO_FRACTION_ABOVE {
        return true;
    }
    (v as i64) as f64 == v
}

/// Plain digits, with one optional dot inside them: `2`, `10`, `0.5`, `007`.
/// No sign, no exponent, no leading or trailing dot. Written by hand rather
/// than by a pattern crate, because this crate takes no dependencies and the
/// rule is four lines.
fn is_amount(s: &str) -> bool {
    let mut before = 0usize;
    let mut after = 0usize;
    let mut dot = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if dot {
                after += 1;
            } else {
                before += 1;
            }
            continue;
        }
        if c == '.' && !dot && before > 0 {
            dot = true;
            continue;
        }
        return false;
    }
    before > 0 && (!dot || after > 0)
}

/// Digits, a dot and EXACTLY three zeros: the thousands separator most of
/// Europe writes, which this language would otherwise read as the number
/// before the dot.
///
/// THE INTEGER PART HOLDS A NONZERO DIGIT, which is the review's correction.
/// "0.000" is nobody's thousand; it is an amount of zero, and the player who
/// wrote it is told the amount must be more than 0 rather than being taught
/// how to write a thousand they never meant.
fn is_thousands(s: &str) -> bool {
    match s.strip_suffix(".000") {
        Some(head) => head.chars().all(|c| c.is_ascii_digit()) && head.chars().any(|c| c != '0'),
        None => false,
    }
}

/// A NUMBER with a sign glued in front of it: `-2`, `+0.5`, `-1e3`.
///
/// The exponent form counts here as well as on its own, because a player who
/// wrote one wrote a number: without this, `-1e3` is all name characters and
/// would be looked up as a prototype and answered with advice about commas.
fn is_signed(s: &str) -> bool {
    match s.strip_prefix(['-', '+']) {
        Some(rest) => is_amount(rest) || is_exponent(rest),
        None => false,
    }
}

/// A number in exponent form: `1e3`, `2.5E-7`. The engine takes one in a data
/// file and a player may well have copied one out of a spreadsheet, so it is
/// a number that is refused rather than a character that is.
fn is_exponent(s: &str) -> bool {
    let at = match s.find(['e', 'E']) {
        Some(i) => i,
        None => return false,
    };
    if !is_amount(&s[..at]) {
        return false;
    }
    let rest = &s[at + 1..];
    let digits = rest.strip_prefix(['-', '+']).unwrap_or(rest);
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}
