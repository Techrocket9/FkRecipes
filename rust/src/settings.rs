use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::op::Op;
use crate::plan::{Lib, SettingDecl, SettingKind};
use crate::value::{finite, kv, str_arr, Value, MAX_EXACT_INT};
use crate::world::World;

impl Lib {
    /// Turns the declared settings into one `Extend` op per setting
    /// prototype, in declaration order, with every name prefixed.
    ///
    /// It takes the same World the data half takes, and the prefix comes from
    /// it, NOT from a parameter: the two stages have to agree on a setting's
    /// name to the byte, and a name passed in here can drift from the one
    /// `plan_data` reads back. Of the World it asks only `mod_name` and
    /// `stage_name`, so the emit layer may pass one that answers the
    /// data-stage questions emptily.
    ///
    /// This is the seam the emit layer stands on at the settings stage;
    /// consumers call Emit and never this. It is public so a consumer's own
    /// tests can hold a plan up to the light without a wasm target.
    pub fn plan_settings(&self, w: &dyn World) -> Result<Vec<Op>, String> {
        let stage = w.stage_name();
        if self.id == 0 {
            return Err(format!(
                "fkrecipes: at the {} stage, this Lib was built without New, so its handles cannot be validated",
                stage
            ));
        }
        let mod_name = w.mod_name();
        if mod_name.is_empty() {
            return Err(format!(
                "fkrecipes: at the {} stage, the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName",
                stage
            ));
        }
        let prefix = format!("{}-", mod_name);
        self.validate_settings(&stage)?;

        let mut ops = Vec::with_capacity(self.settings.len());
        for (i, s) in self.settings.iter().enumerate() {
            let mut pairs = alloc::vec![
                kv("type", Value::string(setting_type_name(s.kind))),
                kv("name", Value::Str(format!("{}{}", prefix, s.name))),
                kv("setting_type", Value::string("startup")),
                kv("default_value", default_value(s)),
                kv("order", Value::Str(order_string(i))),
            ];
            if s.kind == SettingKind::Int || s.kind == SettingKind::Double {
                if let Some(min) = s.spec.min {
                    pairs.push(kv("minimum_value", Value::Num(min)));
                }
                if let Some(max) = s.spec.max {
                    pairs.push(kv("maximum_value", Value::Num(max)));
                }
            }
            if s.kind == SettingKind::Dropdown {
                pairs.push(kv("allowed_values", str_arr(&s.values)));
            }
            ops.push(Op::Extend(Value::Map(pairs)));
        }
        Ok(ops)
    }

    /// Returns the FIRST refusal, scanning in declaration order. The engine's
    /// own answers are why each one exists: two settings of the same type
    /// sharing a name is silent last-writer-wins, and a default outside the
    /// allowed values or the bounds is refused at load with no mod named.
    ///
    /// No refusal here prints a number. A float rendered by two languages is
    /// two different strings sooner or later, and these messages are compared
    /// byte for byte, so each one names the setting and the relationship
    /// instead.
    fn validate_settings(&self, stage: &str) -> Result<(), String> {
        let at = format!("fkrecipes: at the {} stage, ", stage);
        for (i, s) in self.settings.iter().enumerate() {
            for other in self.settings.iter().take(i) {
                if other.name == s.name {
                    return Err(format!(
                        "{}two settings share the name {}; the engine keeps the last one silently",
                        at, s.name
                    ));
                }
            }
            match s.kind {
                SettingKind::Dropdown => {
                    // The loop shape mirrors the Go half's line for line;
                    // .contains would read better in Rust alone.
                    #[allow(clippy::manual_contains)]
                    if !s.values.iter().any(|v| *v == s.def_str) {
                        return Err(format!(
                            "{}the dropdown setting {} defaults to {}, which is not one of its allowed values",
                            at, s.name, s.def_str
                        ));
                    }
                }
                SettingKind::Int | SettingKind::Double => {
                    // The declared integer first, while it is still an
                    // integer: past 2^53 the conversion to double has already
                    // rounded it, so the setting the player sees is not the
                    // one the consumer wrote. The bound is on magnitude
                    // because an int setting's default is legitimately
                    // negative and rounds just the same below -2^53.
                    if s.kind == SettingKind::Int
                        && (s.def_int > MAX_EXACT_INT || s.def_int < -MAX_EXACT_INT)
                    {
                        return Err(format!(
                            "{}the numeric setting {} declares a default a Lua double cannot hold exactly: {}",
                            at, s.name, s.def_int
                        ));
                    }
                    // Finiteness next: every comparison below is meaningless
                    // against a NaN, and an infinity would reach a prototype.
                    let bound_unset = |b: Option<f64>| b.map(finite).unwrap_or(true);
                    if !finite(s.def_num) || !bound_unset(s.spec.min) || !bound_unset(s.spec.max) {
                        return Err(format!(
                            "{}the numeric setting {} declares a value that is not a finite number",
                            at, s.name
                        ));
                    }
                    if let (Some(min), Some(max)) = (s.spec.min, s.spec.max) {
                        if min > max {
                            return Err(format!(
                                "{}the numeric setting {} declares a minimum above its maximum",
                                at, s.name
                            ));
                        }
                    }
                    let below = s.spec.min.map(|min| s.def_num < min).unwrap_or(false);
                    let above = s.spec.max.map(|max| s.def_num > max).unwrap_or(false);
                    if below || above {
                        return Err(format!(
                            "{}the numeric setting {} declares a default outside its own minimum and maximum",
                            at, s.name
                        ));
                    }
                }
                SettingKind::Bool => {}
            }
        }
        Ok(())
    }
}

fn default_value(s: &SettingDecl) -> Value {
    match s.kind {
        SettingKind::Bool => Value::Bool(s.def_bool),
        SettingKind::Dropdown => Value::Str(s.def_str.clone()),
        _ => Value::Num(s.def_num),
    }
}

fn setting_type_name(k: SettingKind) -> &'static str {
    match k {
        SettingKind::Bool => "bool-setting",
        SettingKind::Int => "int-setting",
        SettingKind::Double => "double-setting",
        SettingKind::Dropdown => "string-setting",
    }
}

/// The settings screen's sort key: two base-26 letters from the declaration
/// index, so the screen shows the order the consumer wrote.
///
/// Two letters is 676 startup settings, past anything real; beyond that the
/// strings repeat and the engine breaks the tie by name, which is cosmetic.
/// The arithmetic is integer on purpose: it must agree with the Go mirror.
fn order_string(i: usize) -> String {
    let i = i % (26 * 26);
    let mut s = String::with_capacity(2);
    s.push((b'a' + (i / 26) as u8) as char);
    s.push((b'a' + (i % 26) as u8) as char);
    s
}
