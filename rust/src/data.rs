use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::ingredient_list::{IngredientList, Language, ListEntry, ListKind, ListText, DEFAULT};
use crate::op::{path_key, Op};
use crate::plan::{
    custom_value, Amount, CostChoice, CustomCost, Ingredient, IngredientChoice,
    IngredientsSettingRef, ItemDecl, Lib, Pack, RecipeDecl, SettingDecl, TechDecl, UnitSpec,
};
use crate::value::{
    finite, kv, localised, str_arr, Value, CRAFT_TIME_FLOOR, MAX_EXACT_INT, MAX_FLUID_AMOUNT,
    MAX_ITEM_AMOUNT,
};
use crate::world::World;

/// The custom-cost resolver's type, so the plan can hold one without spelling
/// seven parameters. See [`CUSTOM_COST`].
pub(crate) type CustomCostFn = fn(
    &Lib,
    &dyn World,
    &dyn World,
    &mut Resolution,
    &str,
    &TechDecl,
    &CustomCost,
) -> (Value, bool);

/// THE RESOLVER AS A POINTER, for the same reason the ingredient language is
/// one: a plan that declares no packs setting can never reach a custom cost,
/// and a direct call from the resolution pass would be a reference link-time
/// elimination has to keep. `Lib::packs_decl` installs this and nothing else
/// names it. See [`crate::ingredient_list::Language`].
pub(crate) const CUSTOM_COST: CustomCostFn = Lib::resolve_custom_cost;

impl Lib {
    /// Turns the declared items, recipes and technologies into an `Op`
    /// stream: validate first and refuse with the FIRST problem found, then
    /// resolve what the game actually has, then emit.
    ///
    /// The stream's order is fixed, because the two language halves are
    /// compared through it: every degradation log at the point the planner
    /// decided it, then the item prototypes in declaration order, then the
    /// recipes, then the technologies, then the prerequisite splices into
    /// other mods' technologies.
    ///
    /// This is the seam the emit layer stands on at the data stage; consumers
    /// call Emit and never this.
    pub fn plan_data(&self, w: &dyn World) -> Result<Vec<Op>, String> {
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

        self.validate(w, &prefix)?;
        let res = self.resolve(w, &prefix);
        // WHAT THE PLAYER WROTE, FIRST. Resolution asks the World questions
        // and mostly degrades; three answers it cannot degrade are a stored
        // value that is not one of a dropdown's values, a text setting holding
        // something that is not text, and an ingredient list the language
        // refuses. Each is carried out of the pass rather than raised inside
        // it, because resolution answers questions and this is where a plan is
        // refused; the FIRST one found wins, and it wins over every refusal
        // below because it is the earliest thing the pass met.
        if let Some(message) = &res.refusal {
            return Err(message.clone());
        }
        self.check_resolved_craft_times(&res)?;
        self.check_resolved_packs(&res)?;
        self.check_cycles(w, &res, &prefix)?;

        let mut ops = Vec::with_capacity(
            res.logs.len() + self.items.len() + self.recipes.len() + 2 * self.techs.len(),
        );
        for line in &res.logs {
            ops.push(Op::Log(line.clone()));
        }
        for it in &self.items {
            ops.push(Op::Extend(item_proto(&prefix, it)));
        }
        let unlocked = self.unlocked_recipes();
        for (i, r) in self.recipes.iter().enumerate() {
            ops.push(Op::Extend(recipe_proto(
                &prefix,
                self,
                r,
                &res.recipes[i],
                &res.craft_times[i],
                unlocked[i],
            )));
        }
        for (i, t) in self.techs.iter().enumerate() {
            ops.push(Op::Extend(tech_proto(&prefix, self, w, t, &res.techs[i])));
        }
        for rt in &res.techs {
            if rt.rewrite == 0 {
                continue;
            }
            let rw = &res.rewrites[rt.rewrite - 1];
            // A Set op's value is ALWAYS a real value, never Nil: the emit
            // layer hands it to fkdata::set, and a nil there DELETES the key
            // rather than writing one. Nothing plans a deletion today, and the
            // invariant is asserted in the pure half so it cannot start
            // silently.
            ops.push(Op::Set(
                vec![
                    path_key("technology"),
                    path_key(&rw.before),
                    path_key("prerequisites"),
                ],
                str_arr(&rw.list),
            ));
        }
        Ok(ops)
    }

