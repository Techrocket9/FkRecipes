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
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
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

/// The inline localised-string form `{"", text}`. The engine takes a plain
/// string as the parameter and refuses a number (measured), so the caller
/// stringifies before it gets here.
pub(crate) fn localised(text: &str) -> Value {
    Value::Arr(alloc::vec![Value::string(""), Value::string(text)])
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
