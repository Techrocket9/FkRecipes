use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ingredient_list::{IngredientList, ListEntry, ListKind, ListText, DEFAULT};
use crate::op::Op;
use crate::plan::{
    custom_value, Amount, CostChoice, CostChoices, Ingredient, IngredientChoice, IngredientChoices,
    Lib, Pack, RecipeDecl, SettingDecl, SettingKind, TechDecl,
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
        // dropdown with a Custom arm has its whole preset list composed here.
        // Both read the recipes and technologies, so both need them well
        // formed. The two validators are shared with the data planner rather
        // than written twice.
        self.validate_bindings(&prefix)?;
        self.validate_text_settings(&prefix)?;

        let mut ops = Vec::with_capacity(self.settings.len());
        for (i, s) in self.settings.iter().enumerate() {
            // A legacy setting carries the name and the order the mod already
            // ships; everything else is prefixed and ordered by declaration.
            let order = if s.legacy {
                s.order.clone()
            } else {
                order_string(i)
            };
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
                // THE PRESETS, WRITTEN OUT, on the dropdown that offers the
                // player a text of their own. The engine cannot pre-fill that
                // text from the value they had (the settings stage sees no
                // stored value, measured), so the next best thing is showing
                // them what each preset means in the language they are about
                // to type.
                if let Some(arm) = self.custom_arm_for(&prefix, i + 1) {
                    pairs.push(kv(
                        "localised_description",
                        self.dropdown_description(&prefix, &full, &arm),
                    ));
                }
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
                    text_description(&full, &self.rendered_default(&prefix, s)),
                ));
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
    fn validate_settings(&self, prefix: &str, bound: &[bool]) -> Result<(), String> {
        let at = "fkrecipes: ";
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
        Ok(())
    }

    /// THE ONE WALK over a recipe's `IngredientsBy`, so the three readers of a
    /// Custom arm cannot drift: [`Lib::validate_bindings`] raises the `Err` as
    /// its refusal, [`Lib::custom_arm_for`] composes a description for exactly
    /// the arms this answers `Some` for, and the locale checker asks for a
    /// description for exactly those.
    ///
    /// `Ok(None)` is a recipe with no arm to compose from, INCLUDING one the
    /// binding walk steps past. A recipe naming `Ingredients` beside
    /// `IngredientsBy` is one of those: the data planner answers it with "pick
    /// one", so nothing validates its choices, and rendering an unvalidated
    /// choice would dereference an item handle nobody proved. That
    /// dereference is a panic in the consumer's settings stage, which is why
    /// this condition is written once instead of three times.
    fn ingredients_arm<'a>(
        &self,
        prefix: &str,
        r: &'a RecipeDecl,
    ) -> Result<Option<&'a IngredientChoices>, String> {
        let at = "fkrecipes: ";
        let by = match &r.spec.ingredients_by {
            Some(by) => by,
            None => return Ok(None),
        };
        // The dropdown's own exclusivity with Ingredients is the data
        // planner's sentence; nothing here may fire in front of it.
        if !r.spec.ingredients.is_empty() {
            return Ok(None);
        }
        if !self.valid_dropdown_setting(by.setting) {
            return Err(format!(
                "{}the recipe {} names an ingredients setting that this plan never declared",
                at, r.name
            ));
        }
        let setting = &self.settings[by.setting.index - 1];
        let cv = custom_value(&by.custom_value);
        let custom = match by.custom {
            Some(h) => h,
            None => {
                // A VALUE WITH NOTHING BEHIND IT is the pilot's own defect: the
                // player picks it and gets a recipe made of nothing, with no
                // line in the log saying why. A value the Choices DO cover is
                // an ordinary preset that happens to be spelled `custom`, and a
                // mod that already ships one keeps it: renaming it would reset
                // every player who had chosen it.
                if count_value(&setting.values, cv) > 0 && !by.choices.iter().any(|c| c.value == cv)
                {
                    return Err(format!(
                        "{}the setting {} offers {}, and the recipe {} names no Custom arm for it",
                        at,
                        setting.emitted_name(prefix),
                        cv,
                        r.name
                    ));
                }
                return Ok(None);
            }
        };
        if !self.valid_ingredients_setting(custom) {
            return Err(format!(
                "{}the recipe {} names a Custom ingredients setting that this plan never declared",
                at, r.name
            ));
        }
        Ok(Some(by))
    }

    /// The technology twin of [`Lib::ingredients_arm`], and the same contract:
    /// one condition, three readers.
    ///
    /// THE "EXACTLY ONE COST SOURCE" STEP-PAST STAYS WITH THE VALIDATOR rather
    /// than moving in here, and the difference is deliberate: that skip guards
    /// the CostFrom branch as well, and a cost description composes nothing but
    /// the choices' own strings, so a technology naming two cost sources has no
    /// handle here to dereference.
    fn cost_arm<'a>(
        &self,
        prefix: &str,
        t: &'a TechDecl,
    ) -> Result<Option<&'a CostChoices>, String> {
        let at = "fkrecipes: ";
        let by = match &t.spec.cost_by {
            Some(by) => by,
            None => return Ok(None),
        };
        if !self.valid_dropdown_setting(by.setting) {
            return Err(format!(
                "{}the technology {} names a cost setting that this plan never declared",
                at, t.name
            ));
        }
        let setting = &self.settings[by.setting.index - 1];
        let cv = custom_value(&by.custom_value);
        if by.custom.is_none() {
            if count_value(&setting.values, cv) > 0 && !by.choices.iter().any(|c| c.value == cv) {
                return Err(format!(
                    "{}the setting {} offers {}, and the technology {} names no Custom arm for it",
                    at,
                    setting.emitted_name(prefix),
                    cv,
                    t.name
                ));
            }
            return Ok(None);
        }
        Ok(Some(by))
    }

    /// Every rule about how a text setting is BOUND, and both planners run
    /// it: the settings stage composes a dropdown's description out of a
    /// Custom arm, so it needs the arm to be well formed just as much as the
    /// data stage does.
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
        // localised_description, so a second arm's presets would silently
        // replace the first's; the recipes and the technologies count
        // separately, because each loop names what it walked. Counted in the
        // two walks below, where an arm is proved, and refused after both of
        // them so the sentence names the first such setting in declaration
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
                if r.spec.ingredients_by.is_some() {
                    return Err(format!(
                        "{}{} names both IngredientsBy and IngredientsFrom; pick one",
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
            let by = match self.ingredients_arm(prefix, r)? {
                Some(by) => by,
                None => continue,
            };
            let setting = &self.settings[by.setting.index - 1];
            let full = setting.emitted_name(prefix);
            armed_by_recipe[by.setting.index - 1] += 1;
            let cv = custom_value(&by.custom_value);
            custom_arm_values(
                at,
                &who,
                &full,
                cv,
                by.choices.iter().any(|c| c.value == cv),
                &setting.values,
            )?;
            // The presets are RENDERED into the dropdown's description at the
            // settings stage, so they have to be renderable there. The data
            // planner checks the same thing in its own loop with the same
            // sentence; this is what brings the check forward to the stage that
            // needs it.
            for c in &by.choices {
                self.validate_ingredients(at, &who, Some(&r.spec.category), &c.ingredients)?;
            }
        }

        for t in &self.techs {
            let who = format!("the technology {}", t.name);
            let named = [
                !t.spec.cost_of.is_empty(),
                t.spec.unit.is_some(),
                t.spec.cost_by.is_some(),
                t.spec.cost_from.is_some(),
            ]
            .iter()
            .filter(|x| **x)
            .count();
            // Exactly one cost source is the data planner's sentence, and it
            // is the one an author reads best; everything below assumes it
            // held.
            if named != 1 {
                continue;
            }
            if let Some(cc) = &t.spec.cost_from {
                if !cc.position.is_empty() {
                    return Err(format!(
                        "{}{} names CostFrom with a Position; Position belongs to a Custom arm, and CostFrom is placed by After, Before and AfterTech",
                        at, who
                    ));
                }
                self.validate_custom_cost(at, &who, cc)?;
                continue;
            }
            let by = match self.cost_arm(prefix, t)? {
                Some(by) => by,
                None => continue,
            };
            let setting = &self.settings[by.setting.index - 1];
            let full = setting.emitted_name(prefix);
            armed_by_tech[by.setting.index - 1] += 1;
            // `cost_arm` answered Some, so the arm is there.
            let cc = by
                .custom
                .as_ref()
                .expect("a composed cost arm carries its CustomCost");
            let cv = custom_value(&by.custom_value);
            custom_arm_values(
                at,
                &who,
                &full,
                cv,
                by.choices.iter().any(|c| c.value == cv),
                &setting.values,
            )?;
            // THE PREREQUISITE MOVES WITH THE UNIT everywhere else in CostBy:
            // the chosen tier's source technology becomes the sole
            // prerequisite. A custom arm has no source, so it carries its own
            // ladder, and an arm with none would place the technology nowhere
            // at all.
            if cc.position.is_empty() {
                return Err(format!(
                    "{}{} names a Custom cost arm with no Position; the arm places the technology, so it needs a prerequisite ladder",
                    at, who
                ));
            }
            self.validate_custom_cost(at, &who, cc)?;
        }

        // The composed description is ONE declaration's presets, so two of
        // them reaching one dropdown is a settings screen showing a list that
        // belongs to the other declaration. The two counts are separate
        // because the sentence names what the author wrote; a dropdown armed
        // by one recipe AND one technology is not refused here, and the
        // technology's description is the one that lands, because
        // `custom_arm_for` walks recipes first.
        //
        // IT RUNS AFTER BOTH WALKS, so on a plan with two problems the arm's
        // own sentence wins: an ill formed arm is refused where it is walked,
        // and only a plan whose arms are all well formed reaches here.
        for (i, s) in self.settings.iter().enumerate() {
            if armed_by_recipe[i] > 1 {
                return Err(format!(
                    "{}the setting {} takes a Custom arm from more than one recipe; one dropdown composes one description",
                    at,
                    s.emitted_name(prefix)
                ));
            }
            if armed_by_tech[i] > 1 {
                return Err(format!(
                    "{}the setting {} takes a Custom arm from more than one technology; one dropdown composes one description",
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
            (lang.parse)(&rendered, kind, category, &who, &AllPresent)?;
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
            let name = match ing.candidates.first() {
                Some(first) => first.clone(),
                None => self.items[ing.item.index - 1].emitted_name(prefix),
            };
            entries.push(ListEntry {
                name,
                amount: ing.amount,
            });
        }
        IngredientList { entries }
    }

    /// The Custom arm a dropdown setting carries, if any, decided by the same
    /// two walks the binding validator decides with: a declaration those step
    /// past has no description composed for it here either.
    ///
    /// A REFUSAL IS AN ABSENCE HERE. `validate_bindings` runs first in both
    /// planners and raises the same `Err`, so an arm this reads as `Err` is one
    /// no caller ever reaches; the locale checker, which validates nothing,
    /// reports on locale rather than on a declaration mistake.
    ///
    /// RECIPES THEN TECHNOLOGIES, in declaration order, and the LAST one wins.
    /// Two recipes cannot reach one dropdown, and neither can two
    /// technologies: both are refused at plan validation. What remains is a
    /// dropdown a recipe and a technology both give an arm to, which the two
    /// text settings' own "read by exactly one" rule does not forbid; the
    /// technology's presets are the ones composed.
    pub(crate) fn custom_arm_for(&self, prefix: &str, index: usize) -> Option<CustomArm<'_>> {
        let mut arm = None;
        for r in &self.recipes {
            if let Ok(Some(by)) = self.ingredients_arm(prefix, r) {
                if by.setting.index == index {
                    arm = Some(CustomArm {
                        presets: Presets::Ingredients(&by.choices),
                    });
                }
            }
        }
        for t in &self.techs {
            if let Ok(Some(by)) = self.cost_arm(prefix, t) {
                if by.setting.index == index {
                    arm = Some(CustomArm {
                        presets: Presets::Cost(&by.choices),
                    });
                }
            }
        }
        arm
    }

    /// The dropdown's composed description: the consumer's own entry, then one
    /// line per preset, its LOCALISED label followed by what it means.
    fn dropdown_description(&self, prefix: &str, full: &str, arm: &CustomArm<'_>) -> Value {
        let mut params = alloc::vec![Value::Arr(alloc::vec![Value::Str(format!(
            "mod-setting-description.{}",
            full
        ))])];
        match arm.presets {
            Presets::Ingredients(choices) => {
                for c in choices {
                    let list = self.declared_list(prefix, &c.ingredients);
                    params.push(preset_element(
                        full,
                        &c.value,
                        &(self.installed_language().render)(&ListText::List(list)),
                    ));
                }
            }
            Presets::Cost(choices) => {
                for c in choices {
                    params.push(preset_element(full, &c.value, &cost_preset_text(c)));
                }
            }
        }
        localised_group(&params)
    }
}

/// What a dropdown's Custom arm offers beside the player's own text.
pub(crate) struct CustomArm<'a> {
    pub(crate) presets: Presets<'a>,
}

/// What the arm's dropdown offers besides the text.
pub(crate) enum Presets<'a> {
    Ingredients(&'a [IngredientChoice]),
    Cost(&'a [CostChoice]),
}

/// How many of a dropdown's values are this one.
fn count_value(values: &[String], value: &str) -> usize {
    values.iter().filter(|v| v.as_str() == value).count()
}

/// The shape rule a Custom arm's dropdown has to satisfy, written once because
/// the recipe arm and the cost arm have the same one.
fn custom_arm_values(
    at: &str,
    who: &str,
    setting: &str,
    value: &str,
    covered: bool,
    values: &[String],
) -> Result<(), String> {
    if covered {
        return Err(format!(
            "{}{} gives {} a preset as well as a Custom arm; name the arm's value with CustomValue",
            at, who, value
        ));
    }
    match count_value(values, value) {
        1 => Ok(()),
        0 => Err(format!(
            "{}{} names a Custom arm for {}, which the setting {} does not offer",
            at, who, value, setting
        )),
        _ => Err(format!(
            "{}the setting {} offers {} more than once, and a Custom arm needs it exactly once",
            at, setting, value
        )),
    }
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

/// The text setting's composed description: the consumer's own entry, then
/// the list the word `default` stands for.
fn text_description(full: &str, rendered: &str) -> Value {
    Value::Arr(alloc::vec![
        Value::string(""),
        Value::Arr(alloc::vec![Value::Str(format!(
            "mod-setting-description.{}",
            full
        ))]),
        Value::Str(format!("\ndefault: {}", rendered)),
    ])
}

/// One preset's line: a newline, the value's own locale entry, and what it
/// means. The LABEL IS THE LOCALISED ONE, because that is what the settings
/// screen shows in the dropdown itself; naming the raw key here would tell the
/// player about a value they never see.
fn preset_element(full: &str, value: &str, meaning: &str) -> Value {
    Value::Arr(alloc::vec![
        Value::string(""),
        Value::string("\n"),
        Value::Arr(alloc::vec![Value::Str(format!(
            "string-mod-setting.{}-{}",
            full, value
        ))]),
        Value::Str(format!(": {}", meaning)),
    ])
}

/// What a research preset means: the technology whose cost it copies.
///
/// The FIRST rung of the ladder, which is the source the author means; the
/// rest are what a modpack missing it falls back to, and a description that
/// listed them would be about this library rather than about the choice.
fn cost_preset_text(c: &CostChoice) -> String {
    match c.sources.first() {
        Some(name) => format!("cost of {}", name),
        None => String::from("the fallback cost"),
    }
}

/// The engine's ceiling on one localised string's parameters.
///
/// MEASURED (Factorio 2.0.77, build 84539): a localised string with 21
/// parameters refuses the load, and so does one nested 20 tables deep; 20
/// parameters and 19 nested tables load, and two nested groups of 20 load.
/// (The engine's refusal counts one higher than the tables, "21 > 20 (limit)"
/// for 20 of them; FkLua's data-stage probe pinned both limits.) A dropdown
/// with more presets than fit therefore NESTS rather than overflowing.
const MAX_LOCALISED_PARAMS: usize = 20;

/// Wraps parameters in a concatenating localised string, nesting when there are
/// more than the engine takes: a level that would need more keeps the first
/// nineteen and hands the rest to a nested group in the twentieth slot.
///
/// The top level's first parameter is the consumer's own description key, so a
/// dropdown with nineteen presets or fewer is flat, which is every dropdown
/// anyone has written; the nesting exists so that the twentieth is a line in a
/// tooltip rather than a load failure naming nothing useful.
fn localised_group(params: &[Value]) -> Value {
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