    /// Returns the first refusal in a fixed scan order: items, then recipes,
    /// then technologies, each in declaration order. The cycle overlay is
    /// checked separately, after resolution, because it needs to know which
    /// splices actually survived.
    ///
    /// Every handle is checked here, not where it is read: an index that
    /// reaches resolve unchecked is a panic in somebody's data stage, and a
    /// handle from another plan is in range for this one.
    fn validate(&self, w: &dyn World, prefix: &str) -> Result<(), String> {
        let at = "fkrecipes: ";

        // THE BINDINGS FIRST, and the text settings after them, both shared
        // with the settings planner. They come before the three declaration
        // loops because a Custom arm that does not line up with its dropdown
        // would otherwise be answered by the ordinary allowed-values
        // comparison, which says "offers nothing for the value custom" and
        // points at the wrong thing. Neither of them asks the World anything.
        self.validate_bindings(prefix)?;
        self.validate_text_settings(prefix)?;

        for (i, it) in self.items.iter().enumerate() {
            if it.name.is_empty() {
                return Err(format!("{}an item was declared with an empty name", at));
            }
            // Compared on the EMITTED names, which is the namespace the
            // engine keeps: a legacy name and a generated one can arrive at
            // the same string from different declarations, and only one
            // survives.
            for other in self.items.iter().take(i) {
                if other.emitted_name(prefix) == it.emitted_name(prefix) {
                    return Err(format!(
                        "{}two items share the name {}; the second would overwrite the first",
                        at,
                        it.emitted_name(prefix)
                    ));
                }
            }
            // V1 never rewrites another mod's prototype except to splice a
            // prerequisite, and the cycle overlay counts on every planned
            // name being new: two nodes with one name is a walk that misses
            // the ring.
            if w.item_exists(&it.emitted_name(prefix)) {
                return Err(format!(
                    "{}the item {} already exists in data.raw; this plan would overwrite it",
                    at,
                    it.emitted_name(prefix)
                ));
            }
            if it.spec.stack_size < 0 {
                return Err(format!(
                    "{}the item {} has a negative stack size, which the engine refuses",
                    at, it.name
                ));
            }
            if it.spec.stack_size > MAX_EXACT_INT {
                return Err(format!(
                    "{}the item {} declares a stack size a Lua double cannot hold exactly: {}",
                    at, it.name, it.spec.stack_size
                ));
            }
            if it.spec.icon_size < 0 {
                return Err(format!(
                    "{}the item {} has a negative icon size, which the engine refuses",
                    at, it.name
                ));
            }
            if it.spec.icon_size > MAX_EXACT_INT {
                return Err(format!(
                    "{}the item {} declares an icon size a Lua double cannot hold exactly: {}",
                    at, it.name, it.spec.icon_size
                ));
            }
            if !it.spec.place_result.is_empty() && !w.entity_exists(&it.spec.place_result) {
                return Err(format!(
                    "{}the item {} names a place_result {} that does not exist",
                    at, it.name, it.spec.place_result
                ));
            }
            check_extra(
                at,
                &format!("the item {}", it.name),
                &it.spec.extra,
                ITEM_OWN_FIELDS,
            )?;
        }

        for (i, r) in self.recipes.iter().enumerate() {
            // The result handle first: a recipe with no result has no name to
            // report either, and "two recipes share the name" with an empty
            // name is a worse answer than the one that says what is actually
            // wrong.
            if r.result.index != 0 {
                // A handle was given: it has to be one of this plan's.
                if !self.valid_item(r.result) {
                    return Err(format!(
                        "{}a recipe was declared with no result item; Recipe needs an item this plan declared",
                        at
                    ));
                }
                if !r.spec.result_named.is_empty() {
                    return Err(format!(
                        "{}the recipe {} names both a result item and ResultNamed; pick one",
                        at, r.name
                    ));
                }
            } else if !r.spec.result_named.is_empty() {
                // THIS PLAN'S OWN ITEM FIRST, and the order is the whole
                // point. `result_named` is probed against the World, and the
                // World is data.raw as it stands BEFORE this plan runs, so an
                // item this plan declares two lines up is not there yet and
                // the probe would refuse it as absent. That refusal is correct
                // and its sentence is not: the consumer can see the item.
                // Compared on the EMITTED names, so it catches a legacy
                // declaration and a generated one alike.
                if self
                    .items
                    .iter()
                    .any(|it| it.emitted_name(prefix) == r.spec.result_named)
                {
                    return Err(format!(
                        "{}the recipe {} produces {} through ResultNamed, which this plan declares; use the item's handle instead",
                        at, r.name, r.spec.result_named
                    ));
                }
                // An existing item, so it is probed exactly as an ingredient
                // is.
                if !w.item_exists(&r.spec.result_named) {
                    return Err(format!(
                        "{}the recipe {} produces {}, which does not exist",
                        at, r.name, r.spec.result_named
                    ));
                }
                if r.name.is_empty() {
                    return Err(format!(
                        "{}a recipe producing an existing item was declared with no name; there is no declared item to take one from",
                        at
                    ));
                }
            } else {
                return Err(format!(
                    "{}a recipe was declared with no result item; Recipe needs an item this plan declared",
                    at
                ));
            }
            // A recipe with no name of its own inherits the item's, which is
            // the common shape and stays legal. This fires only when that name
            // is empty too, which today the item check above has already
            // caught: it is the guard that keeps the two checks independent.
            if r.name.is_empty() {
                return Err(format!("{}a recipe was declared with an empty name", at));
            }
            // Compared on the EMITTED names, which is the namespace the
            // engine keeps: a legacy name and a generated one can arrive at
            // the same string from different declarations, and only one
            // survives.
            for other in self.recipes.iter().take(i) {
                if other.emitted_name(prefix) == r.emitted_name(prefix) {
                    return Err(format!(
                        "{}two recipes share the name {}; the second would overwrite the first",
                        at,
                        r.emitted_name(prefix)
                    ));
                }
            }
            if !finite(r.spec.craft_time) {
                return Err(format!(
                    "{}the recipe {} declares a crafting time that is not a finite number",
                    at, r.name
                ));
            }
            // THE TWO CRAFTING-TIME FIELDS BEFORE THE EXTRA SWEEP, which is
            // the order the Go half scans in and so the order a plan with both
            // problems is answered in: a recipe that names CraftTime and
            // CraftTimeFrom together has not said what it costs to make, and
            // that is a larger mistake than a key the library would have
            // written itself. One order, one sentence, both languages.
            if r.spec.craft_time != 0.0 && r.spec.craft_time_from.index != 0 {
                return Err(format!(
                    "{}the recipe {} names both CraftTime and CraftTimeFrom; pick one",
                    at, r.name
                ));
            }
            check_extra(
                at,
                &format!("the recipe {}", r.name),
                &r.spec.extra,
                RECIPE_OWN_FIELDS,
            )?;
            // `enabled` is the ONE field of a recipe a consumer may write, and
            // only while nothing in this plan unlocks the recipe. A recipe
            // some technology unlocks is emitted disabled because the research
            // is what turns it on, and a second writer of that field would be
            // exactly the silent last-writer every other collision is refused
            // for. With no unlock in the plan the library has no opinion to
            // lose, and a mod migrating its recipe a commit before its
            // technology needs to say enabled = false by hand in the meantime.
            if r.spec.extra.iter().any(|(k, _)| k == "enabled") {
                if let Some(tech) = self.unlocker(i) {
                    return Err(format!(
                        "{}the recipe {} puts enabled in Extra, but the technology {} unlocks it, so the library owns that field",
                        at, r.name, tech
                    ));
                }
            }
            if !r.spec.ingredients.is_empty() && r.spec.ingredients_by.is_some() {
                return Err(format!(
                    "{}the recipe {} names both Ingredients and IngredientsBy; pick one",
                    at, r.name
                ));
            }
            if let Some(by) = &r.spec.ingredients_by {
                if !self.valid_dropdown_setting(by.setting) {
                    return Err(format!(
                        "{}the recipe {} names an ingredients setting that this plan never declared",
                        at, r.name
                    ));
                }
                let setting = &self.settings[by.setting.index - 1];
                // A CUSTOM ARM'S VALUE IS NOT A CHOICE, so it comes out of the
                // comparison: the arm is the plan for it, and validate_bindings
                // has already proved the dropdown offers it exactly once.
                let allowed =
                    presets_of(&setting.values, &by.custom, custom_value(&by.custom_value));
                let offered: Vec<String> = by.choices.iter().map(|c| c.value.clone()).collect();
                matches_allowed_values(
                    at,
                    &format!("the recipe {}", r.name),
                    &setting.emitted_name(prefix),
                    &offered,
                    &allowed,
                )?;
                for c in &by.choices {
                    self.validate_ingredients(
                        at,
                        &format!("the recipe {}", r.name),
                        Some(&r.spec.category),
                        &c.ingredients,
                    )?;
                }
            }
            if r.spec.craft_time_from.index != 0
                && !self.valid_double_setting(r.spec.craft_time_from)
            {
                return Err(format!(
                    "{}the recipe {} names a crafting-time setting that this plan never declared",
                    at, r.name
                ));
            }
            // A zero crafting time means the engine's own default and is
            // emitted as no field at all; a negative one is a refusal, not a
            // default.
            if r.spec.craft_time < 0.0 {
                return Err(format!(
                    "{}the recipe {} has a negative crafting time, which the engine refuses",
                    at, r.name
                ));
            }
            // Zero still means "say nothing and let the engine default
            // apply". A positive value below the floor is a load failure the
            // consumer would read as their own mod being broken, so it is
            // refused here by name.
            if r.spec.craft_time > 0.0 && r.spec.craft_time <= CRAFT_TIME_FLOOR {
                return Err(format!(
                    "{}the recipe {} declares a crafting time the engine refuses (energy_required can't be <= 0.001)",
                    at, r.name
                ));
            }
            if r.spec.result_count < 0 {
                return Err(format!(
                    "{}the recipe {} has a negative result count, which the engine refuses",
                    at, r.name
                ));
            }
            if r.spec.result_count > MAX_EXACT_INT {
                return Err(format!(
                    "{}the recipe {} declares a result count a Lua double cannot hold exactly: {}",
                    at, r.name, r.spec.result_count
                ));
            }
            if w.recipe_exists(&r.emitted_name(prefix)) {
                return Err(format!(
                    "{}the recipe {} already exists in data.raw; this plan would overwrite it",
                    at,
                    r.emitted_name(prefix)
                ));
            }
            self.validate_ingredients(
                at,
                &format!("the recipe {}", r.name),
                Some(&r.spec.category),
                &r.spec.ingredients,
            )?;
        }

        for (i, t) in self.techs.iter().enumerate() {
            if t.name.is_empty() {
                return Err(format!(
                    "{}a technology was declared with an empty name",
                    at
                ));
            }
            // Compared on the EMITTED names, which is the namespace the
            // engine keeps: a legacy name and a generated one can arrive at
            // the same string from different declarations, and only one
            // survives.
            for other in self.techs.iter().take(i) {
                if other.emitted_name(prefix) == t.emitted_name(prefix) {
                    return Err(format!(
                        "{}two technologies share the name {}; the second would overwrite the first",
                        at,
                        t.emitted_name(prefix)
                    ));
                }
            }
            if w.tech_exists(&t.emitted_name(prefix)) {
                return Err(format!(
                    "{}the technology {} already exists in data.raw; this plan would overwrite it",
                    at,
                    t.emitted_name(prefix)
                ));
            }
            let has_cost = !t.spec.cost_of.is_empty();
            let has_unit = t.spec.unit.is_some();
            let has_cost_by = t.spec.cost_by.is_some();
            let has_cost_from = t.spec.cost_from.is_some();
            let named = [has_cost, has_unit, has_cost_by, has_cost_from]
                .iter()
                .filter(|x| **x)
                .count();
            if named != 1 {
                return Err(format!(
                    "{}the technology {} must name exactly one of CostOf, Unit, CostBy or CostFrom",
                    at, t.name
                ));
            }
            // CostBy carries the prerequisite with the unit, so it is the
            // thing that places the technology. A second placement would be a
            // second opinion about the same edge.
            if has_cost_by
                && (!t.spec.after.is_empty()
                    || !t.spec.before.is_empty()
                    || t.spec.after_tech.index != 0)
            {
                return Err(format!(
                    "{}the technology {} names CostBy with a placement; the prerequisite moves with the unit, so CostBy places the technology itself",
                    at, t.name
                ));
            }
            if let Some(by) = &t.spec.cost_by {
                if !self.valid_dropdown_setting(by.setting) {
                    return Err(format!(
                        "{}the technology {} names a cost setting that this plan never declared",
                        at, t.name
                    ));
                }
                let setting = &self.settings[by.setting.index - 1];
                let allowed =
                    presets_of(&setting.values, &by.custom, custom_value(&by.custom_value));
                let offered: Vec<String> = by.choices.iter().map(|c| c.value.clone()).collect();
                matches_allowed_values(
                    at,
                    &format!("the technology {}", t.name),
                    &setting.emitted_name(prefix),
                    &offered,
                    &allowed,
                )?;
                // THE DECLARATION IS CHECKED HERE, THE WORLD IS NOT. A
                // fallback count of zero is wrong however the ladder turns
                // out, so it is refused whether or not the fallback is
                // reached; which science packs the GAME has is asked only
                // when the fallback is actually used, because the pilot
                // measured what the other order costs: a fallback priced in a
                // pack a modpack renamed refused a load that would never have
                // reached the fallback at all.
                self.validate_unit(at, &t.name, &by.fallback)?;
            }
            if !t.spec.after.is_empty() && t.spec.after_tech.index != 0 {
                return Err(format!(
                    "{}the technology {} names both After and AfterTech; pick one anchor",
                    at, t.name
                ));
            }
            if !t.spec.before.is_empty() && t.spec.after_tech.index != 0 {
                return Err(format!(
                    "{}the technology {} names Before with AfterTech; InsertBetween splices around a technology that already exists",
                    at, t.name
                ));
            }
            if !t.spec.before.is_empty() && t.spec.after.is_empty() {
                return Err(format!(
                    "{}the technology {} names Before without After; InsertBetween needs both ends",
                    at, t.name
                ));
            }
            if t.spec.after_tech.index != 0 && !self.valid_tech(t.spec.after_tech) {
                return Err(format!(
                    "{}the technology {} names an AfterTech technology that this plan never declared",
                    at, t.name
                ));
            }
            if t.spec.icon_size < 0 {
                return Err(format!(
                    "{}the technology {} has a negative icon size, which the engine refuses",
                    at, t.name
                ));
            }
            if t.spec.icon_size > MAX_EXACT_INT {
                return Err(format!(
                    "{}the technology {} declares an icon size a Lua double cannot hold exactly: {}",
                    at, t.name, t.spec.icon_size
                ));
            }
            check_extra(
                at,
                &format!("the technology {}", t.name),
                &t.spec.extra,
                TECH_OWN_FIELDS,
            )?;
            match &t.spec.unit {
                Some(unit) => {
                    self.validate_unit(at, &t.name, unit)?;
                }
                // A CostBy technology reaches here with neither field set,
                // and has nothing named to check: its ladder is walked at
                // resolution, where a source that cannot be used is stepped
                // past rather than refused. A CostFrom one names no source at
                // all: its whole unit comes from the player's settings.
                None if has_cost_by || has_cost_from => {}
                None => {
                    if !w.tech_exists(&t.spec.cost_of) {
                        return Err(format!(
                            "{}CostOf({}): no technology of that name exists",
                            at, t.spec.cost_of
                        ));
                    }
                    if w.tech_has_research_trigger(&t.spec.cost_of) {
                        return Err(format!(
                            "{}CostOf({}): {} is a research_trigger technology with no unit to copy; name a unit-carrying technology instead",
                            at, t.spec.cost_of, t.spec.cost_of
                        ));
                    }
                    match w.tech_unit(&t.spec.cost_of) {
                        None => {
                            return Err(format!(
                                "{}CostOf({}): {} carries no unit to copy",
                                at, t.spec.cost_of, t.spec.cost_of
                            ))
                        }
                        // The unit is copied verbatim into a prototype, so
                        // it has to BE a prototype's field map. Anything else
                        // is another mod's mistake arriving as this mod's
                        // load failure. "Dictionary", not "table": a Lua
                        // sequence is a table as well, and an array-shaped
                        // unit is exactly one of the things this refuses.
                        Some(Value::Map(pairs)) => {
                            if holds_dropped_subtree(&Value::Map(pairs)) {
                                return Err(format!(
                                    "{}CostOf({}): the unit of {} holds a table this library cannot copy faithfully",
                                    at, t.spec.cost_of, t.spec.cost_of
                                ));
                            }
                            // The level cap rides along the same way and
                            // truncates the same way: a map-shaped max_level
                            // that lost a subtree would be emitted with a hole
                            // in it.
                            if let Some(level) = w.tech_max_level(&t.spec.cost_of) {
                                if holds_dropped_subtree(&level) {
                                    return Err(format!(
                                        "{}CostOf({}): the max_level of {} holds a table this library cannot copy faithfully",
                                        at, t.spec.cost_of, t.spec.cost_of
                                    ));
                                }
                            }
                        }
                        Some(_) => {
                            return Err(format!(
                                "{}CostOf({}): {} has a unit that is not a dictionary",
                                at, t.spec.cost_of, t.spec.cost_of
                            ))
                        }
                    }
                }
            }
            for u in &t.spec.unlocks {
                if !self.valid_recipe(*u) {
                    return Err(format!(
                        "{}the technology {} unlocks a recipe that this plan never declared",
                        at, t.name
                    ));
                }
            }
            if t.spec.enabled_by.index != 0 && !self.valid_bool_setting(t.spec.enabled_by) {
                return Err(format!(
                    "{}the technology {} names an EnabledBy setting that this plan never declared",
                    at, t.name
                ));
            }
        }
        Ok(())
    }

