use alloc::string::String;
use alloc::vec::Vec;

/// A pure mirror of the value model fkdata builds prototypes from: enough of
/// Lua to describe a prototype and nothing more. The emit layer translates it
/// one node at a time, which is what keeps this crate free of any fkdata
/// dependency on the host.
///
/// A map is a vector of pairs in DECLARATION ORDER, never a `HashMap`: the
/// plan is compared byte for byte against the Go mirror, and hash iteration
/// order is not a promise anyone made. fkdata sorts on the way out, so the
/// order here is for the mirror and the transcripts, not for the engine.
///
/// IT GROWS ADDITIVELY, the way [`World`](crate::World) does and for the reason
/// that trait's own note gives: `Bytes` was added to this enum once already,
/// and a consumer matching exhaustively on it would have had their host tests
/// stop compiling over a variant no plan of theirs ever constructs. So the
/// enum is `#[non_exhaustive]` from 0.1.0, which costs a consumer a wildcard
/// arm and is what Go's `Kind` switch already implies, since a Go switch on an
/// int-like kind needs a default anyway. It binds only OTHER crates: this
/// crate's own matches stay exhaustive, which is what `from_v`'s "no catch-all"
/// note asks for, so a variant added later still breaks the emit layer's build
/// rather than arriving there as a silent nil.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Value {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
    /// A STRING WHOSE BYTES ARE NOT TEXT. The host delivers such a value
    /// byte-exact and never rewrites it: a stored setting somebody edited by
    /// hand, a prototype field another mod wrote.
    ///
    /// THE ASYMMETRY WITH GO IS BY CONSTRUCTION, and it is recorded here
    /// rather than smoothed over. A Go `string` IS a byte string, so the Go
    /// mirror carries these bytes in an ordinary `Str` and its model has no
    /// arm for them; a Rust `String` cannot hold them, so this model has one.
    /// The two halves agree about the VALUE and differ only in how the type
    /// system says it, EXCEPT A MAP KEY, which this model cannot represent at
    /// all: `Map` is keyed by `String`, so `from_v` sinks a whole map whose key
    /// is not text to Nil where the Go mirror carries the key.
    ///
    /// IT IS BUILT AT THE HOST BOUNDARY AND NOWHERE ELSE: `from_v` makes one
    /// out of a fkdata string whose bytes are not UTF-8, and a test fixture
    /// makes one to stand in for that. No planner constructs one, and no
    /// planner takes a decision on the strength of one being here rather than
    /// a `Str`, other than the two refusals that exist to say a text setting
    /// or a dropdown cannot hold it.
    ///
    /// IT IS WRITTEN BACK OUT BYTE-EXACT by `to_v`, so a value the plan copies
    /// and never inspects crosses unchanged, which is the contract fkdata
    /// itself keeps.
    Bytes(Vec<u8>),
    Arr(Vec<Value>),
    Map(Vec<(String, Value)>),
}

impl Value {
    /// The absent value.
    pub fn nil() -> Value {
        Value::Nil
    }

    /// A boolean value.
    pub fn boolean(b: bool) -> Value {
        Value::Bool(b)
    }

    /// A numeric value. Factorio has one number type and it is a double, so
    /// integers ride here too.
    pub fn num(n: f64) -> Value {
        Value::Num(n)
    }

    /// A string value.
    pub fn string(s: &str) -> Value {
        Value::Str(String::from(s))
    }

    /// A string value whose bytes are not text. See [`Value::Bytes`]: the host
    /// boundary and the fixtures that stand in for it are the only callers.
    pub fn bytes(b: &[u8]) -> Value {
        Value::Bytes(Vec::from(b))
    }

    /// An array value, which the emit layer writes as a Lua sequence.
    pub fn arr(items: Vec<Value>) -> Value {
        Value::Arr(items)
    }

    /// A map value. The pairs keep the order they are given in.
    pub fn obj(pairs: Vec<(String, Value)>) -> Value {
        Value::Map(pairs)
    }
}

/// One map entry. The key is always a string: nothing this library emits is
/// keyed by anything else.
pub fn kv(key: &str, val: Value) -> (String, Value) {
    (String::from(key), val)
}

