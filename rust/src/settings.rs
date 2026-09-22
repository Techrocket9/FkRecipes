use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ingredient_list::{IngredientList, ListEntry, ListKind, ListText, DEFAULT, MAX_TEXT};
use crate::op::Op;
use crate::plan::{
    Amount, CostChoice, Ingredient, IngredientChoice, Lib, Pack, SettingDecl, SettingKind, TechSpec,
};
use crate::value::{
    finite, kv, str_arr, Value, CRAFT_TIME_FLOOR, LOCALISED_ELEMENT_CEILING, MAX_EXACT_INT,
};
use crate::world::{Named, World};

impl Lib {
    /// Turns the declared settings into one `Extend` op per setting
    /// prototype, in declaration order, with every name prefixed.
    ///
    /// It takes the same World the data half takes, and the prefix comes from
    /// it, NOT from a parameter: the two stages have to agree on a setting's
    /// name to the byte, and a name passed in here can drift from the one
    /// `plan_data` reads back. Of the World it asks only `mod_name`, so the
    /// emit layer may pass one that answers the data-stage questions emptily.
    ///
    /// This is the seam the emit layer stands on at the settings stage;
    /// consumers call Emit and never this. It is public so a consumer's own
    /// tests can hold a plan up to the light without a wasm target.
    pub fn plan_settings(&self, w: &dyn Named) -> Result<Vec<Op>, String> {
        if self.id == 0 {
            return Err(String::from(
                "fkrecipes: this Lib was built without New, so its handles cannot be validated",
            ));
        }
        let mod_name = w.mod_name();
        if mod_name.is_empty() {
            return Err(String::from(
                "fkrecipes: the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName",
            ));
        }
        let prefix = format!("{}-", mod_name);
        let bound = self.craft_time_bound_settings();
        self.validate_settings(&prefix, &bound)?;
        // THE SETTINGS STAGE VALIDATES BINDINGS TOO, and it has to: a text
        // setting's default is rendered into its description here, and a
        // dropdown with a text setting beside it has its whole preset list
        // composed here, and a research number its range.
        // Both read the recipes and technologies, so both need them well
        // formed. The two validators are shared with the data planner rather
        // than written twice.
        self.validate_bindings(&prefix)?;
        self.validate_text_settings(&prefix)?;

        let numbers = self.research_number_settings();
        let mut ops = Vec::with_capacity(self.settings.len());
        for (i, s) in self.settings.iter().enumerate() {
            // A legacy setting carries the name and the order the mod
            // already ships; everything else is prefixed, and ordered by
            // declaration behind whatever `order_after` last named.
            let order = self.emitted_order(i);
            let full = s.emitted_name(&prefix);
            let mut pairs = alloc::vec![
                kv("type", Value::string(setting_type_name(s.kind))),
                kv("name", Value::Str(full.clone())),
                kv("setting_type", Value::string("startup")),
                kv("default_value", default_value(s)),
                kv("order", Value::Str(order)),
            ];
            if s.kind == SettingKind::Int || s.kind == SettingKind::Double {
                let spec = self.effective_numeric_spec(i, &bound);
                if let Some(min) = spec.min {
                    pairs.push(kv("minimum_value", Value::Num(min)));
                }
                if let Some(max) = spec.max {
                    pairs.push(kv("maximum_value", Value::Num(max)));
                }
            }
            // WHETHER ANY COMPOSITION FIRED, so a setting nothing is
            // composed onto can still carry the literal the plan wrote. See
            // `describe_setting`.
            let mut composed = false;
            if s.kind == SettingKind::Dropdown {
                pairs.push(kv("allowed_values", str_arr(&s.values)));
                // THE PRESETS, WRITTEN OUT, on the dropdown that has a text
                // setting beside it. The engine cannot pre-fill that text from
                // the value they had (the settings stage sees no stored value,
                // measured), so the next best thing is showing them what each
                // preset means in the language they are about to type.
                if let Some(presets) = self.composed_dropdown_presets(i + 1) {
                    pairs.push(kv(
                        "localised_description",
                        self.dropdown_description(&prefix, &full, &presets),
                    ));
                    composed = true;
                }
            }
            // A RESEARCH NUMBER STATES ITS RANGE, and beside a research
            // dropdown it states what 0 means: the settings screen shows a
            // numeric field with no visible bounds and no way to guess that.
            if numbers[i].bound {
                pairs.push(kv(
                    "localised_description",
                    number_description(
                        self.setting_description_head(i, &full),
                        &self.research_range_line(i, s, numbers[i].dropdown),
                    ),
                ));
                composed = true;
            }
            if is_text(s.kind) {
                // MEASURED: auto_trim is a GUI behaviour and does not touch
                // what mod-settings.dat holds (a stored value with spaces
                // around it reads back with both space runs), so the parser
                // trims for itself as well. It is set because the settings
                // screen tidying up after a paste is worth having anyway.
                pairs.push(kv("auto_trim", Value::Bool(true)));
                pairs.push(kv(
                    "localised_description",
                    // THE LADDER LINE GOES WHERE NOTHING ELSE SAYS IT, which
                    // is the one condition this call carries beyond the kind.
                    // An INGREDIENT dropdown beside the field renders the lists
                    // the ladder is about and DROPDOWN_LADDER_LINE says so
                    // there, so composing it here as well would say one thing
                    // twice on one screen; a COST dropdown renders no list and
                    // carries no such line, so a packs text beside one keeps
                    // it. With no dropdown at all this field's own default line
                    // is the list that applies. See `text_carries_ladder_line`
                    // and `text_ladder_line`.
                    text_description(
                        self.setting_description_head(i, &full),
                        &self.rendered_default(&prefix, s),
                        &self.text_switch_line(i),
                        s.kind == SettingKind::Ingredients,
                        self.text_carries_ladder_line(i),
                    ),
                ));
                composed = true;
            }
            // AND A SETTING NOTHING IS COMPOSED ONTO CARRIES THE LITERAL
            // ALONE, which is the whole of what `describe_setting` does for a
            // bool, a plain number or a dropdown nothing binds: there is no
            // composition for the head to open, so the description IS the
            // head.
            if s.described && !composed {
                pairs.push(kv(
                    "localised_description",
                    Value::Str(s.description.clone()),
                ));
            }
            ops.push(Op::Extend(Value::Map(pairs)));
        }
        Ok(ops)
    }

    /// What every composed description opens with: the consumer's own
    /// `[mod-setting-description]` key, wrapped in the alternatives form, or
    /// the literal the plan wrote through `describe_setting`.
    ///
    /// ONE FUNCTION FOR ALL THREE COMPOSITIONS, because the choice is the same
    /// choice on a text setting, a research number and a composed dropdown
    /// alike, and a second spelling is how one of the three could keep
    /// composing a key the plan replaced.
    pub(crate) fn setting_description_head(&self, i: usize, full: &str) -> Value {
        let s = &self.settings[i];
        if s.described {
            return Value::Str(s.description.clone());
        }
        locale_ref("mod-setting-description", full, full)
    }

    /// The order string a setting is emitted with, which is also the string
    /// the order scan below compares: a legacy setting's is the one the mod
    /// already ships, and a generated setting's is the order prefix that was
    /// in force where it was declared followed by the two letters its
    /// declaration index gives it.
    ///
    /// ONE READER, TWO CALLERS. The planner writes this into the prototype
    /// and the validator refuses on it, so a change to either would have to
    /// be written twice to go unnoticed.
    fn emitted_order(&self, i: usize) -> String {
        let s = &self.settings[i];
        if s.legacy {
            return s.order.clone();
        }
        format!("{}{}", s.order_prefix, order_string(i))
    }