    /// Asks the World everything the plan needs to know and records what
    /// degraded. The pass order IS the log order, and it is part of the
    /// contract the Go mirror holds to: recipes in declaration order, then
    /// technologies in declaration order, and within a technology its
    /// enablement before its tree placement.
    fn resolve(&self, w: &dyn World, prefix: &str) -> Resolution {
        let mut res = Resolution::default();
        // THE PLAN'S OWN ITEMS, BEFORE ANY TEXT IS PARSED. See [`PlanItems`]:
        // the names this plan is about to emit are the ones its own setting
        // descriptions show the player, so the language has to know them.
        let own = self.plan_items(w, prefix);

        for r in &self.recipes {
            // The crafting time first, then the ingredients: a recipe's own
            // field before what it is made of, mirroring a technology's
            // enablement before its tree placement.
            let mut ct = CraftTime::default();
            if r.spec.craft_time_from.index != 0 {
                let setting = &self.settings[r.spec.craft_time_from.index - 1];
                let full = setting.emitted_name(prefix);
                ct.bound = true;
                ct.value = setting.def_num;
                match w.startup_setting(&full) {
                    Some(Value::Num(n)) => ct.value = n,
                    _ => res.logs.push(format!(
                        "fkrecipes: the setting {} was not readable, so its default applies",
                        full
                    )),
                }
                ct.setting = full;
            }
            res.craft_times.push(ct);

            match &r.spec.ingredients_by {
                Some(by) => {
                    let setting = &self.settings[by.setting.index - 1];
                    let chosen = res.read_dropdown(w, setting, prefix);
                    let cv = custom_value(&by.custom_value);
                    if let Some(h) = by.custom {
                        if chosen == cv {
                            let list =
                                self.resolve_text_ingredients(w, &own, &mut res, prefix, r, h);
                            res.recipes.push(list);
                            continue;
                        }
                        // THE PLAYER IS TOLD THEIR TEXT IS BEING IGNORED. A
                        // list typed into the field while the dropdown sits on
                        // a preset is a preference nothing reads, and silence
                        // there is the report "my ingredients did nothing".
                        note_ignored_text(
                            self.installed_language(),
                            &own,
                            &mut res,
                            &self.settings[h.index - 1].emitted_name(prefix),
                            ListKind::Recipe,
                            &r.spec.category,
                            &setting.emitted_name(prefix),
                            cv,
                        );
                    }
                    let declared = choice_for(&by.choices, &chosen);
                    let mut list = self.resolve_ingredients(w, &mut res, prefix, &r.name, declared);
                    // A plan that named things and got none of them is a
                    // recipe made of nothing. The DEFAULT option is what
                    // applies then, because it is the one the mod ships as
                    // its own answer.
                    if !declared.is_empty() && list.is_empty() && chosen != setting.def_str {
                        res.logs.push(format!(
                            "fkrecipes: {}: the {} ingredients name nothing this game has, so the {} ingredients apply",
                            r.name, chosen, setting.def_str
                        ));
                        list = self.resolve_ingredients(
                            w,
                            &mut res,
                            prefix,
                            &r.name,
                            choice_for(&by.choices, &setting.def_str),
                        );
                    }
                    res.recipes.push(list);
                }
                None => match r.spec.ingredients_from {
                    Some(h) => {
                        let list = self.resolve_text_ingredients(w, &own, &mut res, prefix, r, h);
                        res.recipes.push(list);
                    }
                    None => {
                        let list = self.resolve_ingredients(
                            w,
                            &mut res,
                            prefix,
                            &r.name,
                            &r.spec.ingredients,
                        );
                        res.recipes.push(list);
                    }
                },
            }
        }

        for t in &self.techs {
            let mut rt = ResolvedTech::default();

            if t.spec.enabled_by.index != 0 {
                let s = &self.settings[t.spec.enabled_by.index - 1];
                let full = s.emitted_name(prefix);
                rt.has_enabled_by = true;
                rt.on = s.def_bool;
                match w.startup_setting(&full) {
                    Some(Value::Bool(b)) => rt.on = b,
                    _ => res.logs.push(format!(
                        "fkrecipes: the setting {} was not readable, so its default applies",
                        full
                    )),
                }
            }

            // A HAND-ROLLED UNIT'S PACKS, before the tree placement, in the
            // same order a recipe's ingredients come after its crafting time:
            // what the technology COSTS is its own field, and where it sits
            // is the world's answer.
            if let Some(unit) = &t.spec.unit {
                rt.packs = resolve_packs(w, &mut res, &t.name, &unit.packs);
                rt.no_packs = rt.packs.is_empty();
            }

            // THE PLAYER'S OWN UNIT, in the same place a hand-rolled one is
            // resolved: what the technology COSTS before where it sits.
            if let Some(cc) = &t.spec.cost_from {
                let (unit, no_packs) =
                    (self.installed_custom_cost())(self, w, &own, &mut res, prefix, t, cc);
                rt.unit = Some(unit);
                rt.no_packs = no_packs;
            }

            if let Some(by) = &t.spec.cost_by {
                let setting = &self.settings[by.setting.index - 1];
                let chosen = res.read_dropdown(w, setting, prefix);
                let cv = custom_value(&by.custom_value);
                if let Some(cc) = &by.custom {
                    if chosen == cv {
                        let (unit, no_packs) =
                            (self.installed_custom_cost())(self, w, &own, &mut res, prefix, t, cc);
                        rt.unit = Some(unit);
                        rt.no_packs = no_packs;
                        // THE PREREQUISITE STILL MOVES WITH THE UNIT, and the
                        // unit is the player's, so the ladder the author wrote
                        // is what says where the technology hangs. No rung
                        // present is the same answer a ladder that finds
                        // nothing gives anywhere else: say so, and place the
                        // technology nowhere.
                        rt.prereqs = custom_prereqs(w, &mut res, &t.name, &cc.position);
                        res.techs.push(rt);
                        continue;
                    }
                    // ONE LINE PER EDITED SETTING, and the numbers are as
                    // invisible as the text: a count or a seconds moved while
                    // the dropdown sits on a preset is a preference nothing
                    // reads either, and silence there is the same field report
                    // the text line exists to answer. They come in the order
                    // the unit carries them and the order the custom-cost log
                    // line names them: count, seconds, packs.
                    let dropdown = setting.emitted_name(prefix);
                    note_ignored_number(
                        w,
                        &mut res,
                        &self.settings[cc.count.index - 1],
                        prefix,
                        &dropdown,
                        cv,
                    );
                    note_ignored_number(
                        w,
                        &mut res,
                        &self.settings[cc.seconds.index - 1],
                        prefix,
                        &dropdown,
                        cv,
                    );
                    note_ignored_text(
                        self.installed_language(),
                        &own,
                        &mut res,
                        &self.settings[cc.packs.index - 1].emitted_name(prefix),
                        ListKind::Packs,
                        "",
                        &dropdown,
                        cv,
                    );
                }
                let mut source = String::new();
                for name in sources_for(&by.choices, &chosen) {
                    if w.tech_has_research_trigger(name) {
                        continue;
                    }
                    // NO PRESENCE PROBE HERE, and that is deliberate rather
                    // than an omission: a technology the game does not have
                    // carries no unit either, so this arm steps past an absent
                    // rung and a unit-less one by the same test. A tech_exists
                    // call in front of it would be a branch no test could make
                    // load-bearing, which is a liability wearing the costume
                    // of a defence.
                    //
                    // ONE FALL-THROUGH ARM, TWO CASES, EACH WITH ITS OWN
                    // WITNESS: `None` is an absent or unit-less technology,
                    // `Some(non-map)` is one whose unit this library cannot
                    // copy faithfully (not a dictionary, or holding a subtree
                    // dropped on the way in). The Go mirror splits the same
                    // job across two terms, because its TechUnit returns a
                    // (Value, bool) pair that can disagree with itself and so
                    // needs the flag honoured over the value; `Option<Value>`
                    // makes that disagreement unrepresentable here, which is
                    // why that half has a Go-only test.
                    let u = match w.tech_unit(name) {
                        Some(u) if matches!(u, Value::Map(_)) && !holds_dropped_subtree(&u) => u,
                        _ => continue,
                    };
                    rt.unit = Some(u);
                    if let Some(level) = w.tech_max_level(name) {
                        if !matches!(level, Value::Nil) && !holds_dropped_subtree(&level) {
                            rt.max_level = Some(level);
                        }
                    }
                    source = name.clone();
                    break;
                }
                if source.is_empty() {
                    res.logs.push(format!(
                        "fkrecipes: {}: no source for the {} cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
                        t.name, chosen
                    ));
                    // THE FALLBACK IS RESOLVED ONLY HERE, which is the whole
                    // point of doing it at resolution: a fallback nobody
                    // reaches asks the game nothing and so can refuse
                    // nothing.
                    let packs = resolve_packs(w, &mut res, &t.name, &by.fallback.packs);
                    rt.no_packs = packs.is_empty();
                    rt.unit = Some(unit_value(by.fallback.count, by.fallback.seconds, &packs));
                } else {
                    // THE PREREQUISITE MOVES WITH THE UNIT.
                    rt.prereqs = vec![source];
                }
                res.techs.push(rt);
                continue;
            }

            let after = t.spec.after.as_str();
            let before = t.spec.before.as_str();
            let new_name = t.emitted_name(prefix);
            if t.spec.after_tech.index != 0 {
                // An anchor this plan declares itself needs no presence
                // probe and cannot degrade: the prototype is emitted by the
                // same plan.
                //
                // THE OVERLAY IS WHAT CATCHES A RING, not the argument below.
                // This edge joins it like any other prerequisite and must
                // never be left out of it on the argument's strength. The
                // argument is defence in depth and it only covers handles
                // THIS plan issued: such a handle exists only after the
                // technology it names, so those edges run backwards through
                // declaration order, and a technology anchored this way takes
                // no Before, so nothing in the game's tree is rewritten to
                // require it. A handle from anywhere else can point forwards
                // or at itself, and the walk is what answers that.
                rt.prereqs = vec![self.techs[t.spec.after_tech.index - 1].emitted_name(prefix)];
            } else if after.is_empty() {
                // Before without After was refused in validate; nothing to place.
            } else if before.is_empty() {
                if w.tech_exists(after) {
                    rt.prereqs = vec![String::from(after)];
                } else {
                    res.logs.push(drop_line(&t.name, after));
                }
            } else if !w.tech_exists(before) {
                res.logs.push(format!(
                    "fkrecipes: {}: {} is absent, so InsertBetween degrades to After({})",
                    t.name, before, after
                ));
                if w.tech_exists(after) {
                    rt.prereqs = vec![String::from(after)];
                } else {
                    res.logs.push(drop_line(&t.name, after));
                }
            } else if !w.tech_exists(after) {
                // The anchor is gone, so there is nothing to splice between
                // and nothing to replace in the other technology's list. Emit
                // the technology unattached rather than guess a substitute.
                res.logs.push(drop_line(&t.name, after));
            } else {
                rt.prereqs = vec![String::from(after)];
                let base = res.current_prereqs(w, before);
                let mut list = Vec::with_capacity(base.len() + 1);
                let mut replaced = false;
                for p in &base {
                    if p.as_str() == after {
                        list.push(new_name.clone());
                        replaced = true;
                        continue;
                    }
                    list.push(p.clone());
                }
                if !replaced {
                    list.push(new_name.clone());
                    res.logs.push(format!(
                        "fkrecipes: {}: {} does not require {}, so the new technology is appended to its prerequisites",
                        t.name, before, after
                    ));
                }
                res.rewrites.push(RewriteRec {
                    before: String::from(before),
                    list,
                });
                rt.rewrite = res.rewrites.len();
            }

            res.techs.push(rt);
        }
        res
    }

