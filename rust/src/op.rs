use alloc::string::String;
use alloc::vec::Vec;

use crate::value::Value;

/// One instruction in the plan's output stream. The planner produces the
/// whole stream as ordinary values; the emit layer executes it against
/// fkdata, in order, and stops at the first refusal fkdata raises.
///
/// Three kinds cover everything v1 does: create a prototype, splice a field
/// of somebody else's prototype, say out loud that something degraded.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// One whole prototype, one per op.
    Extend(Value),
    /// Where in data.raw the write lands, and what it writes.
    Set(Vec<PathEl>, Value),
    /// The line, already carrying its "fkrecipes: " prefix.
    Log(String),
}

/// One step of a [`Op::Set`] path. A path is a walk into data.raw, so a step
/// is either a key or a sequence index.
#[derive(Clone, Debug, PartialEq)]
pub enum PathEl {
    Str(String),
    Num(f64),
}

pub(crate) fn path_key(s: &str) -> PathEl {
    PathEl::Str(String::from(s))
}