    /// Returns the FIRST refusal, scanning in declaration order. The engine's
    /// own answers are why each one exists: two settings of the same type
    /// sharing a name is silent last-writer-wins, and a default outside the
    /// allowed values or the bounds is refused at load with no mod named.
    ///
    /// IN THREE PARTS, IN THIS ORDER: an empty `order_after`, which is the
    /// PLAN's mistake and not any setting's; then every setting's own checks
    /// in declaration order; then the order scan, which is the only one that
    /// quotes a setting other than the one it is checking and so may not run
    /// before the others have cleared.
    ///
    /// No refusal here prints a number. A float rendered by two languages is
    /// two different strings sooner or later, and these messages are compared
    /// byte for byte, so each one names the setting and the relationship
    /// instead.
    fn validate_settings(&self, prefix: &str, bound: &[bool]) -> Result<(), String> {
        let at = "fkrecipes: ";
        // BEFORE THE LOOP, AND NOT INSIDE IT. `order_after` records the
        // mistake rather than refusing on the spot, and the plan it spoils is
        // the whole plan: a call with nothing declared after it is still a
        // consumer who meant to place something, and a loop-bound check would
        // let that one through with no settings to trip it.
        if self.empty_order_after {
            return Err(format!(
                "{}OrderAfter was given an empty order; name the order string the generated settings should follow",
                at
            ));
        }
        for (i, s) in self.settings.iter().enumerate() {
            if s.name.is_empty() {
                return Err(format!("{}a setting was declared with an empty name", at));
            }
            // A legacy setting supplies its own order because a mod that
            // already shipped chose one; an empty string is not a choice.
            if s.legacy && s.order.is_empty() {
                return Err(format!(
                    "{}the legacy setting {} was declared with an empty order",
                    at, s.name
                ));
            }
            // Compared on the EMITTED names, which is the namespace the engine
            // keeps: a legacy name and a generated one can arrive at the same
            // string from different declarations, and only one survives.
            for other in self.settings.iter().take(i) {
                if other.emitted_name(prefix) == s.emitted_name(prefix) {
                    return Err(format!(
                        "{}two settings share the name {}; the engine keeps the last one silently",
                        at,
                        s.emitted_name(prefix)
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
                    // A setting the player turns into a crafting time may not
                    // offer a value the engine refuses, so its own minimum has
                    // to clear the floor. Nothing here prints a number: the
                    // floor appears once, as literal text inside the message.
                    if bound[i] && s.spec.min.map(|m| m <= CRAFT_TIME_FLOOR).unwrap_or(false) {
                        return Err(format!(
                            "{}the setting {} backs a crafting time but declares a minimum at or below the engine floor (energy_required can't be <= 0.001)",
                            at, s.name
                        ));
                    }
                    // The EFFECTIVE bounds, so a generated minimum is checked
                    // against the default exactly as a declared one is. A
                    // GENERATED minimum says so in its own refusals: blaming a
                    // "declared minimum" the consumer never wrote sends them
                    // looking for the wrong line.
                    let spec = self.effective_numeric_spec(i, bound);
                    if bound[i] && s.spec.min.is_none() {
                        // Destructured, not defaulted: effective_numeric_spec
                        // fills this arm's minimum in, so an absent one is a
                        // broken invariant. A silent 0.0 would let this half
                        // limp on where the Go mirror would not.
                        let generated = spec
                            .min
                            .expect("a craft-time-bound setting has a generated minimum");
                        if let Some(max) = s.spec.max {
                            if generated > max {
                                return Err(format!(
                                    "{}the setting {} backs a crafting time, so its minimum is 0.002, which is above the declared maximum",
                                    at, s.name
                                ));
                            }
                        }
                        if s.def_num < generated {
                            return Err(format!(
                                "{}the setting {} backs a crafting time, so its minimum is 0.002, which is above the declared default",
                                at, s.name
                            ));
                        }
                        if s.spec.max.map(|max| s.def_num > max).unwrap_or(false) {
                            return Err(format!(
                                "{}the numeric setting {} declares a default outside its own minimum and maximum",
                                at, s.name
                            ));
                        }
                    } else {
                        if let (Some(min), Some(max)) = (spec.min, spec.max) {
                            if min > max {
                                return Err(format!(
                                    "{}the numeric setting {} declares a minimum above its maximum",
                                    at, s.name
                                ));
                            }
                        }
                        let below = spec.min.map(|min| s.def_num < min).unwrap_or(false);
                        let above = spec.max.map(|max| s.def_num > max).unwrap_or(false);
                        if below || above {
                            return Err(format!(
                                "{}the numeric setting {} declares a default outside its own minimum and maximum",
                                at, s.name
                            ));
                        }
                    }
                }
                // A text setting's declaration is checked by
                // `validate_text_settings`, below, which both planners run:
                // the data stage reads the same declared list and must refuse
                // the same declarations.
                SettingKind::Bool | SettingKind::Ingredients | SettingKind::Packs => {}
            }
        }
        // THE ORDER SCAN, A SECOND PASS AND NOT PART OF THE LOOP ABOVE. It
        // quotes a setting other than the one it is checking, so it may not
        // run until every setting has passed its own checks: inside the loop
        // it reads declarations the loop has not reached, which puts an empty
        // name into its own sentence and answers, at the first setting, plans
        // whose real mistake is a later setting's default, name or order.
        //
        // Generated settings are scanned in declaration order and the legacy
        // settings each is held against are too, so a plan with two of these
        // gets the earlier one both times.
        //
        // A TIE WAS ACCEPTED BEFORE THIS CHECK EXISTED. A plan whose legacy
        // "aa" sat beside a generated first setting loaded, with the settings
        // screen breaking the tie by name; it is refused now, deliberately.
        //
        // TWO GENERATED ORDERS ARE NEVER COMPARED. Each is a prefix followed
        // by EXACTLY TWO letters, so two equal strings force equal prefixes,
        // and a tie under one prefix needs the declaration indices to differ
        // by the 676 the two letters count to, which `order_string` documents
        // as cosmetic.
        for (i, s) in self.settings.iter().enumerate() {
            if s.legacy {
                continue;
            }
            let order = self.emitted_order(i);
            for (j, other) in self.settings.iter().enumerate() {
                if !other.legacy {
                    continue;
                }
                let legacy_order = self.emitted_order(j);
                // THE TIE FIRST, for this legacy setting: the engine sorts
                // two settings sharing an order by name, so the screen shows
                // a placement neither the consumer nor this library chose.
                // Only one of the two rules can hold for one pair.
                if legacy_order == order {
                    return Err(format!(
                        "{}the setting {} would carry the order {}, which the legacy setting {} already carries; give one of them an order of its own",
                        at, s.name, order, other.name
                    ));
                }
                // THE PLACEMENT `order_after` EXISTS TO PREVENT, which a tie
                // check alone misses: a legacy order that EXTENDS the named
                // one sorts inside the neighbourhood the placed settings are
                // aimed at, and the two letters walk past it when they roll
                // over from "az" to "ba" ("aba" is past a legacy "ab"). The
                // consumer asked for a placement this plan cannot keep.
                //
                // AN EMPTY PREFIX PLACES NOTHING, and is skipped rather than
                // compared: every order extends the empty string, so a plan
                // that never called `order_after` would have its ordinary
                // legacy orders refused by a rule about a neighbourhood it
                // never asked for.
                if !s.order_prefix.is_empty()
                    && legacy_order.starts_with(&s.order_prefix)
                    && legacy_order != s.order_prefix
                    && legacy_order < order
                {
                    return Err(format!(
                        "{}the setting {} would carry the order {} and sort past the legacy setting {} at {}, which extends {}; OrderAfter({}) places settings before every legacy order that extends {}",
                        at,
                        s.name,
                        order,
                        other.name,
                        legacy_order,
                        s.order_prefix,
                        s.order_prefix,
                        s.order_prefix
                    ));
                }
            }
        }
        Ok(())
    }

    /// Every rule about how a text setting is BOUND, and both planners run
    /// it: the settings stage composes a dropdown's description out of a
    /// text setting beside one, so it needs that pairing to be well formed just
    /// as much as the data stage does.
    ///
    /// IT REFUSES NOTHING THE OTHER TWO VALIDATORS ALREADY OWN. Where a recipe
    /// or a technology names two ingredient sources or two costs that this
    /// file did not add, the walk SKIPS that declaration and leaves the
    /// sentence to the data planner's own loop: a plan with two problems
    /// should be answered by the one its author is likelier to recognise, and
    /// "pick one" is that sentence.
    /// Every rule about `describe_setting`, and both planners run it because
    /// `validate_bindings` does.
    ///
    /// IT LIVES BESIDE THE BINDING RULES RATHER THAN IN `validate_settings`,
    /// and the reason is which planners run each: `validate_settings` is the
    /// settings stage's alone, and a description the plan wrote is a
    /// declaration the DATA stage must refuse too, because a plan that refuses
    /// at one stage and loads at the other is a mod whose two halves disagree
    /// about what it declares.
    ///
    /// THE ORDER IS THE CALL'S AND THEN THE DECLARATION'S. A handle from
    /// another plan is answered first, because nothing about it names a
    /// setting of this plan at all; the described-twice sentences follow in
    /// CALL order, which is the order an author reads their own file in; and
    /// the empty-description sentences last, in declaration order.
    pub(crate) fn validate_descriptions(&self, prefix: &str) -> Result<(), String> {
        let at = "fkrecipes: ";
        if self.describe_foreign {
            return Err(format!(
                "{}DescribeSetting names a setting that this plan never declared",
                at
            ));
        }
        // THE FIRST SUCH CALL IN CALL ORDER, which is the same every run and
        // the same in both halves, and is the one an author reaches first
        // reading their own file.
        if let Some(index) = self.described_twice.first() {
            return Err(format!(
                "{}the setting {} is described twice; DescribeSetting takes one description",
                at,
                self.settings[index - 1].emitted_name(prefix)
            ));
        }
        for s in &self.settings {
            // AN EMPTY DESCRIPTION IS NOT THE SAME AS NO DESCRIPTION, which is
            // why the call is recorded separately from what it carried. A
            // setting emitted with an empty localised_description is a row
            // whose info icon says nothing, and the author who wrote the call
            // meant to say something.
            if s.described && s.description.is_empty() {
                return Err(format!(
                    "{}DescribeSetting was given an empty description for the setting {}",
                    at,
                    s.emitted_name(prefix)
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_bindings(&self, prefix: &str) -> Result<(), String> {
        let at = "fkrecipes: ";
        self.validate_descriptions(prefix)?;

        // ONE DROPDOWN COMPOSES ONE DESCRIPTION. A setting carries a single
        // localised_description, so a second declaration's presets would
        // silently replace the first's; the recipes and the technologies count
        // separately, because each loop names what it walked. Counted in the
        // two walks below, where the pairing is proved, and refused after both
        // of them so the sentence names the first such setting in declaration
        // order rather than whichever walk noticed first.
        let mut armed_by_recipe = alloc::vec![0usize; self.settings.len()];
        let mut armed_by_tech = alloc::vec![0usize; self.settings.len()];
        // AND ONE DROPDOWN SHOWS ONE DECLARATION'S PRESETS, which is what
        // `describes` says out loud. Counted in the same two walks and refused
        // after both of them, for the reason the two above are.
        let mut described = alloc::vec![0usize; self.settings.len()];

        for r in &self.recipes {
            let who = format!("the recipe {}", r.name);
            if let Some(h) = r.spec.ingredients_from {
                if !r.spec.ingredients.is_empty() {
                    return Err(format!(
                        "{}{} names both Ingredients and IngredientsFrom; pick one",
                        at, who
                    ));
                }
                if !self.valid_ingredients_setting(h) {
                    return Err(format!(
                        "{}{} reads its ingredients from a setting that this plan never declared",
                        at, who
                    ));
                }
            }
            let by = match &r.spec.ingredients_by {
                Some(by) => by,
                None => continue,
            };
            // The dropdown's own exclusivity with Ingredients is the data
            // planner's sentence; nothing here may fire in front of it.
            if !r.spec.ingredients.is_empty() {
                continue;
            }
            if !self.valid_dropdown_setting(by.setting) {
                return Err(format!(
                    "{}{} names an ingredients setting that this plan never declared",
                    at, who
                ));
            }
            if r.spec.ingredients_from.is_some() {
                armed_by_recipe[by.setting.index - 1] += 1;
            }
            if by.describes {
                described[by.setting.index - 1] += 1;
            }
            // The presets are RENDERED into the dropdown's description at the
            // settings stage, so they have to be renderable there. The data
            // planner checks the same thing in its own loop with the same
            // sentence; this is what brings the check forward to the stage that
            // needs it.
            for c in &by.choices {
                self.validate_ingredients(at, &who, Some(&r.spec.category), &c.ingredients)?;
                self.validate_no_duplicates(at, &who, prefix, &c.ingredients)?;
            }
        }

        for t in &self.techs {
            let who = format!("the technology {}", t.name);
            // Exactly one cost source is the data planner's sentence, and it
            // is the one an author reads best; everything below assumes it
            // held.
            if named_cost_sources(&t.spec) != 1 {
                continue;
            }
            if let Some(by) = &t.spec.cost_by {
                if !self.valid_dropdown_setting(by.setting) {
                    return Err(format!(
                        "{}{} names a cost setting that this plan never declared",
                        at, who
                    ));
                }
            }
            if let Some(cc) = &t.spec.cost_from {
                let has_tier = match &t.spec.cost_by {
                    Some(by) => {
                        armed_by_tech[by.setting.index - 1] += 1;
                        true
                    }
                    None => false,
                };
                self.validate_custom_cost(at, &who, cc, has_tier)?;
            }
            let by = match &t.spec.cost_by {
                Some(by) if by.describes => by,
                _ => continue,
            };
            // A MARKED DECLARATION THAT COMPOSES NOTHING IS REFUSED, because
            // accepting it would hand the dropdown an empty description while
            // another declaration could have described it, silently. A
            // `CostBy` with no `CostFrom` beside it is the only shape that can
            // do it: a cost preset line names a technology and the switch line
            // names the pack setting, so with no `CostFrom` there is no
            // language and no field to name and nothing is composed at all. A
            // recipe's `IngredientsBy` always composes the ladder line, which
            // is something, so it may describe whether or not a text setting
            // sits beside it.
            //
            // IT ASKS FOR `CostFrom` RATHER THAN
            // `cost_dropdown_composes_preset_lines`, and that is the
            // sentence's doing: the predicate is also false when `CostFrom`
            // names a packs setting this plan never declared, and
            // `validate_custom_cost` above has already refused that with the
            // sentence an author reads best.
            if t.spec.cost_from.is_none() {
                return Err(format!(
                    "{}the technology {} is marked with Describes on the setting {}, but a CostBy with no CostFrom composes nothing onto a dropdown",
                    at,
                    t.name,
                    self.settings[by.setting.index - 1].emitted_name(prefix)
                ));
            }
            described[by.setting.index - 1] += 1;
        }

        // The composed description is ONE declaration's presets, so two of
        // them reaching one dropdown is a settings screen showing a list that
        // belongs to the other declaration. The two counts are separate
        // because the sentence names what the author wrote; a dropdown armed
        // by one recipe AND one technology is not refused here, and which of
        // the two describes it is `composed_dropdown_presets`' answer: the
        // marked one, or the technology's where neither is marked, because
        // that walk runs second.
        //
        // AND TWO MARKED DECLARATIONS ARE THE ONE SHAPE THAT HAS NO ANSWER,
        // which is what the third sentence refuses.
        //
        // IT RUNS AFTER BOTH WALKS, so on a plan with two problems the arm's
        // own sentence wins: an ill formed arm is refused where it is walked,
        // and only a plan whose arms are all well formed reaches here.
        for (i, s) in self.settings.iter().enumerate() {
            if armed_by_recipe[i] > 1 {
                return Err(format!(
                    "{}the setting {} takes a text setting from more than one recipe; one dropdown composes one description",
                    at,
                    s.emitted_name(prefix)
                ));
            }
            if armed_by_tech[i] > 1 {
                return Err(format!(
                    "{}the setting {} takes a text setting from more than one technology; one dropdown composes one description",
                    at,
                    s.emitted_name(prefix)
                ));
            }
            if described[i] > 1 {
                return Err(format!(
                    "{}the setting {} is described by more than one declaration; a dropdown shows one declaration's presets, so mark exactly one of them with Describes",
                    at,
                    s.emitted_name(prefix)
                ));
            }
        }

        let bindings = self.text_setting_bindings();
        for (i, s) in self.settings.iter().enumerate() {
            if !is_text(s.kind) {
                continue;
            }
            if bindings[i].count == 0 {
                // A FIELD THE PLAYER CAN EDIT THAT CHANGES NOTHING is worse
                // than a missing feature: it is a promise in the settings
                // screen that the mod does not keep.
                return Err(format!(
                    "{}the setting {} is declared and nothing reads it; a text setting must be bound to one recipe or technology",
                    at, s.name
                ));
            }
            if bindings[i].count > 1 {
                return Err(format!(
                    "{}the setting {} is read by more than one recipe or technology; a text setting serves exactly one",
                    at, s.name
                ));
            }
        }

        // A RESEARCH NUMBER IS BOUND ONCE TOO, and the composed description is
        // why: it names ONE dropdown as the thing deciding while the field is
        // 0, so a setting two technologies priced themselves with would be
        // described by whichever of them composed last.
        //
        // A NUMBER NOTHING PRICES RESEARCH WITH IS STILL SHARED FREELY: one
        // double behind two recipes' crafting time is a mod-wide speed dial
        // and nothing ever says anything about it, so `research` is what turns
        // a second reader into a refusal, and only a setting some CustomCost
        // names as its count or its seconds carries it.
        //
        // A HANDLE FROM ANOTHER PLAN IS SKIPPED rather than followed, exactly
        // as `craft_time_bound_settings` skips one: a bad reference cannot
        // mark the wrong setting here, and the walks above are what refuse it
        // by name.
        //
        // AND SO IS A DECLARATION THAT HAS NOT SAID WHAT IT COSTS, which is
        // the step past the walks above make, carried into this one: a recipe
        // naming CraftTime beside CraftTimeFrom, and a technology whose cost
        // sources are not exactly one, are answered by "pick one" and "exactly
        // one", and a reader counted out of such a declaration would put this
        // rule's sentence in front of the one its author reads best.
        //
        // IT RUNS AFTER THE TEXT RULE, so a plan carrying both problems is
        // answered by the text's sentence: that rule is the older one and the
        // one an author reads faster.
        let mut readers = alloc::vec![0usize; self.settings.len()];
        let mut research = alloc::vec![false; self.settings.len()];
        for r in &self.recipes {
            // A recipe holding a crafting time AND a handle to one is stepped
            // past whole: it has not said what it costs to make, the data
            // planner says so, and this walk has nothing to add in front of
            // that.
            if r.spec.craft_time != 0.0 && r.spec.craft_time_from.index != 0 {
                continue;
            }
            if self.valid_double_setting(r.spec.craft_time_from) {
                readers[r.spec.craft_time_from.index - 1] += 1;
            }
        }
        for t in &self.techs {
            // The same step past against the same predicate the validator's
            // own technology walk uses, so the two agree by construction.
            if named_cost_sources(&t.spec) != 1 {
                continue;
            }
            if let Some(cc) = &t.spec.cost_from {
                for h in [cc.count, cc.seconds] {
                    if self.valid_int_setting(h) {
                        readers[h.index - 1] += 1;
                        research[h.index - 1] = true;
                    }
                }
            }
        }
        for (i, s) in self.settings.iter().enumerate() {
            if research[i] && readers[i] > 1 {
                return Err(format!(
                    "{}the setting {} is read as a research count or time by more than one declaration; a custom cost's number serves exactly one",
                    at, s.name
                ));
            }
        }
        Ok(())
    }

    /// Every rule about what a text setting DECLARES, and both planners run
    /// it: the settings stage renders the declared list into the description,
    /// and the data stage emits it whenever the text says `default`.
    ///
    /// THE ROUND TRIP IS THE POINT. The description tells the player "this is
    /// what default means", and a player who copies it and changes one number
    /// must get a text this library reads. A declared list that renders into
    /// something the language refuses is a field nobody can edit, so it is
    /// refused here rather than shipped.
    ///
    /// IT ALSO GUARDS THE SEAM, which is why every later reach through the
    /// language table is behind this call: see the comment on the first check
    /// in the loop.
    pub(crate) fn validate_text_settings(&self, prefix: &str) -> Result<(), String> {
        let at = "fkrecipes: ";
        let bindings = self.text_setting_bindings();
        for (i, s) in self.settings.iter().enumerate() {
            let (kind, who) = match s.kind {
                SettingKind::Ingredients => (
                    ListKind::Recipe,
                    format!("the ingredients setting {}", s.name),
                ),
                SettingKind::Packs => (ListKind::Packs, format!("the packs setting {}", s.name)),
                _ => continue,
            };
            // THE SEAM'S GUARD, and the reason this validator is the one that
            // carries it: both planners run it before any loop that could
            // reach the language, and a text setting is the only declaration
            // that reaches it at all. A setting whose constructor did not
            // install the table has nothing to parse or render it with, so it
            // is refused by name rather than dereferenced. A packs setting
            // needs the custom-cost resolver as well, which only the packs
            // constructor installs. See
            // [`crate::ingredient_list::Language`].
            if self.language.is_none()
                || (s.kind == SettingKind::Packs && self.custom_cost.is_none())
            {
                return Err(format!(
                    "{}the text setting {} was declared without the ingredient language; declare it through IngredientsSetting or PacksSetting",
                    at, s.name
                ));
            }
            let lang = self.installed_language();
            let category = bindings[i].category.as_str();
            let entries = if s.kind == SettingKind::Ingredients {
                // The ordinary declared-ingredient rules, naming the SETTING:
                // the finiteness, zero and ceiling checks have to clear before
                // the renderer sees an amount, because a non-finite one renders
                // as text that does not read back.
                self.validate_ingredients(at, &who, Some(category), &s.def_ings)?;
                self.declared_list(prefix, &s.def_ings)
            } else {
                // A DECLARED PACK LIST THAT IS EMPTY IS REFUSED, exactly as a
                // hand-rolled unit's is and for the same reason: the engine
                // LOADS such a cost (measured) and the player gets a research
                // that completes instantly.
                if s.def_packs.is_empty() {
                    return Err(format!(
                        "{}{} declares no science pack; research takes at least one",
                        at, who
                    ));
                }
                validate_declared_packs(at, &who, &s.def_packs)?;
                declared_pack_list(&s.def_packs)
            };
            let rendered = (lang.render)(&ListText::List(entries));
            // THE PARSE IS THE WHOLE CHECK. Whatever it reads back is the
            // rendering again: the renderer writes one canonical form and the
            // parser reads that form to the same list, which is the identity
            // both halves hold as a property over generated lists. So a
            // declared default is refused exactly when the language REFUSES
            // the rendering, and a comparison behind this parse would be a
            // branch nothing can reach.
            //
            // The setting name in these messages is the DECLARED one wrapped
            // in what kind of setting it is, because this refusal is the
            // author's to fix and not the player's: they read "the ingredients
            // setting rivet-ingredients, entry 2 (...)" rather than a bare
            // prefixed name.
            //
            // THE RENDERING GOES IN AS BYTES: the parser reads a stored
            // setting's bytes, and this check has to be the same call the data
            // path makes. A `&str`'s bytes are UTF-8 by construction, so the
            // language's not-text guard cannot fire on this side.
            (lang.parse)(rendered.as_bytes(), kind, category, &who, &AllPresent)?;
        }
        Ok(())
    }

    /// The declared list as the description shows it. Reached only from the
    /// text-setting arm of `plan_settings`, which `validate_text_settings` has
    /// already accepted.
    fn rendered_default(&self, prefix: &str, s: &SettingDecl) -> String {
        (self.installed_language().render)(&ListText::List(match s.kind {
            SettingKind::Packs => declared_pack_list(&s.def_packs),
            _ => self.declared_list(prefix, &s.def_ings),
        }))
    }

    /// A declared list as the language sees it: the ladder's FIRST RUNG for a
    /// laddered ingredient, and this plan's own emitted name for one that
    /// names an item the plan declares.
    ///
    /// The first rung is what the author wrote, and a rendering is the
    /// author's list rather than a report about the game as loaded: the word
    /// `default` is what the player leaves in the field, and the ladder is
    /// walked behind it.
    pub(crate) fn declared_list(&self, prefix: &str, ings: &[Ingredient]) -> IngredientList {
        let mut entries = Vec::with_capacity(ings.len());
        for ing in ings {
            entries.push(ListEntry {
                name: self.declared_head(prefix, ing),
                amount: ing.amount,
            });
        }
        IngredientList { entries }
    }

    /// The name a declared ingredient WILL RESOLVE TO if nothing is missing:
    /// a ladder's first rung, or this plan's own item under its emitted name.
    ///
    /// ONE ANSWER FOR TWO CALLERS, which is the point of writing it out. The
    /// description above shows these names and
    /// [`Lib::validate_no_duplicates`](crate::Lib) compares them; the two
    /// spelling the same rule apart from each other is how a list could be
    /// described as one thing and refused as another. Its precondition is a
    /// validated list, so the index is safe.
    pub(crate) fn declared_head(&self, prefix: &str, ing: &Ingredient) -> String {
        match ing.candidates.first() {
            Some(first) => first.clone(),
            None => self.items[ing.item.index - 1].emitted_name(prefix),
        }
    }

    /// What a dropdown setting composes into its description, decided by the
    /// same two walks the binding validator decides with: a declaration those
    /// step past has nothing composed for it here either.
    ///
    /// AN INGREDIENT DROPDOWN WITH NO TEXT SETTING BESIDE IT STILL COMPOSES
    /// ONE LINE, which is why the text index is an `Option` rather than a
    /// reason to answer `None`. Without a text setting there is no language to
    /// read a preset out in and no second field to name, so the preset lines
    /// and the switch line go; the LADDER line stays,
    /// because the ladder is what that dropdown's presets do on a mod set that
    /// lacks a name, and that is true whether or not anybody can type over
    /// them. See [`DROPDOWN_LADDER_LINE`].
    ///
    /// `describes` WINS, AND WITHOUT IT THE RULE IS POSITIONAL: recipes then
    /// technologies, in declaration order, with the LAST one winning, which is
    /// what a plan that never sets the field keeps. Two recipes cannot put a
    /// text setting on one dropdown, and neither can two technologies: both
    /// are refused at plan validation. What remains is a dropdown a recipe and
    /// a technology both name, which the two text settings' own "read by
    /// exactly one" rule does not forbid; the technology's presets are the ones
    /// composed unless the recipe says otherwise.
    ///
    /// IT IS THE ONE PLACE THE WINNER IS CHOSEN, and every reader asks it: this
    /// module composes from it, `dropdown_composes_ladder_line` is a test on
    /// its kind, the locale checker turns it into an obligation, and
    /// `composed_game_key_advisories` walks the cost arm's keys.
    ///
    /// IT IS TOTAL. Two marked declarations over one dropdown are refused by
    /// `validate_bindings`, and among marked ones the last still wins here, so
    /// the locale checker, which validates nothing, gets an answer rather than
    /// a panic.
    pub(crate) fn composed_dropdown_presets(&self, index: usize) -> Option<Presets<'_>> {
        let mut found = None;
        let mut marked = false;
        for r in &self.recipes {
            let by = match &r.spec.ingredients_by {
                Some(by) => by,
                None => continue,
            };
            if !r.spec.ingredients.is_empty() || !self.valid_dropdown_setting(by.setting) {
                continue;
            }
            if by.setting.index != index || (marked && !by.describes) {
                continue;
            }
            let text = match r.spec.ingredients_from {
                Some(h) if self.valid_ingredients_setting(h) => Some(h.index),
                _ => None,
            };
            found = Some(Presets::Ingredients(&by.choices, text));
            marked = marked || by.describes;
        }
        for t in &self.techs {
            if !self.cost_dropdown_composes_preset_lines(&t.spec) {
                continue;
            }
            let by = t
                .spec
                .cost_by
                .as_ref()
                .expect("the predicate saw a dropdown");
            let cc = t.spec.cost_from.as_ref().expect("the predicate saw a cost");
            if by.setting.index != index || (marked && !by.describes) {
                continue;
            }
            found = Some(Presets::Cost(&by.choices, cc.packs.index));
            marked = marked || by.describes;
        }
        found
    }

    /// The ONE SPELLING of "this technology's cost dropdown has a preset list
    /// composed onto its description". Two readers need exactly this predicate
    /// and neither may spell it again: `composed_dropdown_presets`, which is what
    /// composes the lines AND what makes that dropdown's
    /// `[mod-setting-description]` entry required, and
    /// `composed_game_key_advisories`, which says the one thing this library
    /// has to say about the GAME's key those lines name.
    ///
    /// A SECOND SPELLING WAS WHAT MADE THE GUARDS UNTESTABLE. Each of the four
    /// conditions here is the composition's own, so a dropped one is a composed
    /// line the walk does not know about or a walk naming a line nothing
    /// composes; with one spelling, the required-description findings exercise
    /// every one of them and the advisory walk inherits that for free.
    pub(crate) fn cost_dropdown_composes_preset_lines(&self, spec: &TechSpec) -> bool {
        let (by, cc) = match (&spec.cost_by, &spec.cost_from) {
            (Some(by), Some(cc)) => (by, cc),
            _ => return false,
        };
        named_cost_sources(spec) == 1
            && self.valid_dropdown_setting(by.setting)
            && self.valid_packs_setting(cc.packs)
    }

    /// The word that says where the setting at `other` sits on the settings
    /// screen relative to the one at `self_index`: above or below.
    ///
    /// IT COMPARES THE EMITTED ORDER STRINGS, the same ones `plan_settings`
    /// writes into the prototypes, because that is what the engine sorts by. A
    /// composed sentence that said "the option chosen above" would otherwise be
    /// a guess about a declaration order the consumer is free to choose, and
    /// `order_after` and the Legacy constructors both let them choose one where
    /// the guess is wrong.
    pub(crate) fn relative_order(&self, self_index: usize, other: usize) -> &'static str {
        if self.emitted_order(other) < self.emitted_order(self_index) {
            "above"
        } else {
            "below"
        }
    }

    /// The sentence on a TEXT setting that says what decides while it holds the
    /// reserved word.
    ///
    /// IT OPENS WITH THE INSTRUCTION AND NOT WITH THE CONDITION, and the
    /// research number's own sentence opens the same way (see
    /// `research_range_line`): both fields carry a value that means "not
    /// customised", one of them a word and the other a zero, and a player who
    /// meets the two on one screen meets one sentence shape rather than two.
    /// The verb differs with what is on the other side, a dropdown that DECIDES
    /// against a list that APPLIES, and that is the whole of what the two arms
    /// differ by.
    pub(crate) fn text_switch_line(&self, i: usize) -> String {
        match self.text_switch_dropdown(i) {
            Some(d) => format!(
                "\nLeave this as default and the option chosen {} decides; anything else applies instead.",
                self.relative_order(i, d)
            ),
            None => String::from(
                "\nLeave this as default and this mod's own list applies; anything else applies instead.",
            ),
        }
    }

    /// The dropdown setting that decides while the text setting at `i` says
    /// `default`, or `None` when the declaration that reads it has no dropdown.
    ///
    /// ONE WALK, TWO READERS: the composition on the text setting and the one
    /// on the dropdown itself have to agree about which pair they are
    /// describing, and a second walk spelling the same condition is how the two
    /// could describe different pairs.
    pub(crate) fn text_switch_dropdown(&self, i: usize) -> Option<usize> {
        for r in &self.recipes {
            match r.spec.ingredients_from {
                Some(h) if self.valid_ingredients_setting(h) && h.index - 1 == i => {}
                _ => continue,
            }
            if let Some(by) = &r.spec.ingredients_by {
                if r.spec.ingredients.is_empty() && self.valid_dropdown_setting(by.setting) {
                    return Some(by.setting.index - 1);
                }
            }
            return None;
        }
        for t in &self.techs {
            let cc = match &t.spec.cost_from {
                Some(cc) => cc,
                None => continue,
            };
            if named_cost_sources(&t.spec) != 1
                || !self.valid_packs_setting(cc.packs)
                || cc.packs.index - 1 != i
            {
                continue;
            }
            if let Some(by) = &t.spec.cost_by {
                if self.valid_dropdown_setting(by.setting) {
                    return Some(by.setting.index - 1);
                }
            }
            return None;
        }
        None
    }

    /// Whether the TEXT setting at `i` is the field this plan discloses the
    /// ladder on, and it is the ONE SPELLING of that question: `plan_settings`
    /// composes from it and `guarded_text_description` guards what was
    /// composed, so the emitted description and the guard cannot disagree about
    /// which shape they are looking at.
    ///
    /// THE EXCLUSION IS BY VOCABULARY, which is narrower than "unless a
    /// dropdown sits beside it" and narrower again than "unless the dropdown
    /// beside it composes a ladder line". There is exactly ONE dropdown
    /// sentence, [`DROPDOWN_LADDER_LINE`], and it is in the INGREDIENT
    /// vocabulary: it talks about an entry and about what an option crafts. So
    /// an ingredient text beside a dropdown that composes it drops its own
    /// copy, because the two would say one thing twice on one screen; a PACKS
    /// text never drops it, whatever the dropdown beside it shows, because no
    /// dropdown composes the packs sentence and the ingredient one says nothing
    /// about a research taking fewer packs.
    ///
    /// THE PAIR THAT MAKES THE DIFFERENCE VISIBLE is a dropdown a recipe and a
    /// technology both name. Where the recipe describes it, the dropdown
    /// carries the ingredient sentence over the recipe's presets, the recipe's
    /// ingredient text drops its line, and the packs text beside the same
    /// dropdown KEEPS its packs line. Under the earlier rule, "no dropdown
    /// beside it", a packs text beside a `CostBy` tier said nothing about the
    /// ladder anywhere a player looks, and the log is not a disclosure.
    pub(crate) fn text_carries_ladder_line(&self, i: usize) -> bool {
        if self.settings[i].kind == SettingKind::Packs {
            return true;
        }
        match self.text_switch_dropdown(i) {
            Some(d) => !self.dropdown_composes_ladder_line(d),
            None => true,
        }
    }

    /// Whether `plan_settings` puts [`DROPDOWN_LADDER_LINE`] onto the dropdown
    /// setting at `d`.
    ///
    /// IT IS A TEST ON `composed_dropdown_presets`' OWN ANSWER and not a second
    /// walk. Both arms of the ingredient shape push the line, the bare one and
    /// the one with a text setting beside it, and the cost shape pushes none,
    /// so the kind is the whole of the question. Spelling the walk again is
    /// what a rule with a `describes` branch in it cannot survive: the copy
    /// would answer for the positional rule while the composition answered for
    /// the marked one.
    pub(crate) fn dropdown_composes_ladder_line(&self, d: usize) -> bool {
        matches!(
            self.composed_dropdown_presets(d + 1),
            Some(Presets::Ingredients(..))
        )
    }

    /// The settings a `CustomCost` prices a research with, and which dropdown
    /// decides while each of them is 0. Both planners and the locale checker
    /// ask it, so the description, the locale obligation and the range sentence
    /// are decided once.
    ///
    /// IT STEPS PAST EXACTLY WHAT `validate_bindings` STEPS PAST: a technology
    /// naming some other number of cost sources is answered by "exactly one",
    /// and nothing validated its `CustomCost`, so composing a range out of
    /// bounds nobody checked would be a description about a declaration the
    /// data planner refuses.
    pub(crate) fn research_number_settings(&self) -> Vec<ResearchNumber> {
        let mut out: Vec<ResearchNumber> = Vec::with_capacity(self.settings.len());
        for _ in &self.settings {
            out.push(ResearchNumber::default());
        }
        for t in &self.techs {
            let cc = match &t.spec.cost_from {
                Some(cc) => cc,
                None => continue,
            };
            if named_cost_sources(&t.spec) != 1 {
                continue;
            }
            let dropdown = match &t.spec.cost_by {
                Some(by) if self.valid_dropdown_setting(by.setting) => Some(by.setting.index - 1),
                _ => None,
            };
            for h in [cc.count, cc.seconds] {
                if self.valid_int_setting(h) {
                    out[h.index - 1] = ResearchNumber {
                        bound: true,
                        dropdown,
                    };
                }
            }
        }
        out
    }

    /// What a research number's description says about the range it takes, and
    /// about what 0 means where a dropdown decides.
    ///
    /// THE NUMBERS COME OUT OF THE AMOUNT FORMATTER the language already pins
    /// byte for byte across the two halves, rather than out of either
    /// language's own float formatting: this sentence is compared in the mirror
    /// transcript, and two standard libraries agree about 100000 right up until
    /// they do not.
    ///
    /// A BOUND IS THERE BECAUSE `validate_custom_cost` PROVED IT. Both planners
    /// run it in front of this walk, and `research_number_settings` steps past
    /// exactly the declarations it steps past, so the maximum is declared and
    /// the minimum is too.
    ///
    /// BESIDE A DROPDOWN IT LEADS WITH THE SENTINEL, in `text_switch_line`'s
    /// own shape. 0 there is not a number in a range, it is the way this field
    /// says "not customised", and the range is the secondary fact: a sentence
    /// that opened with "a whole number from 0 to N" made the sentinel read as
    /// the bottom of a range a player might pick deliberately. The verb is
    /// "supplies the number", which is the verb the informational log line
    /// already uses for the same relationship. Without a dropdown there is no
    /// sentinel and the sentence is the range alone.
    fn research_range_line(&self, i: usize, s: &SettingDecl, dropdown: Option<usize>) -> String {
        let amount = self.installed_language().format_amount;
        let max = amount(s.spec.max.unwrap_or(0.0));
        match dropdown {
            Some(d) => format!(
                "\nLeave this at 0 and the option chosen {} supplies the number; otherwise a whole number up to {}.",
                self.relative_order(i, d),
                max
            ),
            None => format!(
                "\nA whole number from {} to {}.",
                amount(s.spec.min.unwrap_or(0.0)),
                max
            ),
        }
    }

    /// The dropdown's composed description: the consumer's own entry, then one
    /// line per preset, its LOCALISED label followed by what it means.
    ///
    /// THREE SHAPES AND NOT TWO. An INGREDIENT dropdown with a text setting
    /// beside it carries the preset lines, the ladder line and the switch line;
    /// one with NO text setting beside it carries the ladder line alone and
    /// returns early, because every other line needs either a language or a
    /// second field; a COST dropdown carries the preset lines and the switch
    /// line and no ladder, because its presets render no list of internal
    /// names.
    fn dropdown_description(&self, prefix: &str, full: &str, presets: &Presets<'_>) -> Value {
        let i = self
            .settings
            .iter()
            .position(|d| d.emitted_name(prefix) == full)
            .expect("a composed dropdown is one of this plan's settings");
        let mut params = alloc::vec![self.setting_description_head(i, full)];
        let text = match *presets {
            // A DROPDOWN WITH NO TEXT SETTING BESIDE IT COMPOSES THE LADDER
            // LINE AND NOTHING ELSE. There is no preset to read out, because
            // rendering one needs a language and only `ingredients_setting`
            // installs one, and no switch line, because there is no second
            // field to name. The LADDER is the one thing that is still
            // true of this shape, and it is true of every plan: a preset whose
            // entry this mod set does not have resolves onto the next name the
            // entry offers or is left out. See [`DROPDOWN_LADDER_LINE`].
            //
            // IT IS A CONSTANT AND IT LINKS NOTHING. The whole of what this arm
            // composes is the consumer's own key and one string literal, so a
            // plan with no text setting still links no parser, no renderer, no
            // amount formatter and no custom-cost resolver: see
            // `rust/examples/notext`, which is that plan and is measured.
            Presets::Ingredients(_, None) => {
                params.push(Value::string(DROPDOWN_LADDER_LINE));
                return localised_group(&params);
            }
            Presets::Ingredients(choices, Some(text)) => {
                for c in choices {
                    let list = self.declared_list(prefix, &c.ingredients);
                    let rendered = (self.installed_language().render)(&ListText::List(list));
                    params.push(preset_element(
                        full,
                        &c.value,
                        alloc::vec![Value::Str(format!(
                            "{}{}",
                            INGREDIENT_PRESET_HEAD, rendered
                        ))],
                    ));
                }
                // THEN THE LADDER, WHICH IS THE ONE THING THE PRESET LINES
                // ABOVE DO NOT SAY: a preset is what the author wrote, and what
                // the game builds from it is what this mod set could resolve.
                // See [`DROPDOWN_LADDER_LINE`].
                params.push(Value::string(DROPDOWN_LADDER_LINE));
                text
            }
            Presets::Cost(choices, text) => {
                for c in choices {
                    params.push(preset_element(full, &c.value, cost_preset_tail(c)));
                }
                text
            }
        };
        // THE SWITCH LINE LAST, because it is about the field beside this one
        // rather than about any preset above it.
        let i = self
            .settings
            .iter()
            .position(|d| d.emitted_name(prefix) == full)
            .expect("a composed dropdown is one of this plan's settings");
        params.push(Value::Str(dropdown_switch_line(
            self.relative_order(i, text - 1),
        )));
        localised_group(&params)
    }
}

/// What a dropdown composes its description from, and which text setting sits
/// beside it: a 1-BASED index, the shape every handle in this crate carries.
///
/// THE INGREDIENT ARM'S INDEX IS OPTIONAL AND THE COST ARM'S IS NOT. An
/// ingredient dropdown with no text setting beside it still composes one line,
/// the ladder; a cost dropdown with no pack setting beside it composes nothing
/// at all and never reaches here, because `cost_dropdown_composes_preset_lines`
/// is what lets it through.
pub(crate) enum Presets<'a> {
    Ingredients(&'a [IngredientChoice], Option<usize>),
    Cost(&'a [CostChoice], usize),
}

/// What a research count or time setting composes from: that it backs one at
/// all, and which dropdown decides while it is 0.
#[derive(Default, Clone, Copy)]
pub(crate) struct ResearchNumber {
    pub(crate) bound: bool,
    pub(crate) dropdown: Option<usize>,
}

/// The sentence appended to a DROPDOWN's composed description: the text setting
/// beside it wins whenever it is not on the word.
fn dropdown_switch_line(where_: &str) -> String {
    format!(
        "\nThe setting {} applies instead while it does not say default.",
        where_
    )
}

/// How many cost sources a technology declares. Exactly one is the rule and
/// the data planner is where it is refused, so every walk that steps past a
/// technology naming some other number asks this one question.
///
/// `cost_by` AND `cost_from` COUNT AS ONE, because they are one cost: the
/// dropdown is the tier and the three settings overwrite it field by field.
/// Every other pairing is still two, so `unit` beside either is refused
/// exactly as it was.
pub(crate) fn named_cost_sources(spec: &TechSpec) -> usize {
    [
        !spec.cost_of.is_empty(),
        spec.unit.is_some(),
        spec.cost_by.is_some() || spec.cost_from.is_some(),
    ]
    .iter()
    .filter(|x| **x)
    .count()
}

/// A packs setting's declared list as the language sees it. A pack is an item
/// with a whole amount, so every entry takes the item form.
pub(crate) fn declared_pack_list(packs: &[Pack]) -> IngredientList {
    let mut entries = Vec::with_capacity(packs.len());
    for p in packs {
        entries.push(ListEntry {
            name: p.name.clone(),
            amount: Amount::Item(p.amount),
        });
    }
    IngredientList { entries }
}

/// A declared pack list's numbers and names, in the order
/// [`Lib::validate_unit`](crate::Lib) asks them of a hand-rolled unit's packs.
fn validate_declared_packs(at: &str, who: &str, packs: &[Pack]) -> Result<(), String> {
    for p in packs {
        if p.amount < 1 {
            return Err(format!(
                "{}{} has a science pack amount below 1, which the engine refuses",
                at, who
            ));
        }
        if p.amount > MAX_EXACT_INT {
            return Err(format!(
                "{}{} declares a science pack amount a Lua double cannot hold exactly: {}",
                at, who, p.amount
            ));
        }
        // EVERY RUNG, as the unit's own check asks it: an empty name in a
        // fallback is a rung that can never resolve.
        if p.name.is_empty() || p.fallbacks.iter().any(|f| f.is_empty()) {
            return Err(format!(
                "{}{} names a science pack with an empty name",
                at, who
            ));
        }
    }
    Ok(())
}

/// The whole `localised_description` a TEXT setting is emitted with: the
/// consumer's own entry, then the four or five things this library owes the
/// player about the field beside it.
///
/// SIX PARAMETERS AT MOST, and none of them a table beyond the consumer's key
/// and the [`locale_ref`] wrapper around it, so the twenty-parameter ceiling
/// [`MAX_LOCALISED_PARAMS`] records is nowhere near reached and this shape
/// needs no nesting rule of its own.
///
/// THE LINES ARE THE ANSWER TO WHAT A CLIENT MEASUREMENT FOUND. A player
/// standing in the Mod Settings screen reads the tooltip whole (measured on
/// 2.0.77 on a DROPDOWN's composed description, one line per preset: seven
/// lines rendered readable and unclipped; the ceilings on a composed
/// description are the parameter count [`MAX_LOCALISED_PARAMS`] holds and the
/// nesting depth its comment records, neither of which is a line count), so
/// the description is where the library can say what the field takes; the
/// closed dropdown's LABEL beside it is truncated at about 37 characters, which
/// is why nothing a player needs may live in a label. The default line shows
/// the list the word `default` stands for, in the internal names the field
/// actually takes; the ladder line, where this field has one, says that the
/// list the game builds from it can be shorter than the list shown; the format
/// line says what to write and states the ceiling; the switch line says which
/// of the two fields is
/// deciding, which the screen cannot show because it has no conditional
/// visibility at all (measured); and the fallback line says what a text this
/// library cannot use costs, which before it was stated nowhere a player
/// looks.
///
/// ONE COMPOSITION, TWO READERS. The settings planner emits this;
/// [`Lib::check_locale`](crate::Lib) asks the same function for the same shape
/// with the list left out, so a line deleted here is a finding rather than a
/// silent loss. See `check_text_description`.
///
/// `ladder` SAYS WHETHER THIS FIELD IS THE PLACE TO DISCLOSE THE LADDER, and it
/// is the caller's answer rather than this function's because the walk that
/// knows is `text_carries_ladder_line`: beside an INGREDIENT dropdown the lists
/// the ladder is about are that dropdown's presets and
/// [`DROPDOWN_LADDER_LINE`] carries the sentence there, while a cost dropdown
/// carries none and leaves the line to this field. It stays directly under the
/// default line where it is composed at all, because that is the list it is
/// about.
///
/// `ingredients` SAYS WHICH OF THE TWO TEXT SETTINGS THIS IS, and it decides
/// two things: the ladder line's vocabulary, and whether the format line names
/// the word `none`. See [`text_ladder_line`] and [`text_format_line`].
pub(crate) fn text_description(
    head: Value,
    rendered: &str,
    switch_line: &str,
    ingredients: bool,
    ladder: bool,
) -> Value {
    let mut params = alloc::vec![
        Value::string(""),
        head,
        Value::Str(format!("\ndefault: {}", rendered)),
    ];
    if ladder {
        params.push(Value::string(text_ladder_line(ingredients)));
    }
    params.push(Value::Str(text_format_line(ingredients)));
    params.push(Value::string(switch_line));
    params.push(Value::string(TEXT_FALLBACK_LINE));
    Value::Arr(params)
}

/// The whole `localised_description` a RESEARCH NUMBER is emitted with: the
/// consumer's own entry, then the range the field takes.
///
/// IT IS THE ONLY PLACE THE RANGE IS STATED. The settings screen shows a
/// numeric field with no visible bounds, and 0 there means something the player
/// cannot guess: the dropdown beside it decides. Both sentences live in
/// `research_range_line`, and this is the shape they are emitted in.
pub(crate) fn number_description(head: Value, range_line: &str) -> Value {
    Value::Arr(alloc::vec![
        Value::string(""),
        head,
        Value::string(range_line),
    ])
}

/// How this library references a locale key: the engine's own ALTERNATIVES
/// form, `{"?", {"section.key"}, "raw"}`, and never the bare `{"section.key"}`
/// it used to emit.
///
/// A BARE KEY THE GAME DOES NOT DEFINE COSTS THE WHOLE THING IT SITS IN, which
/// is measured and not argued (Factorio 2.0.77 build 84539, this repository's
/// client probe and its headless arm agreeing). On a SETTING, a composed
/// description holding one undefined key leaves the row with no info icon and
/// no tooltip at all, while every other row keeps both; on a RECIPE, the entire
/// description block disappears from the tooltip, taking the literal sentences
/// this library wrote itself with it. Neither costs an exit code, an engine
/// warning or a `fkrecipes: ` line, and the engine's own `--dump-data` says the
/// description is present either way, so no headless gate can see it.
///
/// AN UNDEFINED KEY IS A FAILED ALTERNATIVE for `?`, which is the fact the
/// whole form rests on: it does not resolve to the text `Unknown key: "..."`
/// and win, it fails, and the next alternative renders. Measured on the client
/// beside the failure it cures, in one screen: every `?` row renders and the
/// plain join does not.
///
/// THE RAW FALLBACK IS LAST, AND THAT IS A RULE AND NOT A STYLE. A plain string
/// alternative ALWAYS resolves, so a raw string anywhere but the end
/// short-circuits every alternative after it and the key would never be
/// consulted; and when every alternative fails the result is the LAST
/// alternative's own `Unknown key: "..."` marker, so a key in the last slot is
/// the marker this form exists to avoid. Both reasons point the same way.
///
/// IT COSTS ONE LEVEL OF DEPTH AND NOTHING ELSE. The wrapper occupies exactly
/// one parameter slot of the table holding it, the same as the bare key table
/// it replaces, against the measured ceilings [`MAX_LOCALISED_PARAMS`] records
/// (20 parameters per table, 20 levels of depth, no global table budget). The
/// library's realistic worst composition loses nothing it can reach: the
/// theoretical preset ceiling drops from 342 to 323 on a COST dropdown, and a
/// 323-preset description carrying 647 wrappers both loads and resolves in
/// full. An INGREDIENT dropdown's is 322 rather than 323, by arithmetic and
/// not by a second measurement: [`DROPDOWN_LADDER_LINE`] spends one parameter
/// slot a preset would otherwise have, and nothing else about the shape
/// differs.
pub(crate) fn locale_ref(section: &str, key: &str, raw: &str) -> Value {
    Value::Arr(alloc::vec![
        Value::string("?"),
        Value::Arr(alloc::vec![Value::Str(format!("{}.{}", section, key))]),
        Value::string(raw),
    ])
}

/// The OTHER wrapper, and it is a second function rather than a [`locale_ref`]
/// call because its two slots are not [`locale_ref`]'s two slots.
///
/// WHAT IT IS FOR. A recipe or a technology that carries a note and declares no
/// `description` used to be emitted as `{"", "<note>"}`, and a prototype's own
/// `localised_description` field WINS OVER the `[recipe-description]` or
/// `[technology-description]` entry the author wrote in their `.cfg`, so the
/// note stood in that description's place for the whole of that load. The
/// library cannot SEE a locale key; it can reference one that degrades to
/// nothing.
///
/// THE SHAPE, AND THE CRUX ROW THAT PICKED IT. Measured on Factorio 2.0.77
/// (build 84539, mac-arm64, steam) with a Lua-only probe mod calling
/// `localised_print` from `control.lua` and a second probe hanging each shape
/// on a base prototype at `data-final-fixes` under `--dump-data`:
///
/// ```text
/// {"?", {"", {"technology-description.X"}, "\n"}, ""} with X UNDEFINED
/// renders EMPTY.
/// ```
///
/// A CONCATENATION GROUP HOLDING AN UNDEFINED KEY IS ITSELF A FAILED
/// ALTERNATIVE, so the separator newline rides INSIDE the alternative and dies
/// with it. That is what makes this shape and not the flatter
/// `{"", {"?", {key}, ""}, "\n", "<note>"}`: the flat one renders a dangling
/// leading newline on every consumer who declares no entry, which is most of
/// them. With the key DEFINED the same shape renders `AUTHOR TECH DESC\n`, and
/// the note follows it; measured on the real base key
/// `technology-description.logistics` as well, and on `recipe-description`.
///
/// THE RAW FALLBACK IS LAST HERE TOO, and it is the EMPTY STRING rather than
/// prose: there is nothing to say where the author wrote no entry. The rule is
/// [`locale_ref`]'s own and the reason is the same, a plain string alternative
/// always resolves and short circuits everything after it, which is why the
/// source property test walks this shape too.
///
/// `None` IS THE KEY LENGTH, AND IT IS NOT A FORMALITY. The engine polices the
/// KEY SLOT at [`LOCALISED_ELEMENT_CEILING`] bytes like every other string
/// element, and a key is ONE element by definition: it cannot be chunked. The
/// engine's own prototype-name ceiling is 200 bytes (measured; `Name field is
/// too large. Max allowed size is: 200.`) and nothing in this library refuses a
/// shorter one, so `technology-description.` at 23 bytes over a 200-byte name
/// is a 223-byte element the engine refuses, and the composition would be the
/// lock-out the note exists to prevent. Above the length that fits, the key
/// form is DROPPED and the note is emitted alone exactly as it was before this
/// wrapper existed: the author's locale entry is displaced on those two names,
/// which is a tooltip and not a load failure. The fitting lengths are 181 bytes
/// of recipe name and 177 of technology name.
pub(crate) fn description_ref(kind: &str, name: &str) -> Option<Value> {
    let key = format!("{}-description.{}", kind, name);
    if key.len() > LOCALISED_ELEMENT_CEILING {
        return None;
    }
    Some(Value::Arr(alloc::vec![
        Value::string("?"),
        Value::Arr(alloc::vec![
            Value::string(""),
            Value::Arr(alloc::vec![Value::Str(key)]),
            Value::string("\n"),
        ]),
        Value::string(""),
    ]))
}

/// The sentence that says what to type and how much of it.
///
/// THE NUMBER IS [`MAX_TEXT`] AND NOT A DIGIT TYPED HERE. The limit the player
/// is told and the limit the parser enforces are one number by construction,
/// so a change to the ceiling cannot ship a description that lies about it.
/// That is held by a source property rather than by a value test, because a
/// typed 2000 and this expression render the same bytes while the ceiling
/// happens to be 2000: `the_ceiling_is_never_typed_into_a_message` forbids the
/// ceiling's own decimal rendering in every string literal this crate builds a
/// message out of. It is a constant, so naming it here links no part of the
/// language: a plan with no text setting still links no parser and no
/// renderer.
///
/// IT NAMES THE DEFAULT LINE rather than describing internal names in the
/// abstract, because that line is the copyable example, and copying it is
/// exactly what the composition is for. "AS ON THE DEFAULT LINE" IS TRUE OF
/// EVERY COMPOSITION THIS LIBRARY EMITS, which is why the words are not a row
/// count: the default line is directly above this one beside a dropdown, and
/// one line above it where the ladder line is composed. It used to be three
/// lines up.
pub(crate) fn text_format_line(ingredients: bool) -> String {
    let mut line = format!(
        "\nInternal names, as on the default line, up to {} characters.",
        MAX_TEXT
    );
    if ingredients {
        line.push_str(INGREDIENT_NONE_CLAUSE);
    }
    line
}

/// The word `none`, disclosed on the one setting kind that takes it.
///
/// THE WORD IS A FEATURE AND WAS DOCUMENTED NOWHERE A PLAYER LOOKS. `none`
/// empties an ingredient list, the recipe reaches the game with no ingredients
/// at all, and the recycling recipe the engine derives from it goes with it.
/// That is a legitimate thing for a player to want, so it is kept and said out
/// loud.
///
/// AND IT IS NOT SAID ON A PACKS SETTING, which is why it is a clause of its
/// own rather than part of the sentence above it. A packs list REFUSES the
/// word, with `research takes at least one science pack`: telling a player to
/// type a word the library turns down would be worse than saying nothing at
/// all.
pub(crate) const INGREDIENT_NONE_CLAUSE: &str =
    " The word none empties the list, so the recipe costs nothing to craft.";

/// What a RESOLVE-OR-DROP ladder costs the list a text setting renders, said on
/// the field that renders it, and packs and ingredients get different words for
/// the same rule.
///
/// THE LIBRARY COMPOSES THIS ITSELF, WHICH IS THE WHOLE OF WHY IT EXISTS. The
/// ladder gets no note on the emitted recipe or technology, deliberately, and
/// every place that says so used to justify it by quoting a disclosure "the
/// dropdown's own composed description already discloses it". No composition in
/// this library wrote that sentence: the live text was the PILOT CONSUMER'S own
/// locale entry, which a consumer is free to write differently or not at all,
/// so the library's reason for staying silent on the prototype rested on
/// somebody else's string (the consumer's third migration assessment, finding
/// 22). These three lines are that sentence, composed here. Every plan that
/// renders a list composes one of them, onto the field that renders the lists
/// it is about, and onto exactly one field of any pair: see
/// [`Lib::text_carries_ladder_line`](crate::Lib), which is where the choice is
/// made.
///
/// THREE CLAUSES AND NOT ONE, BECAUSE THE LADDER DOES THREE THINGS. A rung that
/// exists is SUBSTITUTED, a ladder that runs out is DROPPED, and two entries
/// that land on one name are MERGED with their amounts added. A sentence
/// promising only the substitution would be false on every plan that can reach
/// the second: `Ingredient::named(2, "a", &["b"])` with neither name in the
/// game leaves the entry out with a log line, a declared pack whose every rung
/// is absent is left out of the unit, and a research left with no pack at all
/// is emitted with none. THE MERGE IS DISCLOSED NOWHERE ELSE below a ceiling:
/// two entries whose ladders both land on `iron-plate` emit one ingredient of
/// the summed amount, a number in no tooltip and in no declaration, and
/// `merge_ingredient` records a note only where that sum crosses the engine's
/// own wall. The closing clause is what tells a player what to expect on the
/// screen they are looking at: a shorter list than the one this tooltip shows.
///
/// AND IT STOPS SHORT OF AN EMPTIED RECIPE, deliberately. A ladder that leaves
/// the recipe with NOTHING is a free craft rather than a shorter list, and it
/// carries [`ingredientless_note`](crate::data::ingredientless_note) on the
/// prototype instead.
///
/// IT IS NOT CONDITIONAL ON A DECLARED LADDER, and that is a deliberate
/// deviation from the narrower shape this round was asked for. A list with no
/// fallbacks anywhere in it still DROPS an entry the game does not have, which
/// is the half of the sentence that is always true, so conditioning on a
/// declared ladder would leave exactly the plans that can ONLY drop saying
/// nothing at all.
///
/// IT IS LEFT OUT ONLY WHERE SOMETHING BESIDE THE FIELD ALREADY SAYS IT. Beside
/// an INGREDIENT dropdown the lists a player chooses between are that
/// dropdown's presets and the ladder is disclosed there, by
/// [`DROPDOWN_LADDER_LINE`], on the field that renders them; composing this
/// line as well would say one thing twice on one screen, in two vocabularies,
/// about two different lists. Beside a COST dropdown it is composed, because
/// that dropdown renders no list and carries no ladder line, so this field is
/// the only one of the two where a player can read the rule at all. With no
/// dropdown the field's own DEFAULT LINE is the list that applies and this line
/// sits directly under it. `text_carries_ladder_line` is where the choice is
/// made, and it reads the same `text_switch_dropdown` walk the switch line
/// uses.
pub(crate) fn text_ladder_line(ingredients: bool) -> &'static str {
    if ingredients {
        INGREDIENT_LADDER_LINE
    } else {
        PACKS_LADDER_LINE
    }
}

/// [`text_ladder_line`]'s INGREDIENT arm: the entry goes, and what the player
/// crafts is shorter than what they read.
///
/// THE SUBJECT IS THE ENTRY AND NOT THE LIST, which is what let the sentence
/// lose a third of its bytes without losing a clause. "Where a list this mod
/// chose names something your mods do not have, the next name it offers is used
/// instead" spent eighteen words setting up a condition the shorter opening
/// states as a fact about the entry itself.
pub(crate) const INGREDIENT_LADDER_LINE: &str =
    "\nAn entry your mods lack takes the mod's next name for it or is left out; two landing on one name are added, so what you craft can be shorter than shown.";

/// [`text_ladder_line`]'s PACKS arm, in the packs vocabulary: a pack rather
/// than a name, and a research that takes fewer of them rather than a craft
/// that costs less.
///
/// THE CLOSING CLAUSE IS NOT THE INGREDIENT ONE WITH A WORD CHANGED. A research
/// unit is priced in packs and a recipe is crafted from a list, so "what you
/// craft" names nothing on a technology; and a research that loses every pack
/// it names is emitted with no pack at all, which "fewer packs than shown"
/// covers and "shorter than shown" reads past.
///
/// AND THE WORD IS "PACK" RATHER THAN "SCIENCE PACK" in the opening now. The
/// field takes nothing but packs, which its own default line shows and its
/// `[mod-setting-name]` entry says, so the longer name was spending eight bytes
/// on a distinction the screen had already made; the vocabulary that separates
/// this arm from the ingredient one is "pack" against "name" and "the research"
/// against "what you craft", which is what the two negative gate checks read.
pub(crate) const PACKS_LADDER_LINE: &str =
    "\nA pack your mods lack takes the mod's next name for it or is left out; two landing on one pack are added, so the research can take fewer packs than shown.";

/// The same disclosure on an INGREDIENT DROPDOWN, where the lists it is about
/// are the author's PRESETS rather than one default list.
///
/// ITS CLOSING CLAUSE SAYS "AN OPTION" BECAUSE THE LISTS IT IS ABOUT ARE
/// OPTIONS, and the player reading it is choosing between them: every preset on
/// that dropdown renders a list, and the ladder applies to whichever one they
/// land on, so what that option crafts can be shorter than what the option
/// itself shows. The text setting's arm says "what you craft" instead, because
/// there the list it is about is the one the field falls back to, on the
/// DEFAULT LINE, and there is only the one. That clause is the whole of the
/// difference between the two ingredient arms, which is why a gate that
/// separates them reads the ending rather than the opening.
///
/// THE LINE DIRECTLY ABOVE IT IS THE LAST PRESET, in the composition that has
/// presets; in the BARE composition, where no text setting sits beside the
/// dropdown, this line is the whole of what the library composes and the line
/// above it is the consumer's own entry. Naming the neighbour by what it is
/// rather than by a row count is deliberate: the count has moved twice.
///
/// AND IT IS THE ONLY PLACE THE LADDER IS DISCLOSED FOR THE PAIR. The text
/// setting beside this dropdown does not carry [`text_ladder_line`], because
/// the lists a player is choosing between are the presets on this row.
///
/// A COST DROPDOWN CARRIES IT NOT AT ALL: its preset is a localised label
/// followed by a localised technology name, so there is no rendered list of
/// internal names for a ladder to shorten. What a copied cost's own packs do when this game
/// lacks them is disclosed where it happens, on the technology's tooltip, by
/// `pack_dropped_note` and `packless_source_note`; a sentence on the dropdown
/// would be about a list that dropdown does not render.
pub(crate) const DROPDOWN_LADDER_LINE: &str =
    "\nAn entry your mods lack takes the mod's next name for it or is left out; two landing on one name are added, so an option can craft a shorter list than it shows.";

/// What happens to a text this library cannot use.
///
/// IT IS THE ONE THING THE SCREEN CANNOT SHOW. The text is set aside and the
/// field then decides exactly as it does while it holds the reserved word
/// (decision 2), so a player whose text went unused sees a settings screen that
/// still holds it and a game that ignores it. Saying so in the description is
/// the only warning available before the fact.
///
/// "AS THOUGH IT SAID DEFAULT" POINTS AT THE SWITCH LINE, and the
/// wording is chosen for where it lands on the screen. The line used to read
/// "that default applies instead", which is deictic, and its nearest antecedent
/// is the DEFAULT LINE at the head of the composition, which renders the
/// author's declared list and nothing else; beside a dropdown that is a
/// contradiction a player can read in one glance (measured: the tooltip said
/// one list, the recipe the game built was another). `text_switch_line`
/// composes the row immediately above this one and already says what the word
/// default does in THIS field: the option chosen above or below where there is
/// a dropdown, this mod's own list where there is not. Pointing at that row is
/// what makes this line true on every preset. The two are named rather than
/// counted, because the rows between them have moved once already.
///
/// IT PROMISES THE NARROW CLAIM AND NOT A LOAD, which is what the wording is
/// for. Setting a text aside is not the same as loading: the list that then
/// decides is held to every rule it always was, so a modpack in which it
/// cannot produce a legal result still stops the load, and on a refused load
/// the log ops never reach the host at all, so the load error is the only
/// place the reason can be: `Resolution::fallback_fact` is what puts it there,
/// which is what keeps this line's second clause true. Naming both places, the
/// log or the load error, is therefore the whole claim this line is allowed to
/// make.
pub(crate) const TEXT_FALLBACK_LINE: &str =
    "\nText this mod cannot use is set aside as though it said default; the reason is in the log or the load error.";

/// One preset's line: a newline, the value's own locale entry, and what it
/// means. The LABEL IS THE LOCALISED ONE, because that is what the settings
/// screen shows in the dropdown itself; naming the raw key here would tell the
/// player about a value they never see. That entry is REQUIRED by the locale
/// checker, so where it is missing the checker has already said so and
/// [`locale_ref`]'s raw fallback shows the player the internal value, which is
/// a thing they can type.
///
/// THE TAIL IS VALUES, NOT A STRING, because what a preset means is not always
/// text this library can spell: a research preset ends in the source
/// technology's own name key, which only the engine can render. An ingredient
/// preset hands over the one string it always was.
fn preset_element(full: &str, value: &str, tail: Vec<Value>) -> Value {
    let mut items = alloc::vec![
        Value::string(""),
        Value::string("\n"),
        locale_ref("string-mod-setting", &format!("{}-{}", full, value), value),
    ];
    items.extend(tail);
    Value::Arr(items)
}

/// What opens the second line of an INGREDIENT preset, and it is a line of its
/// own rather than a separator.
///
/// TWO VOCABULARIES ARE NOT ONE RUN OF TEXT. A preset's label is the
/// consumer's display prose ("Default: 4 iron plates, 2 gears") and the
/// rendering after it is the internal names the field beside it takes ("4
/// iron-plate, ..."). Joined by ": " they read as one sentence in two
/// languages, and only the second half can be copied into the field: measured
/// on 2.0.77, the first half pasted into the text setting is refused and
/// nothing in the tooltip said which half was which. The break and the words
/// `to type` put the copyable half on its own line, under the words a player
/// acts on, so the two vocabularies are two lines.
///
/// "TO TYPE" AND NOT "TYPE", WHICH IS WHAT THE WORD ALWAYS MEANT. Alone, `type`
/// reads first as the NOUN, and a label reading `type:` over a list of internal
/// names says that the list is a kind of something rather than that it is what
/// to put in the field. The instruction was the whole point of the line and the
/// second word is what makes it one.
///
/// A COST PRESET KEEPS ITS `": cost of "` (see [`cost_preset_tail`]) because it
/// has only one vocabulary: a localised label followed by a localised
/// technology name is prose throughout, and there is nothing in it to copy.
pub(crate) const INGREDIENT_PRESET_HEAD: &str = "\n  to type: ";

/// What a research preset means: the technology whose cost it copies, named
/// through the game's own key, which is as close to the name the player sees
/// everywhere else as this stage can get.
///
/// IT IS NOT A PROMISE OF THE DISPLAYED NAME. Where the game composes a name
/// rather than keying it the player reads the internal name, permanently: base
/// defines no `technology-name.logistics-2`, the ENGINE composes `Logistics 2`
/// at RUNTIME out of a name ending in a level number, and the data-stage
/// prototype carries no `localised_name` at all (measured on 2.0.77 build
/// 84539: null on `logistics-2` and on `logistics`, and 3 technologies of 275
/// carry the field, each holding another key table). Copying that field is the
/// repair the consumer's third assessment proposed for its finding 15, and it
/// is UNAVAILABLE rather than unpriced: this line is composed at the SETTINGS
/// stage, where `data.raw` is an empty table with `technology` nil, and a
/// setting prototype is not readable at the data stage and cannot be declared
/// there at all. Fix round 3's decision 7 in `agents/customizer-design.md` has
/// all four measurements.
///
/// The FIRST rung of the ladder, which is the source the author means; the
/// rest are what a modpack missing it falls back to, and a description that
/// listed them would be about this library rather than about the choice.
///
/// THE NAME KEY IS THE GAME'S, NOT THIS MOD'S, so the locale checker REQUIRES
/// nothing from this composition: `technology-name.<source>` belongs to
/// whoever ships that technology, and requiring it would be requiring the
/// consumer to squat in a namespace the same checker's collision scan exists
/// to police. It says so once as an ADVISORY instead; see
/// `Lib::composed_game_key_advisories`.
///
/// WHERE THE TECHNOLOGY OR ITS ENTRY IS MISSING, THE LINE DEGRADES TO THE RAW
/// INTERNAL NAME rather than to the marker or to nothing, because
/// [`locale_ref`] wraps it: the preset line reads `"\n<value>: cost of
/// <source>"` and the tooltip survives whole. Before the wrapper a source no
/// installed mod declared cost the consumer the entire tooltip, silently,
/// which is what the client probe measured.
///
/// AND `display` REPLACES THE WHOLE TAIL, both arms of it. An author who writes
/// one is saying what this option costs in their own words, which is the one
/// thing this stage cannot work out for itself: it sees `mods` and never
/// `data.raw`, so what it would otherwise name is the ladder's FIRST rung,
/// which is the truth about the declaration and can be false about the game. It
/// replaces "the fallback cost" as readily as it replaces a technology's name,
/// because a choice with no source at all is exactly one an author may want to
/// describe. See [`CostChoice::display`].
fn cost_preset_tail(c: &CostChoice) -> Vec<Value> {
    if !c.display.is_empty() {
        return alloc::vec![Value::Str(format!(": {}", c.display))];
    }
    match c.sources.first() {
        Some(name) => alloc::vec![
            Value::string(": cost of "),
            locale_ref("technology-name", name, name),
        ],
        None => alloc::vec![Value::string(": the fallback cost")],
    }
}

/// The engine's ceiling on one localised string's parameters, and it is PER
/// TABLE rather than per string.
///
/// MEASURED (Factorio 2.0.77, build 84539), twice independently: ONE TABLE
/// TAKES 20 PARAMETERS and the 21st refuses the load, `Too many parameters for
/// localised string: 21 > 20 (limit).`, with a literal and a table parameter
/// counting alike; nesting is capped at 20 LEVELS OF DEPTH and the 21st
/// refuses, `Too deep recursion for localised string: 21 > 20 (limit).`, where
/// the root table is level 1, every parameter sits one level below the table
/// holding it, a plain-string parameter occupies a level of its own and the key
/// at element 0 does not; and there is NO GLOBAL TABLE BUDGET at all, a
/// description holding 421 tables at depth 3 loading with exit 0. A recipe
/// prototype carries the same two ceilings with its own prototype kind in the
/// refusal text.
///
/// THE RULE THE VALUE DRIVES IS UNCHANGED. A dropdown with more presets than
/// fit NESTS rather than overflowing, and nesting spends depth, which is the
/// budget with 20 levels in it, so the overflow is a fill rather than a wall.
pub(crate) const MAX_LOCALISED_PARAMS: usize = 20;

/// Wraps parameters in a concatenating localised string, nesting when there are
/// more than the engine takes: a level that would need more keeps the first
/// nineteen and hands the rest to a nested group in the twentieth slot.
///
/// THE FILL POINT IS PER DROPDOWN KIND, because the presets are not the only
/// thing at the top level. Beside them sit the consumer's own description key
/// first and the switch line last, and on an INGREDIENT dropdown the ladder
/// line between them, so an ingredient dropdown stays flat up to SEVENTEEN presets
/// and a cost dropdown up to EIGHTEEN. Both are far above every dropdown
/// anyone has written; the nesting exists so that the one past the fill point
/// is a line in a tooltip rather than a load failure naming nothing useful.
/// `an_ingredient_description_nests_past_seventeen_presets` and
/// `a_cost_dropdown_description_nests_past_nineteen_presets` pin the two points.
pub(crate) fn localised_group(params: &[Value]) -> Value {
    let mut items = alloc::vec![Value::string("")];
    if params.len() <= MAX_LOCALISED_PARAMS {
        items.extend(params.iter().cloned());
        return Value::Arr(items);
    }
    items.extend(params[..MAX_LOCALISED_PARAMS - 1].iter().cloned());
    items.push(localised_group(&params[MAX_LOCALISED_PARAMS - 1..]));
    Value::Arr(items)
}

fn is_text(k: SettingKind) -> bool {
    matches!(k, SettingKind::Ingredients | SettingKind::Packs)
}

/// The World the round-trip check reads under: every name exists, and nothing
/// else is asked. See [`Lib::validate_text_settings`].
///
/// IT HAS TO ANSWER YES. The rendered default is checked at PLAN time, and at
/// the settings stage there is no data.raw to ask (measured: it is empty
/// there); the question is whether the LANGUAGE reads back what the renderer
/// wrote, not whether this particular modpack has the names. Whether the game
/// has one is the ladder's business at the data stage, where a missing name is
/// dropped rather than refused.
///
/// The other questions answer emptily rather than panicking. `parse` asks
/// about presence and nothing else, so they are unreachable; a panic in a
/// consumer's guest over an unreachable branch is a worse trade than an answer
/// nobody reads.
struct AllPresent;

impl Named for AllPresent {
    fn mod_name(&self) -> String {
        String::new()
    }
}

impl World for AllPresent {
    fn item_exists(&self, _name: &str) -> bool {
        true
    }
    fn fluid_exists(&self, _name: &str) -> bool {
        true
    }
    fn tool_exists(&self, _name: &str) -> bool {
        true
    }
    fn startup_setting(&self, _name: &str) -> Option<Value> {
        None
    }
    fn tech_names(&self) -> Vec<String> {
        Vec::new()
    }
    fn tech_prereqs(&self, _name: &str) -> Vec<String> {
        Vec::new()
    }
    fn tech_unit(&self, _name: &str) -> Option<Value> {
        None
    }
    fn tech_max_level(&self, _name: &str) -> Option<Value> {
        None
    }
    fn tech_has_research_trigger(&self, _name: &str) -> bool {
        false
    }
    fn tech_exists(&self, _name: &str) -> bool {
        false
    }
    fn entity_exists(&self, _name: &str) -> bool {
        false
    }
    fn recipe_exists(&self, _name: &str) -> bool {
        false
    }
}

fn default_value(s: &SettingDecl) -> Value {
    match s.kind {
        SettingKind::Bool => Value::Bool(s.def_bool),
        SettingKind::Dropdown => Value::Str(s.def_str.clone()),
        // THE WORD, never the rendered list. A rendered list as the default
        // would turn every player who never opened the settings screen into
        // an edited-text player the day the author changes it, because the
        // engine stores every setting's current value (measured: a fresh
        // install with no file gets one holding the default text).
        SettingKind::Ingredients | SettingKind::Packs => Value::string(DEFAULT),
        _ => Value::Num(s.def_num),
    }
}

fn setting_type_name(k: SettingKind) -> &'static str {
    match k {
        SettingKind::Bool => "bool-setting",
        SettingKind::Int => "int-setting",
        SettingKind::Double => "double-setting",
        SettingKind::Dropdown | SettingKind::Ingredients | SettingKind::Packs => "string-setting",
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