    /// Refuses a bound crafting time the engine would not take. It runs after
    /// resolution because the value is a fact about what the World answered,
    /// not about what the plan declared.
    ///
    /// The setting is generated with a minimum above the floor, so the
    /// ordinary way to reach this is another mod: setting names are a global
    /// namespace and the engine keeps the last declaration of a same-type
    /// name, silently. A refusal naming the setting beats the engine's load
    /// failure blaming the consumer.
    fn check_resolved_craft_times(&self, res: &Resolution) -> Result<(), String> {
        for (i, ct) in res.craft_times.iter().enumerate() {
            if !ct.bound {
                continue;
            }
            // Finiteness FIRST, and not only for the message: an infinity is
            // above the floor, so the floor arm would wave it through and ship
            // a recipe that never completes. It is one of the THREE floats
            // that arrive from outside and so never crossed the declaration
            // checks; a research count and a research time are the other two,
            // and `resolve_custom_cost` asks them the same question.
            if !finite(ct.value) {
                return Err(format!(
                    "fkrecipes: the recipe {} reads its crafting time from {}, which answers a value that is not a finite number",
                    self.recipes[i].name, ct.setting
                ));
            }
            if ct.value > CRAFT_TIME_FLOOR {
                continue;
            }
            return Err(format!(
                "fkrecipes: the recipe {} reads its crafting time from {}, which answers at or below the engine floor (energy_required can't be <= 0.001)",
                self.recipes[i].name, ct.setting
            ));
        }
        Ok(())
    }

    /// A unit with no science pack left is refused, in declaration order.
    ///
    /// THIS IS THE ONE PLACE A DROP BECOMES A REFUSAL. An ingredient that
    /// drops leaves a cheaper recipe, which is a game somebody can still
    /// play; a research unit with no packs at all is a technology the player
    /// cannot pay for, and the engine takes it (measured only as far as the
    /// load, so this library does not find out what it does in a game). The
    /// author is told by name instead.
    fn check_resolved_packs(&self, res: &Resolution) -> Result<(), String> {
        for (i, rt) in res.techs.iter().enumerate() {
            if !rt.no_packs {
                continue;
            }
            return Err(format!(
                "fkrecipes: the technology {} has no science pack the game has; research takes at least one",
                self.techs[i].name
            ));
        }
        Ok(())
    }

    /// The first technology in declaration order that unlocks this recipe, by
    /// its declared name, or `None` when nothing in the plan does.
    ///
    /// An out-of-range unlock handle is STEPPED PAST rather than followed:
    /// this runs during the recipe loop, which is before the technology loop
    /// that refuses such a handle by name, and answering it here would replace
    /// that sentence with one about a field the consumer did not get wrong.
    fn unlocker(&self, recipe: usize) -> Option<&str> {
        self.techs
            .iter()
            .find(|t| {
                t.spec
                    .unlocks
                    .iter()
                    .any(|u| self.valid_recipe(*u) && u.index - 1 == recipe)
            })
            .map(|t| t.name.as_str())
    }

    /// Marks the recipes some technology unlocks. Those are emitted disabled,
    /// because the research is what turns them on.
    fn unlocked_recipes(&self) -> Vec<bool> {
        let mut marks = vec![false; self.recipes.len()];
        for t in &self.techs {
            for u in &t.spec.unlocks {
                marks[u.index - 1] = true;
            }
        }
        marks
    }
}

pub(crate) struct ResolvedIngredient {
    pub(crate) name: String,
    /// Carries the KIND as well as the number: an ingredient's `type` field is
    /// decided here and nowhere else, so a fluid cannot reach the prototype
    /// as an item by being read through the wrong branch downstream.
    pub(crate) amount: Amount,
}

/// One planned rewrite of another technology's prerequisite list. A second
/// splice into the same technology builds on the first: two Set ops on one
/// path would otherwise mean the last one silently undoes the earlier splice.
pub(crate) struct RewriteRec {
    pub(crate) before: String,
    pub(crate) list: Vec<String>,
}

#[derive(Default)]
pub(crate) struct ResolvedTech {
    pub(crate) prereqs: Vec<String>,
    /// The cost a CostBy ladder settled on, and the level cap that rode
    /// along with it. Both are None for every other cost shape.
    pub(crate) unit: Option<Value>,
    pub(crate) max_level: Option<Value>,
    pub(crate) has_enabled_by: bool,
    pub(crate) on: bool,
    /// A 1-based index into `Resolution::rewrites`; zero is none.
    pub(crate) rewrite: usize,
    /// The science packs a hand-rolled `Unit` resolved to, drops removed. A
    /// CostBy technology keeps its whole unit in `unit` instead, fallback
    /// included.
    pub(crate) packs: Vec<ResolvedPack>,
    /// Set when a unit this plan rolled ITSELF (a declared `Unit`, or a
    /// `CostChoices` fallback that was actually reached) ended up with no
    /// science pack the game has. Carried rather than refused on the spot,
    /// because resolution answers questions and `plan_data` is where a plan
    /// is refused.
    pub(crate) no_packs: bool,
}

/// One science pack a ladder settled on.
pub(crate) struct ResolvedPack {
    pub(crate) name: String,
    pub(crate) amount: i64,
}

/// What a bound recipe's energy_required resolved to, and which setting
/// answered, so a refusal can name it.
#[derive(Default)]
pub(crate) struct CraftTime {
    pub(crate) bound: bool,
    pub(crate) setting: String,
    pub(crate) value: f64,
}

#[derive(Default)]
pub(crate) struct Resolution {
    pub(crate) logs: Vec<String>,
    pub(crate) craft_times: Vec<CraftTime>,
    pub(crate) recipes: Vec<Vec<ResolvedIngredient>>,
    pub(crate) techs: Vec<ResolvedTech>,
    pub(crate) rewrites: Vec<RewriteRec>,
    /// The FIRST answer resolution could not degrade. Carried rather than
    /// returned so the pass stays one shape: it keeps resolving, with an empty
    /// list where the refused answer would have gone, and `plan_data` raises
    /// this before it reads any of it.
    pub(crate) refusal: Option<String>,
}

impl Resolution {
    /// Records a refusal, keeping the first: resolution runs in declaration
    /// order, so the first is the one a reader would have met.
    fn refuse(&mut self, message: String) {
        if self.refusal.is_none() {
            self.refusal = Some(message);
        }
    }
}

impl Resolution {
    /// The prerequisite list a splice should build on: the one an earlier
    /// splice in this same plan already planned, or the game's own.
    pub(crate) fn current_prereqs(&self, w: &dyn World, tech: &str) -> Vec<String> {
        for rw in self.rewrites.iter().rev() {
            if rw.before.as_str() == tech {
                return rw.list.clone();
            }
        }
        w.tech_prereqs(tech)
    }
}

/// Says out loud that a text nothing is reading was edited.
///
/// EDITED MEANS "THE LANGUAGE DOES NOT READ IT AS THE WORD", decided by the
/// same parse the data path decides with: a text this planner would have taken
/// as the mod's own list is not an edit, tolerated trailing comma and all, and
/// a second trimmer here would be a second answer to the same question. A text
/// that does not parse at all IS an edit and is still only a line: the dropdown
/// sits on a preset, so nothing was going to read the list, and refusing a load
/// over it would be the worse answer.
///
/// `own` is the overlay World the language reads a typed list under; its
/// `startup_setting` is the game's own, so the value read here is the value the
/// data path would read. The KIND and the CATEGORY come from the call site
/// rather than from the setting, because each caller knows both: a recipe's
/// dropdown hands over an ingredient list in that recipe's category, and a
/// technology's hands over science packs, which have no category at all.
// EIGHT PARAMETERS. Seven are facts only the call site has: which list the
// dropdown hands over, in which recipe's category, under which value. The
// first is the language table, which this function reaches the way everything
// outside its module does.
#[allow(clippy::too_many_arguments)]
fn note_ignored_text(
    lang: &Language,
    own: &dyn World,
    res: &mut Resolution,
    full: &str,
    kind: ListKind,
    category: &str,
    dropdown: &str,
    cv: &str,
) {
    // Both string shapes are looked at, for the reason `read_text_setting`
    // gives: what counts as an edit is the parser's answer over the same bytes
    // the data path would have read, and a stored value whose bytes are not
    // text is an edit like any other rather than something to pass over in
    // silence.
    let stored = match own.startup_setting(full) {
        Some(Value::Str(text)) => text.into_bytes(),
        Some(Value::Bytes(b)) => b,
        _ => return,
    };
    if (lang.is_edited)(&stored, kind, category, full, own) {
        res.logs.push(format!(
            "fkrecipes: {} is edited, but {} is not on {}, so the text is ignored",
            full, dropdown, cv
        ));
    }
}

