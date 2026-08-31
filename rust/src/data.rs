use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::op::{path_key, Op};
use crate::plan::{ItemDecl, Lib, RecipeDecl, TechDecl};
use crate::value::{finite, kv, localised, str_arr, Value, MAX_EXACT_INT};
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

        self.validate(w, &stage, &prefix)?;
        let res = self.resolve(w, &prefix);
        self.check_cycles(w, &res, &prefix, &stage)?;

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
    fn validate(&self, w: &dyn World, stage: &str, prefix: &str) -> Result<(), String> {
        let at = format!("fkrecipes: at the {} stage, ", stage);

        for (i, it) in self.items.iter().enumerate() {
            if it.name.is_empty() {
                return Err(format!("{}an item was declared with an empty name", at));
            }
            for other in self.items.iter().take(i) {
                if other.name == it.name {
                    return Err(format!(
                        "{}two items share the name {}; the second would overwrite the first",
                        at, it.name
                    ));
                }
            }
            // V1 never rewrites another mod's prototype except to splice a
            // prerequisite, and the cycle overlay counts on every planned
            // name being new: two nodes with one name is a walk that misses
            // the ring.
            if w.item_exists(&format!("{}{}", prefix, it.name)) {
                return Err(format!(
                    "{}the item {}{} already exists in data.raw; this plan would overwrite it",
                    at, prefix, it.name
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
        }

        for (i, r) in self.recipes.iter().enumerate() {
            // The result handle first: a recipe with no result has no name to
            // report either, and "two recipes share the name" with an empty
            // name is a worse answer than the one that says what is actually
            // wrong.
            if !self.valid_item(r.result) {
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
            for other in self.recipes.iter().take(i) {
                if other.name == r.name {
                    return Err(format!(
                        "{}two recipes share the name {}; the second would overwrite the first",
                        at, r.name
                    ));
                }
            }
            if !finite(r.spec.craft_time) {
                return Err(format!(
                    "{}the recipe {} declares a crafting time that is not a finite number",
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
            if w.recipe_exists(&format!("{}{}", prefix, r.name)) {
                return Err(format!(
                    "{}the recipe {}{} already exists in data.raw; this plan would overwrite it",
                    at, prefix, r.name
                ));
            }
            for ing in &r.spec.ingredients {
                if ing.amount < 1 {
                    return Err(format!(
                        "{}the recipe {} has an ingredient amount below 1, which the engine refuses",
                        at, r.name
                    ));
                }
                if ing.amount > MAX_EXACT_INT {
                    return Err(format!(
                        "{}the recipe {} declares an ingredient amount a Lua double cannot hold exactly: {}",
                        at, r.name, ing.amount
                    ));
                }
                if ing.candidates.is_empty() && !self.valid_item(ing.item) {
                    return Err(format!(
                        "{}the recipe {} names an ingredient item that this plan never declared",
                        at, r.name
                    ));
                }
            }
        }

        for (i, t) in self.techs.iter().enumerate() {
            if t.name.is_empty() {
                return Err(format!(
                    "{}a technology was declared with an empty name",
                    at
                ));
            }
            for other in self.techs.iter().take(i) {
                if other.name == t.name {
                    return Err(format!(
                        "{}two technologies share the name {}; the second would overwrite the first",
                        at, t.name
                    ));
                }
            }
            if w.tech_exists(&format!("{}{}", prefix, t.name)) {
                return Err(format!(
                    "{}the technology {}{} already exists in data.raw; this plan would overwrite it",
                    at, prefix, t.name
                ));
            }
            let has_cost = !t.spec.cost_of.is_empty();
            let has_unit = t.spec.unit.is_some();
            if has_cost == has_unit {
                return Err(format!(
                    "{}the technology {} must name exactly one of CostOf or Unit",
                    at, t.name
                ));
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
            match &t.spec.unit {
                Some(unit) => {
                    if unit.count < 1 {
                        return Err(format!(
                            "{}the technology {} has a unit count below 1, which the engine refuses",
                            at, t.name
                        ));
                    }
                    if unit.count > MAX_EXACT_INT {
                        return Err(format!(
                            "{}the technology {} declares a unit count a Lua double cannot hold exactly: {}",
                            at, t.name, unit.count
                        ));
                    }
                    if !finite(unit.seconds) {
                        return Err(format!(
                            "{}the technology {} declares a research time that is not a finite number",
                            at, t.name
                        ));
                    }
                    if unit.seconds <= 0.0 {
                        return Err(format!(
                            "{}the technology {} has a research time at or below zero, which the engine refuses",
                            at, t.name
                        ));
                    }
                    for p in &unit.packs {
                        if p.amount < 1 {
                            return Err(format!(
                                "{}the technology {} has a science pack amount below 1, which the engine refuses",
                                at, t.name
                            ));
                        }
                        if p.amount > MAX_EXACT_INT {
                            return Err(format!(
                                "{}the technology {} declares a science pack amount a Lua double cannot hold exactly: {}",
                                at, t.name, p.amount
                            ));
                        }
                        if p.name.is_empty() {
                            return Err(format!(
                                "{}the technology {} prices itself in a pack with an empty name",
                                at, t.name
                            ));
                        }
                        if !w.item_exists(&p.name) {
                            return Err(format!(
                                "{}the technology {} prices itself in {}, which does not exist",
                                at, t.name, p.name
                            ));
                        }
                    }
                }
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
            let mut list = Vec::with_capacity(r.spec.ingredients.len());
            for ing in &r.spec.ingredients {
                if ing.candidates.is_empty() {
                    list.push(ResolvedIngredient {
                        name: format!("{}{}", prefix, self.items[ing.item.index - 1].name),
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
                        r.name,
                        join_names(&ing.candidates, ", ")
                    )),
                }
            }
            res.recipes.push(list);
        }

        for t in &self.techs {
            let mut rt = ResolvedTech::default();

            if t.spec.enabled_by.index != 0 {
                let s = &self.settings[t.spec.enabled_by.index - 1];
                let full = format!("{}{}", prefix, s.name);
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

            let after = t.spec.after.as_str();
            let before = t.spec.before.as_str();
            let new_name = format!("{}{}", prefix, t.name);
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
                rt.prereqs = vec![format!(
                    "{}{}",
                    prefix,
                    self.techs[t.spec.after_tech.index - 1].name
                )];
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
    pub(crate) has_enabled_by: bool,
    pub(crate) on: bool,
    /// A 1-based index into `Resolution::rewrites`; zero is none.
    pub(crate) rewrite: usize,
}

#[derive(Default)]
pub(crate) struct Resolution {
    pub(crate) logs: Vec<String>,
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
        kv("name", Value::Str(format!("{}{}", prefix, it.name))),
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
    Value::Map(pairs)
}

fn recipe_proto(
    prefix: &str,
    l: &Lib,
    r: &RecipeDecl,
    ings: &[ResolvedIngredient],
    unlocked: bool,
) -> Value {
    let mut pairs = vec![
        kv("type", Value::string("recipe")),
        kv("name", Value::Str(format!("{}{}", prefix, r.name))),
    ];
    append_localised(&mut pairs, &r.spec.display_name, &r.spec.description);
    if !r.spec.category.is_empty() {
        pairs.push(kv("category", Value::string(&r.spec.category)));
    }
    // energy_required is omitted rather than sent as zero: an absent field is
    // the engine's own default, and a zero is a crafting time of zero.
    if r.spec.craft_time > 0.0 {
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
    pairs.push(kv(
        "results",
        Value::Arr(vec![Value::Map(vec![
            kv("type", Value::string("item")),
            kv(
                "name",
                Value::Str(format!("{}{}", prefix, l.items[r.result.index - 1].name)),
            ),
            kv("amount", Value::Num(count as f64)),
        ])]),
    ));
    Value::Map(pairs)
}

fn tech_proto(prefix: &str, l: &Lib, w: &dyn World, t: &TechDecl, rt: &ResolvedTech) -> Value {
    let mut pairs = vec![
        kv("type", Value::string("technology")),
        kv("name", Value::Str(format!("{}{}", prefix, t.name))),
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
    pairs.push(kv("unit", tech_unit(w, t)));
    // max_level lives on the TECHNOLOGY, not in its unit, so copying the unit
    // verbatim carries a count_formula but leaves the level cap behind.
    // cost_of is one named point for cost AND position, so it reads the cap
    // too: an infinite source technology produces an infinite copy. A
    // hand-rolled UnitSpec has no source to read, and gets no cap.
    if t.spec.unit.is_none() {
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
                    Value::Str(format!("{}{}", prefix, l.recipes[u.index - 1].name)),
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
    Value::Map(pairs)
}

fn tech_unit(w: &dyn World, t: &TechDecl) -> Value {
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
