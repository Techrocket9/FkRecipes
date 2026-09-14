use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ingredient_list::{IngredientList, ListEntry, ListKind, ListText, DEFAULT, MAX_TEXT};
use crate::op::Op;
use crate::plan::{
    Amount, CostChoice, Ingredient, IngredientChoice, Lib, Pack, SettingDecl, SettingKind, TechSpec,
};
use crate::value::{finite, kv, str_arr, Value, CRAFT_TIME_FLOOR, MAX_EXACT_INT};
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
            if s.kind == SettingKind::Dropdown {
                pairs.push(kv("allowed_values", str_arr(&s.values)));
                // THE PRESETS, WRITTEN OUT, on the dropdown that has a text
                // setting beside it. The engine cannot pre-fill that text from
                // the value they had (the settings stage sees no stored value,
                // measured), so the next best thing is showing them what each
                // preset means in the language they are about to type.
                if let Some(presets) = self.presets_beside_text(i + 1) {
                    pairs.push(kv(
                        "localised_description",
                        self.dropdown_description(&prefix, &full, &presets),
                    ));
                }
            }
            // A RESEARCH NUMBER STATES ITS RANGE, and beside a research
            // dropdown it states what 0 means: the settings screen shows a
            // numeric field with no visible bounds and no way to guess that.
            if numbers[i].bound {
                pairs.push(kv(
                    "localised_description",
                    number_description(&full, &self.research_range_line(i, s, numbers[i].dropdown)),
                ));
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
                    text_description(
                        &full,
                        &self.rendered_default(&prefix, s),
                        &self.text_switch_line(i),
                        s.kind == SettingKind::Ingredients,
                    ),
                ));
            }
            ops.push(Op::Extend(Value::Map(pairs)));
        }
        Ok(ops)
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
    pub(crate) fn validate_bindings(&self, prefix: &str) -> Result<(), String> {
        let at = "fkrecipes: ";

        // ONE DROPDOWN COMPOSES ONE DESCRIPTION. A setting carries a single
        // localised_description, so a second declaration's presets would
        // silently replace the first's; the recipes and the technologies count
        // separately, because each loop names what it walked. Counted in the
        // two walks below, where the pairing is proved, and refused after both
        // of them so the sentence names the first such setting in declaration
        // order rather than whichever walk noticed first.
        let mut armed_by_recipe = alloc::vec![0usize; self.settings.len()];
        let mut armed_by_tech = alloc::vec![0usize; self.settings.len()];

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
        }

        // The composed description is ONE declaration's presets, so two of
        // them reaching one dropdown is a settings screen showing a list that
        // belongs to the other declaration. The two counts are separate
        // because the sentence names what the author wrote; a dropdown armed
        // by one recipe AND one technology is not refused here, and the
        // technology's description is the one that lands, because
        // `setting_descriptions` walks recipes first.
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

    /// The presets a dropdown setting composes into its description, if a text
    /// setting sits beside it, decided by the same two walks the binding
    /// validator decides with: a declaration those step past has nothing
    /// composed for it here either.
    ///
    /// A DROPDOWN WITH NO TEXT SETTING BESIDE IT COMPOSES NOTHING, because
    /// there is nothing the presets have to be read in the language of and
    /// nothing to say the text overrides them.
    ///
    /// RECIPES THEN TECHNOLOGIES, in declaration order, and the LAST one wins.
    /// Two recipes cannot reach one dropdown, and neither can two
    /// technologies: both are refused at plan validation. What remains is a
    /// dropdown a recipe and a technology both put a text setting beside,
    /// which the two text settings' own "read by exactly one" rule does not
    /// forbid; the technology's presets are the ones composed.
    pub(crate) fn presets_beside_text(&self, index: usize) -> Option<Presets<'_>> {
        let mut found = None;
        for r in &self.recipes {
            let by = match &r.spec.ingredients_by {
                Some(by) => by,
                None => continue,
            };
            if !r.spec.ingredients.is_empty() || !self.valid_dropdown_setting(by.setting) {
                continue;
            }
            let text = match r.spec.ingredients_from {
                Some(h) if self.valid_ingredients_setting(h) => h.index,
                _ => continue,
            };
            if by.setting.index == index {
                found = Some(Presets::Ingredients(&by.choices, text));
            }
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
            if by.setting.index == index {
                found = Some(Presets::Cost(&by.choices, cc.packs.index));
            }
        }
        found
    }

    /// The ONE SPELLING of "this technology's cost dropdown has a preset list
    /// composed onto its description". Two readers need exactly this predicate
    /// and neither may spell it again: `presets_beside_text`, which is what
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
    pub(crate) fn text_switch_line(&self, i: usize) -> String {
        match self.text_switch_dropdown(i) {
            Some(d) => format!(
                "\nWhile this says default the option chosen {} applies; anything else applies instead of it.",
                self.relative_order(i, d)
            ),
            None => String::from("\nWhile this says default this mod's own list applies."),
        }
    }

    /// The dropdown setting that decides while the text setting at `i` says
    /// `default`, or `None` when the declaration that reads it has no dropdown.
    ///
    /// ONE WALK, TWO READERS: the composition on the text setting and the one
    /// on the dropdown itself have to agree about which pair they are
    /// describing, and a second walk spelling the same condition is how the two
    /// could describe different pairs.
    fn text_switch_dropdown(&self, i: usize) -> Option<usize> {
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
    fn research_range_line(&self, i: usize, s: &SettingDecl, dropdown: Option<usize>) -> String {
        let amount = self.installed_language().format_amount;
        let max = amount(s.spec.max.unwrap_or(0.0));
        match dropdown {
            Some(d) => format!(
                "\nA whole number from 0 to {}. While it is 0 the option chosen {} decides.",
                max,
                self.relative_order(i, d)
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
    fn dropdown_description(&self, prefix: &str, full: &str, presets: &Presets<'_>) -> Value {
        let mut params = alloc::vec![locale_ref("mod-setting-description", full, full)];
        let text = match *presets {
            Presets::Ingredients(choices, text) => {
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
                // THE WRAP, DISCLOSED WHERE THE WRAPPED TEXT IS. Every line
                // above this one is a typeable list, and a list is the only
                // thing in a tooltip a player copies. See [`LIST_WRAP_LINE`].
                params.push(Value::string(LIST_WRAP_LINE));
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

/// What a dropdown with a text setting beside it offers, and which setting that
/// is: a 1-BASED index, the shape every handle in this crate carries.
pub(crate) enum Presets<'a> {
    Ingredients(&'a [IngredientChoice], usize),
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
/// consumer's own entry, then the four things this library owes the player
/// about the field beside it.
///
/// SIX PARAMETERS, and none of them a table beyond the consumer's key and the
/// [`locale_ref`] wrapper around it, so the twenty-parameter ceiling
/// [`MAX_LOCALISED_PARAMS`] records is nowhere near reached and this shape
/// needs no nesting rule of its own.
///
/// THE FIVE LINES ARE THE ANSWER TO WHAT A CLIENT MEASUREMENT FOUND. A player
/// standing in the Mod Settings screen reads the tooltip whole (measured on
/// 2.0.77 on a DROPDOWN's composed description, one line per preset: seven
/// lines rendered readable and unclipped; the ceilings on a composed
/// description are the parameter count [`MAX_LOCALISED_PARAMS`] holds and the
/// nesting depth its comment records, neither of which is a line count), so
/// the description is where the library can say what the field takes; the
/// closed dropdown's LABEL beside it is truncated at about 37 characters, which
/// is why nothing a player needs may live in a label. The default line shows
/// the list the word `default` stands for, in the internal names the field
/// actually takes; the wrap line says that a list too long for the tooltip is
/// still one list, which is the one thing about that line a player cannot see;
/// the format line says so in words and states the ceiling; the
/// switch line says which of the two fields is deciding, which the screen
/// cannot show because it has no conditional visibility at all (measured); and
/// the fallback line says what a text this library cannot use costs, which
/// before it was stated nowhere a player looks.
///
/// ONE COMPOSITION, TWO READERS. The settings planner emits this;
/// [`Lib::check_locale`](crate::Lib) asks the same function for the same shape
/// with the list left out, so a line deleted here is a finding rather than a
/// silent loss. See `check_text_description`.
///
/// THE WRAP LINE SITS DIRECTLY UNDER THE DEFAULT LINE, because the default line
/// is the list it is about and the one a player copies. The format line below
/// it still says "as the default line above does", which two lines up is as
/// true as one.
///
/// `ingredients` SAYS WHICH OF THE TWO TEXT SETTINGS THIS IS, and the only
/// thing it decides is whether the format line names the word `none`: see
/// [`text_format_line`].
pub(crate) fn text_description(
    full: &str,
    rendered: &str,
    switch_line: &str,
    ingredients: bool,
) -> Value {
    Value::Arr(alloc::vec![
        Value::string(""),
        locale_ref("mod-setting-description", full, full),
        Value::Str(format!("\ndefault: {}", rendered)),
        Value::string(LIST_WRAP_LINE),
        Value::Str(text_format_line(ingredients)),
        Value::string(switch_line),
        Value::string(TEXT_FALLBACK_LINE),
    ])
}

/// The whole `localised_description` a RESEARCH NUMBER is emitted with: the
/// consumer's own entry, then the range the field takes.
///
/// IT IS THE ONLY PLACE THE RANGE IS STATED. The settings screen shows a
/// numeric field with no visible bounds, and 0 there means something the player
/// cannot guess: the dropdown beside it decides. Both sentences live in
/// `research_range_line`, and this is the shape they are emitted in.
pub(crate) fn number_description(full: &str, range_line: &str) -> Value {
    Value::Arr(alloc::vec![
        Value::string(""),
        locale_ref("mod-setting-description", full, full),
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
/// not by a second measurement: [`LIST_WRAP_LINE`] spends one parameter slot a
/// preset would otherwise have, and nothing else about the shape differs.
pub(crate) fn locale_ref(section: &str, key: &str, raw: &str) -> Value {
    Value::Arr(alloc::vec![
        Value::string("?"),
        Value::Arr(alloc::vec![Value::Str(format!("{}.{}", section, key))]),
        Value::string(raw),
    ])
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
/// IT NAMES THE DEFAULT LINE ABOVE IT rather than describing internal names in
/// the abstract, because that line is the copyable example, and copying it is
/// exactly what the composition is for.
pub(crate) fn text_format_line(ingredients: bool) -> String {
    let mut line = format!(
        "\nWrite internal names, as the default line above does, in at most {} characters.",
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

/// What the engine's own wrapping costs a player who copies a line, said where
/// the wrapped line is.
///
/// THE CONTINUATION STARTS AT THE LEFT MARGIN (measured on the client): a list
/// too long for the tooltip breaks, and the second half is not indented under
/// the first, so it reads as a line of its own and a player who copies what
/// looks like a whole line loses the last ingredient. The wrap is the engine's
/// and no composition can change it; what a composition can do is say that the
/// continuation belongs to the line above it.
///
/// IT SAYS "THE CONTINUATION" AND NOT "BOTH LINES", AND THAT IS ARITHMETIC AND
/// NOT STYLE. Two is not a bound on anything here. The wrap threshold is near
/// 57 to 60 characters (measured on the client), and the list this line
/// renders is the AUTHOR'S OWN DECLARED LIST, which has no bound short of the
/// language's 2000-character parse ceiling, so three and more visual lines are
/// reachable. On an ingredient dropdown the line above the list is the
/// consumer's LOCALISED LABEL, which can wrap on its own and is not a list at
/// all. A player who trusted "both lines are one list" over a list that
/// wrapped twice would copy two of three lines and drop the tail, which is the
/// exact failure this sentence exists to prevent, so the sentence names the
/// RELATIONSHIP (a continuation belongs to what it continues) instead of
/// counting lines it cannot count.
///
/// IT GOES WHEREVER A TYPEABLE LIST IS RENDERED AND NOWHERE ELSE: a text
/// setting's default line, and an ingredient dropdown's preset lines. A cost
/// dropdown's preset is a localised label followed by a localised technology
/// name, prose in one vocabulary with nothing in it to copy, so the sentence
/// there would be about a hazard that preset does not carry.
pub(crate) const LIST_WRAP_LINE: &str =
    "\nA list too long for one line continues on the next; the continuation is part of the same list.";

/// What happens to a text this library cannot use.
///
/// IT IS THE ONE THING THE SCREEN CANNOT SHOW. The text is set aside and the
/// field then decides exactly as it does while it holds the reserved word
/// (decision 2), so a player whose text went unused sees a settings screen that
/// still holds it and a game that ignores it. Saying so in the description is
/// the only warning available before the fact.
///
/// "BEHAVES AS THOUGH IT SAID DEFAULT" POINTS AT THE SWITCH LINE, and the
/// wording is chosen for where it lands on the screen. The line used to read
/// "that default applies instead", which is deictic, and its nearest antecedent
/// three rows above is the DEFAULT LINE, which renders the author's declared
/// list and nothing else; beside a dropdown that is a contradiction a player
/// can read in one glance (measured: the tooltip said one list, the recipe the
/// game built was another). `text_switch_line` composes the row immediately
/// above this one and already says what the word default does in THIS field:
/// the option chosen above or below where there is a dropdown, this mod's own
/// list where there is not. Pointing at that row is what makes this line true
/// on every preset.
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
    "\nA text this mod cannot use is set aside and the field behaves as though it said default; the reason is in the log, or in the load error if the load stops anyway.";

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
/// nothing in the tooltip said which half was which. The break and the word
/// `type` put the copyable half on its own line, under the word a player acts
/// on, so the two vocabularies are two lines.
///
/// A COST PRESET KEEPS ITS `": cost of "` (see [`cost_preset_tail`]) because it
/// has only one vocabulary: a localised label followed by a localised
/// technology name is prose throughout, and there is nothing in it to copy.
pub(crate) const INGREDIENT_PRESET_HEAD: &str = "\n  type: ";

/// What a research preset means: the technology whose cost it copies, named
/// the way the player sees it named everywhere else in the game.
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
fn cost_preset_tail(c: &CostChoice) -> Vec<Value> {
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
/// first and the switch line last, and on an INGREDIENT dropdown the wrap line
/// between them, so an ingredient dropdown stays flat up to SEVENTEEN presets
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