/// Says out loud that a NUMBER nothing is reading was moved.
///
/// EDITED MEANS "NOT THE NUMBER THE MOD DECLARED", which is the whole question
/// a number can be asked: nothing stands in for the mod's own answer here the
/// way the word `default` stands for a list, so the declared default IS the
/// untouched value and a player who never moved the field hears nothing.
///
/// A setting that is not readable, or that answers with something other than a
/// number, draws nothing either, the same tolerance the text line has: the
/// dropdown sits on a preset, so nothing was going to read this number, and a
/// value this planner cannot read is not a value the player set.
///
/// THE GAME'S OWN WORLD ANSWERS HERE, not the own-items overlay the texts are
/// read under: no list is parsed, so there is nothing for the overlay to add.
fn note_ignored_number(
    w: &dyn World,
    res: &mut Resolution,
    s: &SettingDecl,
    prefix: &str,
    dropdown: &str,
    cv: &str,
) {
    let full = s.emitted_name(prefix);
    if let Some(Value::Num(v)) = w.startup_setting(&full) {
        if v != s.def_num {
            res.logs.push(format!(
                "fkrecipes: {} is edited, but {} is not on {}, so the number is ignored",
                full, dropdown, cv
            ));
        }
    }
}

/// What a setting that answered with something no arithmetic can use is told.
/// It names the SETTING rather than the technology, because the technology's
/// own declaration is fine and the value came from outside it.
fn not_finite(setting: &str) -> String {
    format!(
        "fkrecipes: {} holds a value that is not a finite number",
        setting
    )
}

fn drop_line(tech: &str, after: &str) -> String {
    format!(
        "fkrecipes: {}: {} is absent, so the prerequisite is dropped",
        tech, after
    )
}

fn join_names(names: &[String], sep: &str) -> String {
    let mut out = String::new();
    for (i, n) in names.iter().enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        out.push_str(n);
    }
    out
}

fn item_proto(prefix: &str, it: &ItemDecl) -> Value {
    let mut pairs = vec![
        kv("type", Value::string("item")),
        kv("name", Value::Str(it.emitted_name(prefix))),
    ];
    append_localised(&mut pairs, &it.spec.display_name, &it.spec.description);
    if !it.spec.icon.is_empty() {
        pairs.push(kv("icon", Value::string(&it.spec.icon)));
    }
    if it.spec.icon_size != 0 {
        pairs.push(kv("icon_size", Value::Num(it.spec.icon_size as f64)));
    }
    let stack = if it.spec.stack_size == 0 {
        50
    } else {
        it.spec.stack_size
    };
    pairs.push(kv("stack_size", Value::Num(stack as f64)));
    if !it.spec.subgroup.is_empty() {
        pairs.push(kv("subgroup", Value::string(&it.spec.subgroup)));
    }
    if !it.spec.order.is_empty() {
        pairs.push(kv("order", Value::string(&it.spec.order)));
    }
    if !it.spec.place_result.is_empty() {
        pairs.push(kv("place_result", Value::string(&it.spec.place_result)));
    }
    pairs.extend(it.spec.extra.iter().cloned());
    Value::Map(pairs)
}

fn recipe_proto(
    prefix: &str,
    l: &Lib,
    r: &RecipeDecl,
    ings: &[ResolvedIngredient],
    ct: &CraftTime,
    unlocked: bool,
) -> Value {
    let mut pairs = vec![
        kv("type", Value::string("recipe")),
        kv("name", Value::Str(r.emitted_name(prefix))),
    ];
    append_localised(&mut pairs, &r.spec.display_name, &r.spec.description);
    if !r.spec.category.is_empty() {
        pairs.push(kv("category", Value::string(&r.spec.category)));
    }
    // energy_required is omitted rather than sent as zero: an absent field is
    // the engine's own default, and a zero is a crafting time the engine
    // refuses. A bound recipe carries whatever the player's setting answered,
    // which the floor check has already cleared.
    if ct.bound {
        pairs.push(kv("energy_required", Value::Num(ct.value)));
    } else if r.spec.craft_time > 0.0 {
        pairs.push(kv("energy_required", Value::Num(r.spec.craft_time)));
    }
    // `enabled` MAY BE THE CONSUMER'S. Validation accepts the key in Extra
    // only when no technology in this plan unlocks the recipe, and the value
    // then takes the slot the library's own field would have had rather than
    // riding at the end with the rest of Extra: the field order a migrating
    // mod's golden saw does not move when the hand-written value arrives.
    match r.spec.extra.iter().find(|(k, _)| k == "enabled") {
        Some((_, v)) => pairs.push(kv("enabled", v.clone())),
        None => pairs.push(kv("enabled", Value::Bool(!unlocked))),
    }

    // Recipe ingredients are the LONG DICT form. The technology unit's short
    // tuple form is REFUSED here and the other way round; measured, not
    // generalised from one to the other.
    let mut items = Vec::with_capacity(ings.len());
    for ing in ings {
        let typ = if ing.amount.is_fluid() {
            "fluid"
        } else {
            "item"
        };
        items.push(Value::Map(vec![
            kv("type", Value::string(typ)),
            kv("name", Value::Str(ing.name.clone())),
            kv("amount", Value::Num(ing.amount.value())),
        ]));
    }
    pairs.push(kv("ingredients", Value::Arr(items)));

    let count = if r.spec.result_count == 0 {
        1
    } else {
        r.spec.result_count
    };
    // The result is either an item this plan declares (prefixed or legacy by
    // its own declaration) or one that already exists, named verbatim because
    // it is somebody else's and validation has probed it.
    let result = if r.result.index != 0 {
        l.items[r.result.index - 1].emitted_name(prefix)
    } else {
        r.spec.result_named.clone()
    };
    pairs.push(kv(
        "results",
        Value::Arr(vec![Value::Map(vec![
            kv("type", Value::string("item")),
            kv("name", Value::Str(result)),
            kv("amount", Value::Num(count as f64)),
        ])]),
    ));
    if !r.spec.order.is_empty() {
        pairs.push(kv("order", Value::string(&r.spec.order)));
    }
    // `enabled` is skipped here because it was already written above, in the
    // library's own slot.
    pairs.extend(r.spec.extra.iter().filter(|(k, _)| k != "enabled").cloned());
    Value::Map(pairs)
}

fn tech_proto(prefix: &str, l: &Lib, w: &dyn World, t: &TechDecl, rt: &ResolvedTech) -> Value {
    let mut pairs = vec![
        kv("type", Value::string("technology")),
        kv("name", Value::Str(t.emitted_name(prefix))),
    ];
    append_localised(&mut pairs, &t.spec.display_name, &t.spec.description);
    if !t.spec.icon.is_empty() {
        pairs.push(kv("icon", Value::string(&t.spec.icon)));
    }
    if t.spec.icon_size != 0 {
        pairs.push(kv("icon_size", Value::Num(t.spec.icon_size as f64)));
    }
    if !rt.prereqs.is_empty() {
        pairs.push(kv("prerequisites", str_arr(&rt.prereqs)));
    }
    pairs.push(kv("unit", tech_unit(w, t, rt)));
    // max_level lives on the TECHNOLOGY, not in its unit, so copying the unit
    // verbatim carries a count_formula but leaves the level cap behind.
    // cost_of is one named point for cost AND position, so it reads the cap
    // too: an infinite source technology produces an infinite copy. A
    // hand-rolled UnitSpec has no source to read, and gets no cap.
    if t.spec.cost_by.is_some() {
        // The ladder already read the cap off the source it settled on, and
        // dropped it if it was one this library could not carry.
        if let Some(level) = &rt.max_level {
            pairs.push(kv("max_level", level.clone()));
        }
    } else if t.spec.unit.is_none() && t.spec.cost_from.is_none() {
        // A present-but-nil read is a value this library could not carry (a
        // LuaObject, or a table with a key it drops); emitting it would write
        // a nil max_level into the prototype.
        match w.tech_max_level(&t.spec.cost_of) {
            Some(Value::Nil) | None => {}
            Some(level) => pairs.push(kv("max_level", level)),
        }
    }
    if !t.spec.unlocks.is_empty() {
        let mut effects = Vec::with_capacity(t.spec.unlocks.len());
        for u in &t.spec.unlocks {
            effects.push(Value::Map(vec![
                kv("type", Value::string("unlock-recipe")),
                kv(
                    "recipe",
                    Value::Str(l.recipes[u.index - 1].emitted_name(prefix)),
                ),
            ]));
        }
        pairs.push(kv("effects", Value::Arr(effects)));
    }
    // HIDDEN, NOT ABSENT. A technology researched in an existing save whose
    // prototype vanishes is dropped from that save, and flipping a startup
    // setting is exactly the mid-save event this library invites, so a
    // switched-off technology keeps its prototype and loses its visibility.
    if rt.has_enabled_by {
        if rt.on {
            pairs.push(kv("enabled", Value::Bool(true)));
        } else {
            pairs.push(kv("enabled", Value::Bool(false)));
            pairs.push(kv("hidden", Value::Bool(true)));
        }
    }
    if !t.spec.order.is_empty() {
        pairs.push(kv("order", Value::string(&t.spec.order)));
    }
    pairs.extend(t.spec.extra.iter().cloned());
    Value::Map(pairs)
}

fn tech_unit(w: &dyn World, t: &TechDecl, rt: &ResolvedTech) -> Value {
    if t.spec.cost_by.is_some() || t.spec.cost_from.is_some() {
        // Resolution walked the ladder and settled this, fallback included;
        // a CostFrom unit was built there too, out of the player's settings.
        return rt.unit.clone().unwrap_or(Value::Nil);
    }
    match &t.spec.unit {
        // Technology unit ingredients are the SHORT TUPLE form. The dict form
        // is refused here by the engine. The packs are the RESOLVED ones:
        // resolution walked each ladder, and a pack the game does not have is
        // already gone with its log line behind it.
        Some(unit) => unit_value(unit.count, unit.seconds, &rt.packs),
        // Verbatim, whatever it holds: a count_formula is a string and
        // copying one needs no evaluator, so multi-level and infinite
        // technologies come along for free. Validation proved the unit is
        // there; the absent case still maps to the same nil the Go mirror
        // returns.
        None => w.tech_unit(&t.spec.cost_of).unwrap_or(Value::Nil),
    }
}

fn append_localised(pairs: &mut Vec<(String, Value)>, display_name: &str, description: &str) {
    if !display_name.is_empty() {
        pairs.push(kv("localised_name", localised(display_name)));
    }
    if !description.is_empty() {
        pairs.push(kv("localised_description", localised(description)));
    }
}

/// Reports the marker a lossy read leaves behind.
///
/// A value this library cannot carry faithfully converts to Nil as a WHOLE
/// subtree rather than being partly kept, and fkdata never delivers a nil map
/// value or array element of its own (its write side skips nils), so a Nil
/// anywhere inside a value that came out of data.raw means exactly one thing:
/// a table was dropped on the way in. A copied unit that lost a subtree is a
/// technology researchable for free, which is why this is a refusal rather
/// than a log line.
pub(crate) fn holds_dropped_subtree(v: &Value) -> bool {
    match v {
        Value::Map(pairs) => pairs
            .iter()
            .any(|(_, val)| matches!(val, Value::Nil) || holds_dropped_subtree(val)),
        Value::Arr(items) => items
            .iter()
            .any(|item| matches!(item, Value::Nil) || holds_dropped_subtree(item)),
        _ => false,
    }
}

