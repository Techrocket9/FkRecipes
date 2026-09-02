use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::op::{path_key, Op};
use crate::plan::{
    CostChoice, Ingredient, IngredientChoice, ItemDecl, Lib, RecipeDecl, SettingDecl, TechDecl,
    UnitSpec,
};
use crate::value::{finite, kv, localised, str_arr, Value, CRAFT_TIME_FLOOR, MAX_EXACT_INT};
use crate::world::World;

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
        self.check_resolved_craft_times(&res)?;
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
            check_extra(
                at,
                &format!("the recipe {}", r.name),
                &r.spec.extra,
                RECIPE_OWN_FIELDS,
            )?;
            if r.spec.craft_time != 0.0 && r.spec.craft_time_from.index != 0 {
                return Err(format!(
                    "{}the recipe {} names both CraftTime and CraftTimeFrom; pick one",
                    at, r.name
                ));
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
                let offered: Vec<String> = by.choices.iter().map(|c| c.value.clone()).collect();
                matches_allowed_values(
                    at,
                    &format!("the recipe {}", r.name),
                    &setting.emitted_name(prefix),
                    &offered,
                    &setting.values,
                )?;
                for c in &by.choices {
                    self.validate_ingredients(
                        at,
                        &format!("the recipe {}", r.name),
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
            self.validate_ingredients(at, &format!("the recipe {}", r.name), &r.spec.ingredients)?;
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
            let named = [has_cost, has_unit, has_cost_by]
                .iter()
                .filter(|x| **x)
                .count();
            if named != 1 {
                return Err(format!(
                    "{}the technology {} must name exactly one of CostOf, Unit or CostBy",
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
                let offered: Vec<String> = by.choices.iter().map(|c| c.value.clone()).collect();
                matches_allowed_values(
                    at,
                    &format!("the technology {}", t.name),
                    &setting.emitted_name(prefix),
                    &offered,
                    &setting.values,
                )?;
                self.validate_unit(at, w, &t.name, &by.fallback)?;
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
                    self.validate_unit(at, w, &t.name, unit)?;
                }
                // A CostBy technology reaches here with neither field set,
                // and has nothing named to check: its ladder is walked at
                // resolution, where a source that cannot be used is stepped
                // past rather than refused.
                None if has_cost_by => {}
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
                None => {
                    let list =
                        self.resolve_ingredients(w, &mut res, prefix, &r.name, &r.spec.ingredients);
                    res.recipes.push(list);
                }
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

            if let Some(by) = &t.spec.cost_by {
                let setting = &self.settings[by.setting.index - 1];
                let chosen = res.read_dropdown(w, setting, prefix);
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
                    rt.unit = Some(unit_value(&by.fallback));
                    res.logs.push(format!(
                        "fkrecipes: {}: no source for the {} cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
                        t.name, chosen
                    ));
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
            // a recipe that never completes. This is the one float in the
            // library that arrives from outside and so never crossed the
            // declaration checks.
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
    pub(crate) amount: i64,
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
    pairs.push(kv("enabled", Value::Bool(!unlocked)));

    // Recipe ingredients are the LONG DICT form. The technology unit's short
    // tuple form is REFUSED here and the other way round; measured, not
    // generalised from one to the other.
    let mut items = Vec::with_capacity(ings.len());
    for ing in ings {
        items.push(Value::Map(vec![
            kv("type", Value::string("item")),
            kv("name", Value::Str(ing.name.clone())),
            kv("amount", Value::Num(ing.amount as f64)),
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
    pairs.extend(r.spec.extra.iter().cloned());
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
    } else if t.spec.unit.is_none() {
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
    if t.spec.cost_by.is_some() {
        // Resolution walked the ladder and settled this, fallback included.
        return rt.unit.clone().unwrap_or(Value::Nil);
    }
    match &t.spec.unit {
        // Technology unit ingredients are the SHORT TUPLE form. The dict form
        // is refused here by the engine.
        Some(unit) => {
            let mut packs = Vec::with_capacity(unit.packs.len());
            for p in &unit.packs {
                packs.push(Value::Arr(vec![
                    Value::string(&p.name),
                    Value::Num(p.amount as f64),
                ]));
            }
            Value::Map(vec![
                kv("count", Value::Num(unit.count as f64)),
                kv("time", Value::Num(unit.seconds)),
                kv("ingredients", Value::Arr(packs)),
            ])
        }
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
    fn validate_ingredients(&self, at: &str, who: &str, ings: &[Ingredient]) -> Result<(), String> {
        for ing in ings {
            if ing.amount < 1 {
                return Err(format!(
                    "{}{} has an ingredient amount below 1, which the engine refuses",
                    at, who
                ));
            }
            if ing.amount > MAX_EXACT_INT {
                return Err(format!(
                    "{}{} declares an ingredient amount a Lua double cannot hold exactly: {}",
                    at, who, ing.amount
                ));
            }
            if ing.candidates.is_empty() && !self.valid_item(ing.item) {
                return Err(format!(
                    "{}{} names an ingredient item that this plan never declared",
                    at, who
                ));
            }
        }
        Ok(())
    }

    fn validate_unit(
        &self,
        at: &str,
        w: &dyn World,
        name: &str,
        u: &UnitSpec,
    ) -> Result<(), String> {
        if u.count < 1 {
            return Err(format!(
                "{}the technology {} has a unit count below 1, which the engine refuses",
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
            if p.name.is_empty() {
                return Err(format!(
                    "{}the technology {} prices itself in a pack with an empty name",
                    at, name
                ));
            }
            if !w.item_exists(&p.name) {
                return Err(format!(
                    "{}the technology {} prices itself in {}, which does not exist",
                    at, name, p.name
                ));
            }
        }
        Ok(())
    }
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
    fn read_dropdown(&mut self, w: &dyn World, s: &SettingDecl, prefix: &str) -> String {
        let full = s.emitted_name(prefix);
        if let Some(Value::Str(v)) = w.startup_setting(&full) {
            return v;
        }
        self.logs.push(format!(
            "fkrecipes: the setting {} was not readable, so its default applies",
            full
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

impl Lib {
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
                if w.item_exists(c) {
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

/// A hand-rolled cost as the engine wants it.
fn unit_value(u: &UnitSpec) -> Value {
    let packs: Vec<Value> = u
        .packs
        .iter()
        .map(|p| {
            Value::Arr(vec![
                Value::Str(p.name.clone()),
                Value::Num(p.amount as f64),
            ])
        })
        .collect();
    Value::Map(vec![
        kv("count", Value::Num(u.count as f64)),
        kv("time", Value::Num(u.seconds)),
        kv("ingredients", Value::Arr(packs)),
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
const RECIPE_OWN_FIELDS: &[&str] = &[
    "type",
    "name",
    "localised_name",
    "localised_description",
    "category",
    "energy_required",
    "enabled",
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