/// Turns a name list into an array of strings, the shape every prerequisite
/// list uses.
pub(crate) fn str_arr(names: &[String]) -> Value {
    let mut items = Vec::with_capacity(names.len());
    for n in names {
        items.push(Value::Str(n.clone()));
    }
    Value::Arr(items)
}

/// The ENGINE's limit on one STRING ELEMENT of a localised string on a
/// DATA-STAGE prototype. [`LOCALISED_CHUNK_BUDGET`] is THIS LIBRARY's own, the
/// size a chunk is filled to; they are different numbers on purpose, and the
/// two blocks say why.
///
/// MEASURED, not assumed (Factorio 2.0.77, build 84539, mac-arm64, steam; the
/// binary was re-asked its version before the runs), with a throwaway probe
/// mod:
///
/// - 200 BYTES PER STRING ELEMENT, policed on the key slot and on every literal
///   parameter alike. 200 loads; 201 refuses the whole load with `Error while
///   loading recipe prototype "..." (recipe): Localised string key is too
///   large: 201 > 200 (limit). in property tree at
///   ROOT.recipe.<name>.localised_description[1]`. The index is 0-BASED over
///   the elements, so `{"", X}` reports X at index 1 and a one-element `{X}` at
///   0.
/// - NO AGGREGATE BUDGET: sixteen elements of which fifteen are 199 bytes, 2985
///   bytes in one description, exit 0.
/// - BYTES AND NOT CHARACTERS: 100 x U+00E9 is 100 characters and 200 bytes and
///   loads; 101 is 202 bytes and refuses reporting `202 > 200`.
/// - `localised_name` is policed identically
///   (`ROOT.recipe.inserter.localised_name[1]`), and item, recipe and
///   technology prototypes all carry the rule.
/// - A SETTING PROTOTYPE IS NOT SUBJECT TO IT AT ALL. A string-setting loaded
///   with 201, 400, 1000, 2000 and 5000-byte elements in
///   `localised_description` and in `localised_name`: exit 0 every time, no
///   message of any kind, every byte reaching `mod-settings-dump.json`.
///
/// THE SETTINGS SIDE IS THEREFORE NOT CHUNKED, and that is a rule rather than
/// an oversight. The measured negative above says it does not have to be, and
/// the locale guard says it must not be: `localised_carries` compares a
/// composed line WHOLE against one `Value::Str` element (`locale.rs`), so a
/// chunked settings line would make `check_locale` report every composed line
/// as missing and move `testdata/locale/findings.golden`. Do not unify the two
/// sides.
///
/// IT IS COMPARED AGAINST AND NEVER COMPUTED WITH, the habit `CRAFT_TIME_FLOOR`
/// established: nothing here derives the budget from it. The one reader is the
/// host test that walks every composition this library can build, so it is
/// gated to the test build rather than carrying a silenced `dead_code`.
#[cfg(test)]
pub(crate) const LOCALISED_ELEMENT_CEILING: usize = 200;

/// What a chunk is filled to, twenty bytes short of the engine's ceiling
/// ([`LOCALISED_ELEMENT_CEILING`]) for three reasons, none of them arithmetic:
/// an off-by-one in the chunker cannot land exactly on a refusal; a hard cut
/// inside an over-long word backs off to a UTF-8 character boundary and loses
/// up to three bytes without approaching the ceiling; and a later edit to one
/// of these sentences has room before anything has to be rechunked.
pub(crate) const LOCALISED_CHUNK_BUDGET: usize = 180;