impl Lib {
    /// The category comes along because one of these rules is about the
    /// RECIPE rather than the ingredient: a fluid in the crafting category is
    /// a load failure the engine reports in its own words, and this refuses
    /// it first, by name, before anything is emitted.
    ///
    /// `None` asks every rule EXCEPT that one, which is what the settings
    /// stage can answer: a text setting's declared list is checked there
    /// before it is rendered into a description, and which recipe reads that
    /// setting, in which category, is a data-stage question.
    pub(crate) fn validate_ingredients(
        &self,
        at: &str,
        who: &str,
        category: Option<&str>,
        ings: &[Ingredient],
    ) -> Result<(), String> {
        for ing in ings {
            // THE NAME BEFORE THE NUMBERS, and before the category, which is
            // the one ordering that makes the sentences below safe: each of
            // them names the ladder's first candidate, and a first candidate
            // that is the empty string would leave a hole in the middle of a
            // refusal. A rung that can never resolve is the empty pack rung's
            // mistake with one word changed, and it gets the same answer.
            if ing.candidates.iter().any(|c| c.is_empty()) {
                return Err(format!(
                    "{}{} names an ingredient with an empty name",
                    at, who
                ));
            }
            match ing.amount {
                Amount::Item(n) => {
                    if n < 1 {
                        return Err(format!(
                            "{}{} has an ingredient amount below 1, which the engine refuses",
                            at, who
                        ));
                    }
                    if n > MAX_EXACT_INT {
                        return Err(format!(
                            "{}{} declares an ingredient amount a Lua double cannot hold exactly: {}",
                            at, who, n
                        ));
                    }
                }
                Amount::Fluid(v) => {
                    if !finite(v) {
                        return Err(format!(
                            "{}{} declares a fluid amount that is not a finite number",
                            at, who
                        ));
                    }
                    if v <= 0.0 {
                        return Err(format!(
                            "{}{} has a fluid amount at or below zero, which the engine refuses",
                            at, who
                        ));
                    }
                    if category.map(takes_items_only).unwrap_or(false) {
                        return Err(format!(
                            "{}{} takes the fluid {}, and a recipe in the crafting category takes items only",
                            at,
                            who,
                            first_candidate(ing)
                        ));
                    }
                    // THE CEILING IS THE PLAYER-TYPED PATH'S CEILING, asked
                    // of the author's own declaration for the same measured
                    // reason: above it the engine does not refuse the load,
                    // it aborts inside FixedPointNumber and hands the player
                    // the crash handler. The category question comes first
                    // because a fluid the recipe cannot take at all is the
                    // larger mistake, whatever its amount.
                    if v > MAX_FLUID_AMOUNT {
                        return Err(format!(
                            "{}{} takes the fluid {} at an amount above 1e301, which the game cannot hold",
                            at,
                            who,
                            first_candidate(ing)
                        ));
                    }
                }
            }
            if ing.candidates.is_empty() && !self.valid_item(ing.item) {
                return Err(format!(
                    "{}{} names an ingredient item that this plan never declared",
                    at, who
                ));
            }
            // THE ITEM CEILING, asked of the AUTHOR'S OWN LIST exactly as the
            // language asks it of a text the player typed: the engine holds an
            // item amount in a u16 and refuses 65536 with a message about a
            // data type (measured), so a declared list and a typed one are
            // held to one rule. It sits behind the handle check because the
            // sentence names what the recipe takes, and this plan's own item
            // has no candidate to name until its handle is proved.
            if let Amount::Item(n) = ing.amount {
                if n > MAX_ITEM_AMOUNT {
                    return Err(format!(
                        "{}{} takes {} of {}, and an item amount goes up to {}",
                        at,
                        who,
                        n,
                        self.ingredient_shown(ing),
                        MAX_ITEM_AMOUNT
                    ));
                }
            }
        }
        Ok(())
    }

    /// The name a refusal about a declared ingredient quotes: the ladder's
    /// first choice, or the DECLARED name of this plan's own item. Asked only
    /// after the handle has been proved, which is what makes the index safe.
    fn ingredient_shown(&self, ing: &Ingredient) -> String {
        match ing.candidates.first() {
            Some(name) => name.clone(),
            None => self.items[ing.item.index - 1].name.clone(),
        }
    }

    /// A player-written research cost: three handles this plan issued, and two
    /// declared bounds that make every value the engine can hand back legal.
    ///
    /// THE BOUNDS ARE ASKED OF THE DECLARATION, not of what the setting
    /// answers. The engine RESETS a stored value outside a setting's own range
    /// to that setting's default rather than clamping it (measured), and it
    /// refuses to load a numeric setting whose default lies outside its own
    /// bounds, so a minimum of at least 1 on the count and above 0 on the
    /// seconds closes the chain: nothing this library can read back is a count
    /// of 0 or a time of 0, and both of those the engine refuses in a unit.
    pub(crate) fn validate_custom_cost(
        &self,
        at: &str,
        who: &str,
        cc: &CustomCost,
    ) -> Result<(), String> {
        if !self.valid_packs_setting(cc.packs) {
            return Err(format!(
                "{}{} reads its science packs from a setting that this plan never declared",
                at, who
            ));
        }
        if !self.valid_int_setting(cc.count) {
            return Err(format!(
                "{}{} reads its research count from a setting that this plan never declared",
                at, who
            ));
        }
        if !self.valid_double_setting(cc.seconds) {
            return Err(format!(
                "{}{} reads its research time from a setting that this plan never declared",
                at, who
            ));
        }
        let count = &self.settings[cc.count.index - 1];
        if count.spec.min.map(|m| m < 1.0).unwrap_or(true) {
            return Err(format!(
                "{}the setting {} backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)",
                at, count.name
            ));
        }
        let seconds = &self.settings[cc.seconds.index - 1];
        if seconds.spec.min.map(|m| m <= 0.0).unwrap_or(true) {
            return Err(format!(
                "{}the setting {} backs a research time but declares no minimum above 0 (the engine refuses a unit time of 0)",
                at, seconds.name
            ));
        }
        Ok(())
    }

    fn validate_unit(&self, at: &str, name: &str, u: &UnitSpec) -> Result<(), String> {
        if u.count < 1 {
            return Err(format!(
                "{}the technology {} has a unit count below 1, which the engine refuses",
                at, name
            ));
        }
        // A UNIT THAT NAMED NO PACK AT ALL is refused here, before any world
        // question, and that placement is the whole point: the sentence about
        // packs the game does not have is reserved for a list that named some
        // and lost them all, so an author who simply forgot to price the
        // research is told THAT rather than being told the game is missing
        // something. It sits behind the count check so a fallback with a
        // count of zero still hears about the count.
        if u.packs.is_empty() {
            return Err(format!(
                "{}the technology {} declares no science pack; research takes at least one",
                at, name
            ));
        }
        if u.count > MAX_EXACT_INT {
            return Err(format!(
                "{}the technology {} declares a unit count a Lua double cannot hold exactly: {}",
                at, name, u.count
            ));
        }
        if !u.seconds.is_finite() {
            return Err(format!(
                "{}the technology {} declares a research time that is not a finite number",
                at, name
            ));
        }
        if u.seconds <= 0.0 {
            return Err(format!(
                "{}the technology {} has a research time at or below zero, which the engine refuses",
                at, name
            ));
        }
        for p in &u.packs {
            if p.amount < 1 {
                return Err(format!(
                    "{}the technology {} has a science pack amount below 1, which the engine refuses",
                    at, name
                ));
            }
            if p.amount > MAX_EXACT_INT {
                return Err(format!(
                    "{}the technology {} declares a science pack amount a Lua double cannot hold exactly: {}",
                    at, name, p.amount
                ));
            }
            // EVERY RUNG, not just the first: an empty name in a fallback is
            // a rung that can never resolve and would silently shorten the
            // ladder the author wrote.
            if p.name.is_empty() || p.fallbacks.iter().any(|f| f.is_empty()) {
                return Err(format!(
                    "{}the technology {} prices itself in a pack with an empty name",
                    at, name
                ));
            }
            // WHETHER THE GAME HAS THE PACK IS NOT ASKED HERE. It is a ladder
            // now, and a ladder is walked at resolution, where a pack the
            // game does not have is dropped with a log line the way an
            // ingredient is.
        }
        Ok(())
    }
}

/// The dropdown values a Choices list has to cover: all of them, minus the one
/// the Custom arm answers to.
fn presets_of<T>(values: &[String], custom: &Option<T>, cv: &str) -> Vec<String> {
    if custom.is_none() {
        return values.to_vec();
    }
    values
        .iter()
        .filter(|v| v.as_str() != cv)
        .cloned()
        .collect()
}

fn matches_allowed_values(
    at: &str,
    who: &str,
    setting: &str,
    offered: &[String],
    allowed: &[String],
) -> Result<(), String> {
    let n = offered.len().max(allowed.len());
    for i in 0..n {
        if i >= offered.len() {
            return Err(format!(
                "{}{} offers nothing for the value {} that the setting {} allows",
                at, who, allowed[i], setting
            ));
        }
        if i >= allowed.len() {
            return Err(format!(
                "{}{} offers something for {}, which the setting {} does not allow",
                at, who, offered[i], setting
            ));
        }
        if offered[i] != allowed[i] {
            return Err(format!(
                "{}{} offers something for {} where the setting {} allows {}",
                at, who, offered[i], setting, allowed[i]
            ));
        }
    }
    Ok(())
}

impl Resolution {
    /// The dropdown's chosen value, or its default when the game does not
    /// answer with a string. An unreadable setting is logged once here, the
    /// same way enablement and crafting time log theirs.
    ///
    /// A STORED VALUE THE DROPDOWN DOES NOT LIST IS REFUSED. The engine resets
    /// one to the default before any stage runs (measured), so this is only
    /// reachable through a mod-settings.dat somebody edited by hand; what it
    /// replaces is worse than a refusal, because the value simply matched no
    /// choice and the recipe came out made of nothing with no line in the log.
    fn read_dropdown(&mut self, w: &dyn World, s: &SettingDecl, prefix: &str) -> String {
        let full = s.emitted_name(prefix);
        // What the refusal below quotes, for the two shapes a stored string
        // arrives in. A value whose bytes are not text takes the SAME refusal
        // as any other unlisted value, because that is what it is: an offered
        // value comes from the author's own source and is text, so no offered
        // value can be these bytes.
        //
        // IT IS NOT COMPARED, only quoted. Rendering it for the sentence is
        // lossy (`fkdata::raise` takes a `&str`, so a refusal is text on this
        // side; the Go half quotes the raw bytes, which is the one place the
        // two sentences can differ), and a lossy rewrite fed into the
        // comparison could match an offered value the engine never stored.
        let quoted = match w.startup_setting(&full) {
            Some(Value::Str(v)) => {
                if s.values.contains(&v) {
                    return v;
                }
                v
            }
            Some(Value::Bytes(b)) => String::from_utf8_lossy(&b).into_owned(),
            _ => {
                self.logs.push(format!(
                    "fkrecipes: the setting {} was not readable, so its default applies",
                    full
                ));
                return s.def_str.clone();
            }
        };
        self.refuse(format!(
            "fkrecipes: {} holds \"{}\", which is not one of its values",
            full, quoted
        ));
        s.def_str.clone()
    }
}

