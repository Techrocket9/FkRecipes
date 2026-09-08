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