/// Splits `text` into pieces no localised-string element ceiling can refuse,
/// over BYTES, preferring to end a chunk after an ASCII space.
///
/// FOUR PROPERTIES, BY CONSTRUCTION:
///
/// - every chunk is at most [`LOCALISED_CHUNK_BUDGET`] bytes long, so no chunk
///   can reach [`LOCALISED_ELEMENT_CEILING`];
/// - the chunks concatenated are the input byte for byte. The engine
///   concatenates a localised string's parameters, so nothing a player reads
///   changes;
/// - no chunk ever ends inside a UTF-8 character;
/// - the split is a pure function of the bytes, so the Rust and the Go half
///   cannot disagree about it.
///
/// THE CONTINUATION-BYTE BACK-OFF CANNOT SPIN. A run of continuation bytes
/// longer than the budget is not UTF-8 at all, which a `&str` cannot be and a
/// Go string is free to be; the guard below keeps the loop advancing on such a
/// string rather than emitting an empty chunk forever, and the Go mirror
/// carries the same guard so the two halves are the same code and not merely
/// the same outcome.
pub(crate) fn chunk_localised(text: &str) -> Vec<&str> {
    if text.len() <= LOCALISED_CHUNK_BUDGET {
        return alloc::vec![text];
    }
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let rem = &bytes[start..];
        if rem.len() <= LOCALISED_CHUNK_BUDGET {
            out.push(&text[start..]);
            break;
        }
        let cut = match last_space(&rem[..LOCALISED_CHUNK_BUDGET]) {
            // The space ENDS the chunk it was found in rather than opening the
            // next one, so a break between words keeps the space with the words
            // before it.
            //
            // IT IS NOT A CLAIM ABOUT EVERY CHUNK. The hard cut below takes no
            // notice of what follows it, so "z"x180 + " " + "z"x100 splits into
            // 180 and 101 bytes and the second chunk DOES begin with the space.
            // The engine concatenates the parameters, so a reader sees the same
            // text either way; this arm is about where a break falls when there
            // is a choice, not about what a chunk may start with.
            Some(sp) => sp + 1,
            None => {
                let mut cut = LOCALISED_CHUNK_BUDGET;
                while cut > 0 && is_utf8_continuation(rem[cut]) {
                    cut -= 1;
                }
                if cut == 0 {
                    cut = LOCALISED_CHUNK_BUDGET;
                }
                cut
            }
        };
        out.push(&text[start..start + cut]);
        start += cut;
    }
    out
}

/// The byte index of the last ASCII space in `s`, or `None`.
fn last_space(s: &[u8]) -> Option<usize> {
    s.iter().rposition(|b| *b == b' ')
}

/// Whether `b` is the second or later byte of a UTF-8 character, which is where
/// a cut may not land.
fn is_utf8_continuation(b: u8) -> bool {
    b & 0xC0 == 0x80
}

/// [`chunk_localised`]'s result as localised-string parameters.
pub(crate) fn localised_chunks(text: &str) -> Vec<Value> {
    chunk_localised(text)
        .into_iter()
        .map(Value::string)
        .collect()
}

/// The inline localised-string form `{"", text}`, split across as many
/// parameters as the engine's element ceiling needs. The engine takes a plain
/// string as the parameter and refuses a number (measured), so the caller
/// stringifies before it gets here.
///
/// A TEXT INSIDE THE BUDGET IS ONE CHUNK, so the shape stays `{"", text}` byte
/// for byte and a golden taken before this splitter existed does not move for
/// it.
pub(crate) fn localised(text: &str) -> Value {
    crate::settings::localised_group(&localised_chunks(text))
}

/// The refusal a byte string takes at the one surface of this library that
/// cannot carry one: a name it has to read as text.
///
/// IT LIVES IN THE PURE HALF SO IT HAS A WITNESS. The only caller is the emit
/// layer, which is wasm-gated and where no host test can reach a branch; the
/// sentence a player would read is pinned by a host test here instead. The
/// shape follows fkdata's own `text` helper, which refuses the same way for
/// the same reason at the handful of surfaces the ENGINE constrains: the
/// surface is named and the bytes are printed in hex, because a terminal makes
/// what it likes of the bytes themselves and a lossy rewrite would change the
/// value's length silently.
///
/// COMPILED WHERE IT IS READ AND NOWHERE ELSE, which is a stronger statement
/// than silence. The only caller is behind `cfg(target_family = "wasm")` and
/// the only witness is behind `cfg(test)`, so the plain host build a consumer's
/// `cargo check` makes wants neither and the gate is exactly those two
/// configurations. Written as `allow(dead_code)` instead, the item would be
/// compiled into that build with its warning turned off, and the same
/// attribute would go on turning the warning off under `cargo test`: deleting
/// the witness would then cost nothing and this message would lose the only
/// test that reads it. Under this shape each configuration that compiles the
/// item has a reader, so deleting either one is a dead_code warning, which
/// `RUSTFLAGS=-Dwarnings` makes a build failure. Making it `pub` would quiet
/// the warning too, at the price of a message helper in this crate's public
/// surface, which is the worse trade.
#[cfg(any(target_family = "wasm", test))]
pub(crate) fn not_text(surface: &str, bytes: &[u8]) -> String {
    alloc::format!(
        "fkrecipes: {} is not valid UTF-8, and this library reads it as text rather than rewriting it: the bytes are {}",
        surface,
        hex(bytes)
    )
}