fn choice_for<'a>(choices: &'a [IngredientChoice], value: &str) -> &'a [Ingredient] {
    for c in choices {
        if c.value == value {
            return &c.ingredients;
        }
    }
    &[]
}

fn sources_for<'a>(choices: &'a [CostChoice], value: &str) -> &'a [String] {
    for c in choices {
        if c.value == value {
            return &c.sources;
        }
    }
    &[]
}

/// The World a PLAYER'S TEXT is read under: the game as it stands, plus the
/// items THIS PLAN is about to emit.
///
/// MEASURED (2.0.77): a text copied straight out of the setting's own
/// description (`1 fkrecipes-example-steel-rivet, 10 water`) refused the load
/// with "no item or fluid is named fkrecipes-example-steel-rivet". The data
/// planner resolves texts before its own items reach data.raw, so the one
/// list the description invites the player to copy was the one list they could
/// not type. The overlay is the smaller of the two fixes: emitting the items
/// first would work too and would reorder the whole stream.
///
/// ITEMS ONLY, and the two other probes are untouched on purpose: a plan's own
/// items are never fluids and never tools, so a pack text naming one still
/// hears "is an item, not a science pack", which is the true answer.
///
/// The names are a VECTOR in declaration order, like everything else here.
struct PlanItems<'a> {
    inner: &'a dyn World,
    names: Vec<String>,
}

impl crate::world::Named for PlanItems<'_> {
    fn mod_name(&self) -> String {
        self.inner.mod_name()
    }
}

impl World for PlanItems<'_> {
    fn item_exists(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name) || self.inner.item_exists(name)
    }

    // Everything else is the game's own answer.
    fn startup_setting(&self, name: &str) -> Option<Value> {
        self.inner.startup_setting(name)
    }
    fn tech_names(&self) -> Vec<String> {
        self.inner.tech_names()
    }
    fn tech_prereqs(&self, name: &str) -> Vec<String> {
        self.inner.tech_prereqs(name)
    }
    fn tech_unit(&self, name: &str) -> Option<Value> {
        self.inner.tech_unit(name)
    }
    fn tech_max_level(&self, name: &str) -> Option<Value> {
        self.inner.tech_max_level(name)
    }
    fn tech_has_research_trigger(&self, name: &str) -> bool {
        self.inner.tech_has_research_trigger(name)
    }
    fn tech_exists(&self, name: &str) -> bool {
        self.inner.tech_exists(name)
    }
    fn entity_exists(&self, name: &str) -> bool {
        self.inner.entity_exists(name)
    }
    fn recipe_exists(&self, name: &str) -> bool {
        self.inner.recipe_exists(name)
    }
    // The two DEFAULT methods are overridden to delegate rather than left to
    // panic: a consumer's fixture answers them, and this overlay must not
    // stand between a fluid ingredient and the World that knows about it.
    fn fluid_exists(&self, name: &str) -> bool {
        self.inner.fluid_exists(name)
    }
    fn tool_exists(&self, name: &str) -> bool {
        self.inner.tool_exists(name)
    }
}

impl Lib {
    /// The overlay, built once per data plan from this plan's own item
    /// declarations. The EMITTED name is what goes in, prefixed or legacy by
    /// the declaration's own rule, because that is the name the description
    /// showed and the name the recipe will carry.
    fn plan_items<'a>(&self, w: &'a dyn World, prefix: &str) -> PlanItems<'a> {
        let mut names = Vec::with_capacity(self.items.len());
        for it in &self.items {
            names.push(it.emitted_name(prefix));
        }
        PlanItems { inner: w, names }
    }

    /// What a text setting says, as the language reads it.
    ///
    /// AN UNREADABLE SETTING BECOMES THE WORD, which is the same degradation
    /// every other bound setting takes and lands on the same path a player who
    /// never typed takes. A readable value that is not a string is refused:
    /// the engine resets a wrong-typed stored value to the default before any
    /// stage runs (measured), so this is a hand-edited file, and guessing what
    /// a number meant as an ingredient list is not something to do on a
    /// player's behalf.
    ///
    /// IT YIELDS BYTES, and both string arms are one answer here. Whether a
    /// stored value is text is the LANGUAGE's question, asked once at the top
    /// of its parse and answered the same way in both halves; a second answer
    /// taken here, by treating `Value::Bytes` as "not a string", would refuse
    /// with the wrong sentence and would be a rule only this half has.
    fn read_text_setting(
        &self,
        w: &dyn World,
        res: &mut Resolution,
        full: &str,
    ) -> Option<Vec<u8>> {
        match w.startup_setting(full) {
            Some(Value::Str(text)) => Some(text.into_bytes()),
            Some(Value::Bytes(b)) => Some(b),
            Some(_) => {
                res.refuse(format!("fkrecipes: {} is not text", full));
                None
            }
            None => {
                res.logs.push(format!(
                    "fkrecipes: the setting {} was not readable, so its default applies",
                    full
                ));
                Some(Vec::from(DEFAULT.as_bytes()))
            }
        }
    }

    /// One recipe's ingredients, from the text the player wrote.
    ///
    /// THE WORD `default` IS THE PRE-EXISTING PATH, ladders and all, and it
    /// logs nothing of its own: the player who never typed gets exactly the
    /// recipe the author declared, drops included. Anything else is taken as
    /// written, and one line records what was read.
    ///
    /// TWO WORLDS, AND THE SPLIT IS THE POINT. What the PLAYER typed is read
    /// against `own`, which knows this plan's own item names; the author's own
    /// ladders behind the word `default` are walked against the game as it
    /// stands, exactly as they were before, because a ladder is a tolerance
    /// for a modpack rather than a lookup of this plan's own prototypes.
    fn resolve_text_ingredients(
        &self,
        w: &dyn World,
        own: &dyn World,
        res: &mut Resolution,
        prefix: &str,
        r: &RecipeDecl,
        h: IngredientsSettingRef,
    ) -> Vec<ResolvedIngredient> {
        let s = &self.settings[h.index - 1];
        let full = s.emitted_name(prefix);
        let text = match self.read_text_setting(w, res, &full) {
            Some(text) => text,
            None => return Vec::new(),
        };
        let lang = self.installed_language();
        match (lang.parse)(&text, ListKind::Recipe, &r.spec.category, &full, own) {
            // THE MESSAGE IS THE WHOLE REFUSAL, verbatim: the language wrote
            // it for the player, naming the setting, the entry and the
            // problem, and there is nothing this layer can add to it.
            Err(message) => {
                res.refuse(message);
                Vec::new()
            }
            Ok(ListText::Default) => self.resolve_ingredients(w, res, prefix, &r.name, &s.def_ings),
            Ok(ListText::List(list)) => {
                let out: Vec<ResolvedIngredient> = list
                    .entries
                    .iter()
                    .map(|e| ResolvedIngredient {
                        name: e.name.clone(),
                        amount: e.amount,
                    })
                    .collect();
                res.logs.push(format!(
                    "fkrecipes: {} takes its ingredients from {}: {}",
                    r.emitted_name(prefix),
                    full,
                    (lang.render_list)(&list)
                ));
                out
            }
        }
    }

    /// One technology's unit, from two numeric settings and a pack text.
    ///
    /// The three are read in the order the log line names them and the order
    /// the unit carries them: count, time, packs.
    ///
    /// THE TWO NUMBERS ARE FACTS ABOUT THE WORLD, not about the declaration,
    /// so they are asked the finiteness question the resolved crafting time is
    /// asked and then the bound their own declaration promised. The declared
    /// minima keep the ENGINE from handing back a count below 1 or a time at
    /// or below zero (measured: a stored value outside a setting's own bounds
    /// is reset to that setting's default), but a fixture World can answer
    /// anything at all, and a NaN reaching the decimal rule is a unit rendered
    /// as `NaN` in this half and a trap in the Go mirror.
    fn resolve_custom_cost(
        &self,
        w: &dyn World,
        own: &dyn World,
        res: &mut Resolution,
        prefix: &str,
        t: &TechDecl,
        cc: &CustomCost,
    ) -> (Value, bool) {
        let lang = self.installed_language();
        let (count, count_setting) = self.read_num_setting(w, res, prefix, cc.count.index);
        let (seconds, seconds_setting) = self.read_num_setting(w, res, prefix, cc.seconds.index);
        let s = &self.settings[cc.packs.index - 1];
        let full = s.emitted_name(prefix);
        let packs = match self.read_text_setting(w, res, &full) {
            None => Vec::new(),
            Some(text) => match (lang.parse)(&text, ListKind::Packs, "", &full, own) {
                Err(message) => {
                    res.refuse(message);
                    Vec::new()
                }
                // THE LADDERS ARE THE AUTHOR'S, so the word walks them and a
                // pack the game does not have is dropped with its line, the
                // way it is for a hand-rolled unit.
                Ok(ListText::Default) => resolve_packs(w, res, &t.name, &s.def_packs),
                Ok(ListText::List(list)) => list
                    .entries
                    .iter()
                    .map(|e| ResolvedPack {
                        name: e.name.clone(),
                        // A pack list resolves through tool_exists and a tool
                        // is an item, so every entry the parser returns here
                        // carries an item amount: `resolve_for_packs` answers
                        // "not a fluid" for every name it accepts, and the
                        // fluid arm of an entry is reached only behind that
                        // answer. A 0 here would be a research the engine
                        // refuses with a message naming nothing of this
                        // library's, so the impossible case says so instead.
                        amount: match e.amount {
                            Amount::Item(n) => n,
                            Amount::Fluid(_) => {
                                unreachable!("a science pack list parsed a fluid entry")
                            }
                        },
                    })
                    .collect(),
            },
        };
        // THE NUMBERS ARE ASKED AFTER THE TEXT, and a bad text answers first.
        // All three fields are the player's and a world where two of them are
        // wrong is a world the two halves would otherwise report differently;
        // the pack text is the field a player is likeliest to have typed by
        // hand, so it is the one whose sentence comes back. See
        // agents/customizer-design.md.
        //
        // THE ORDER IS FINITENESS FIRST, both numbers, and only then the two
        // floors: a floor is a question only a finite number can be asked. A
        // NaN is neither below 1 nor at or below zero, so a floor arm reached
        // first would wave it through, and an infinity would be sorted by
        // whichever side of the floor it fell on rather than told the one
        // thing that is actually wrong with it. One arm answers, because the
        // first refusal is the one a Resolution keeps.
        if !finite(count) {
            res.refuse(not_finite(&count_setting));
        } else if !finite(seconds) {
            res.refuse(not_finite(&seconds_setting));
        } else if count < 1.0 {
            res.refuse(format!(
                "fkrecipes: {} holds a research count below 1",
                count_setting
            ));
        } else if seconds <= 0.0 {
            res.refuse(format!(
                "fkrecipes: {} holds a research time at or below zero",
                seconds_setting
            ));
        }
        // ONE LINE WHATEVER THE TEXT SAID, unlike the ingredients path: the
        // count and the seconds come from their settings on every load, so
        // there is always something the player set that this records.
        res.logs.push(format!(
            "fkrecipes: {} takes its research cost from {}: count {}, time {}, packs {}",
            t.emitted_name(prefix),
            full,
            (lang.format_amount)(count),
            (lang.format_amount)(seconds),
            (lang.render_list)(&resolved_pack_list(&packs))
        ));
        // The count is emitted as the NUMBER the setting answered with, not as
        // an integer this library rounded: the engine's own field is a double
        // like every other, and the setting's declared bounds are what keep it
        // a whole one.
        let ings: Vec<Value> = packs
            .iter()
            .map(|p| {
                Value::Arr(vec![
                    Value::Str(p.name.clone()),
                    Value::Num(p.amount as f64),
                ])
            })
            .collect();
        (
            Value::Map(vec![
                kv("count", Value::Num(count)),
                kv("time", Value::Num(seconds)),
                kv("ingredients", Value::Arr(ings)),
            ]),
            packs.is_empty(),
        )
    }

    /// A number a player set, or the declared default with a line saying the
    /// setting was not readable, AND the setting's emitted name, because the
    /// caller's refusals name whichever setting answered. One reader for both
    /// numeric kinds: an int setting and a double setting both answer with a
    /// Lua number, and an int setting's declared default is already a double
    /// by the time it is here.
    fn read_num_setting(
        &self,
        w: &dyn World,
        res: &mut Resolution,
        prefix: &str,
        index: usize,
    ) -> (f64, String) {
        let s = &self.settings[index - 1];
        let full = s.emitted_name(prefix);
        match w.startup_setting(&full) {
            Some(Value::Num(n)) => (n, full),
            _ => {
                res.logs.push(format!(
                    "fkrecipes: the setting {} was not readable, so its default applies",
                    full
                ));
                (s.def_num, full)
            }
        }
    }

    fn resolve_ingredients(
        &self,
        w: &dyn World,
        res: &mut Resolution,
        prefix: &str,
        recipe: &str,
        ings: &[Ingredient],
    ) -> Vec<ResolvedIngredient> {
        let mut list = Vec::with_capacity(ings.len());
        for ing in ings {
            if ing.candidates.is_empty() {
                list.push(ResolvedIngredient {
                    name: self.items[ing.item.index - 1].emitted_name(prefix),
                    amount: ing.amount,
                });
                continue;
            }
            let mut picked: Option<String> = None;
            for c in &ing.candidates {
                // Items and fluids are separate namespaces, so the ladder
                // asks the question its own kind answers. A fluid rung found
                // among the items would be a name the recipe cannot use.
                let present = if ing.is_fluid() {
                    w.fluid_exists(c)
                } else {
                    w.item_exists(c)
                };
                if present {
                    picked = Some(c.clone());
                    break;
                }
            }
            match picked {
                Some(name) => list.push(ResolvedIngredient {
                    name,
                    amount: ing.amount,
                }),
                None => res.logs.push(format!(
                    "fkrecipes: {}: none of {} is present, so the ingredient is dropped",
                    recipe,
                    join_names(&ing.candidates, ", ")
                )),
            }
        }
        list
    }
}