/// Lowercase hex, for the refusal above, and gated with it for the reason it
/// gives.
#[cfg(any(target_family = "wasm", test))]
fn hex(b: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::new();
    for x in b {
        out.push(DIGITS[(x >> 4) as usize] as char);
        out.push(DIGITS[(x & 0x0f) as usize] as char);
    }
    out
}

/// The guard every float crosses before it can reach an op. An infinity or a
/// NaN in a prototype is a load failure the consumer cannot read their way
/// out of, and the two languages print them differently, so the planner
/// refuses instead of emitting one.
pub(crate) fn finite(n: f64) -> bool {
    n.is_finite()
}

/// The largest integer a Lua double holds exactly, 2^53. Past it the engine's
/// own numbers start rounding, so a plan that declared an amount above it
/// would emit a different one, silently. The planner refuses instead. The Go
/// mirror carries the same constant.
pub(crate) const MAX_EXACT_INT: i64 = 9007199254740992;

/// The engine's ceiling for an ITEM ingredient's amount, inclusive.
///
/// MEASURED (Factorio 2.0.77, build 84539, mac-arm64, steam): a recipe
/// ingredient with `amount = 65536` refuses the load with "Value (65536)
/// outside of range. The data type allows values from 0 to 65535", and 65535
/// loads. A fluid has no such ceiling: 1000000000 loads and dumps as written.
pub(crate) const MAX_ITEM_AMOUNT: i64 = 65535;

/// The engine's ceiling for a FLUID ingredient's amount, inclusive.
///
/// MEASURED (Factorio 2.0.77, build 84539, mac-arm64, steam): a fluid
/// ingredient of 1e301 loads and dumps as written; 1e302 does not refuse the
/// load, it aborts inside the engine with "FixedPointNumber.hpp:31: double
/// value not in range for fixed point number: inf" and hands the player the
/// crash handler. The wall itself is DBL_MAX / 2^24 = 1.0715086071862672e301,
/// the fixed-point conversion's scale; FkLua's data-stage probe bracketed it at
/// 1.0715e301 loads and 1.0716e301 aborts. The round number below it is the
/// ceiling on the safe side of the measurement, and the refusal quotes it.
pub(crate) const MAX_FLUID_AMOUNT: f64 = 1e301;

/// The engine's exclusive floor for a recipe's `energy_required`. A value must
/// be STRICTLY GREATER than this.
///
/// MEASURED, not assumed (Factorio 2.0.77, build 84539, mac-arm64, steam; the
/// binary was re-asked its version before the runs). `energy_required = 0` and
/// `energy_required = -1` both refuse the load, exit 1, with the engine's own
/// message: Error while loading recipe prototype "..." (recipe):
/// energy_required can't be <= 0.001. Values 0.0011 and 0.002 load (exit 0)
/// and reach data-raw-dump.json unchanged. An omitted `energy_required` stays
/// absent, and the engine applies its 0.5 default at runtime.
///
/// Compared against, never computed with: a const expression folded by two
/// languages is the measured FkLua trap this repository already carries.
pub(crate) const CRAFT_TIME_FLOOR: f64 = 0.001;

/// The smallest round value safely above the floor. A generated setting that
/// backs a crafting time gets it as its minimum unless the consumer set one,
/// so the settings GUI cannot produce a value that kills the load.
pub(crate) const CRAFT_TIME_AUTO_MINIMUM: f64 = 0.002;