/// Walks every pack's ladder and reports what the game actually has.
///
/// A PACK IS DROPPED, NOT REFUSED, exactly as an ingredient is, and the log
/// line is the ingredient's line with one word changed: the author who wrote
/// a ladder asked for tolerance, and a modpack that renamed the science packs
/// is the case the ladder is for. The unit that ends up with nothing is
/// refused later, by name, because that one cannot be researched at all.
///
/// THROUGH `tool_exists`, NEVER `item_exists`: the engine takes tool-type
/// items in a research unit and nothing else (measured: "Invalid research
/// unit (iron-plate). Research unit(s) can only be tool type items at the
/// moment."), so a rung that is an item but not a tool is not a rung.
fn resolve_packs(
    w: &dyn World,
    res: &mut Resolution,
    tech: &str,
    packs: &[Pack],
) -> Vec<ResolvedPack> {
    let mut list = Vec::with_capacity(packs.len());
    for p in packs {
        let mut picked: Option<String> = None;
        if w.tool_exists(&p.name) {
            picked = Some(p.name.clone());
        } else {
            for f in &p.fallbacks {
                if w.tool_exists(f) {
                    picked = Some(f.clone());
                    break;
                }
            }
        }
        match picked {
            Some(name) => list.push(ResolvedPack {
                name,
                amount: p.amount,
            }),
            None => res.logs.push(format!(
                "fkrecipes: {}: none of {} is present, so the science pack is dropped",
                tech,
                pack_ladder(p)
            )),
        }
    }
    list
}

/// The packs a unit was actually priced in, in the shape the language writes
/// out: what the log line prints is what a player could type back into the
/// field.
fn resolved_pack_list(packs: &[ResolvedPack]) -> IngredientList {
    let mut entries = Vec::with_capacity(packs.len());
    for p in packs {
        entries.push(ListEntry {
            name: p.name.clone(),
            amount: Amount::Item(p.amount),
        });
    }
    IngredientList { entries }
}

/// Walks a Custom arm's Position ladder. The first technology the game has
/// becomes the sole prerequisite, exactly as a chosen tier's source would; a
/// ladder with no rung present leaves the technology unattached and says so, in
/// the shape every other dropped ladder uses.
fn custom_prereqs(
    w: &dyn World,
    res: &mut Resolution,
    tech: &str,
    position: &[String],
) -> Vec<String> {
    for name in position {
        if w.tech_exists(name) {
            return vec![name.clone()];
        }
    }
    res.logs.push(format!(
        "fkrecipes: {}: none of {} is present, so the technology has no prerequisite",
        tech,
        join_names(position, ", ")
    ));
    Vec::new()
}

/// A pack's rungs as the drop line names them, first choice first.
fn pack_ladder(p: &Pack) -> String {
    let mut out = p.name.clone();
    for f in &p.fallbacks {
        out.push_str(", ");
        out.push_str(f);
    }
    out
}

/// A hand-rolled cost as the engine wants it, built from the packs the game
/// answered for rather than from the ones the author wrote.
fn unit_value(count: i64, seconds: f64, packs: &[ResolvedPack]) -> Value {
    let ings: Vec<Value> = packs
        .iter()
        .map(|p| {
            Value::Arr(vec![
                Value::Str(p.name.clone()),
                Value::Num(p.amount as f64),
            ])
        })
        .collect();
    Value::Map(vec![
        kv("count", Value::Num(count as f64)),
        kv("time", Value::Num(seconds)),
        kv("ingredients", Value::Arr(ings)),
    ])
}

/// The field names each prototype builder writes itself. A key in `extra` that
/// collides with one is REFUSED rather than merged: two writers of one field is
/// a silent last-writer, and the loser would be whichever order this library
/// happens to append in. Listed rather than derived, because a builder emits a
/// field CONDITIONALLY and the answer must not depend on which arms fired for
/// this particular declaration: an extra key that collides only when a sibling
/// field happens to be set would be a refusal a consumer could not reproduce.
const ITEM_OWN_FIELDS: &[&str] = &[
    "type",
    "name",
    "localised_name",
    "localised_description",
    "icon",
    "icon_size",
    "stack_size",
    "subgroup",
    "order",
    "place_result",
];
// `enabled` is NOT in the recipe list, and is the single exception in this
// file: `validate` takes it on its own, because whether the library owns it
// depends on the PLAN rather than on which arms the builder fired, and the
// sentence it is refused with names the technology that decided.
const RECIPE_OWN_FIELDS: &[&str] = &[
    "type",
    "name",
    "localised_name",
    "localised_description",
    "category",
    "energy_required",
    "ingredients",
    "results",
    "order",
];
const TECH_OWN_FIELDS: &[&str] = &[
    "type",
    "name",
    "localised_name",
    "localised_description",
    "icon",
    "icon_size",
    "prerequisites",
    "unit",
    "max_level",
    "effects",
    "enabled",
    "hidden",
    "order",
];

fn check_extra(at: &str, who: &str, extra: &[(String, Value)], own: &[&str]) -> Result<(), String> {
    for (i, (key, _)) in extra.iter().enumerate() {
        if key.is_empty() {
            return Err(format!(
                "{}{} sets a field through Extra with an empty name",
                at, who
            ));
        }
        if own.contains(&key.as_str()) {
            return Err(format!(
                "{}{} sets {} through Extra, which this library emits",
                at, who, key
            ));
        }
        // Two extra keys writing one field is the same silent last-writer, and
        // this one is entirely the consumer's own doing.
        if extra.iter().take(i).any(|(earlier, _)| earlier == key) {
            return Err(format!("{}{} sets {} through Extra twice", at, who, key));
        }
    }
    Ok(())
}

/// Whether a recipe in this category takes items and nothing else. An empty
/// category is the engine's own default, which IS `crafting` (measured: a
/// fluid ingredient with no category refuses with "Recipe is in 'crafting'
/// category but has a non-item ingredient 'water' (fluid)."), so the two
/// spellings are one answer.
///
/// ONE PREDICATE FOR TWO CALLERS: the recipe validation asks it of the
/// author's declaration and the ingredient list asks it of the player's, and
/// a rule spelled twice is a rule that can drift in one place.
pub(crate) fn takes_items_only(category: &str) -> bool {
    category.is_empty() || category == "crafting"
}

/// The name a refusal about a laddered ingredient quotes: the author's first
/// choice, which is the one they wrote the declaration for. Validation runs
/// before resolution, so which rung the game actually has is not known yet
/// and cannot be what the sentence names.
fn first_candidate(ing: &Ingredient) -> String {
    match ing.candidates.first() {
        Some(name) => name.clone(),
        None => String::new(),
    }
}
