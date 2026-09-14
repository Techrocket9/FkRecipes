use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::cycle::render_cycle_path;
use crate::ingredient_list::{IngredientList, ListEntry, ListKind, ListText, DEFAULT};
use crate::op::{path_key, Op};
use crate::plan::{
    Amount, CostChoice, CustomCost, Ingredient, IngredientChoice, ItemDecl, Lib, Pack, RecipeDecl,
    SettingDecl, TechDecl, UnitSpec,
};
use crate::settings::{localised_group, named_cost_sources};
use crate::value::{
    finite, kv, localised, localised_chunks, str_arr, Value, CRAFT_TIME_FLOOR, MAX_EXACT_INT,
    MAX_FLUID_AMOUNT, MAX_ITEM_AMOUNT,
};
use crate::world::World;

/// Which rule a number is held to, named because the pair below is two of them
/// and clippy calls the tuple complex otherwise.
pub(crate) type FaultFn = fn(f64) -> NumberFault;

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
    &CostTier,
    NoteTarget,
) -> Option<Value>;

/// What a technology's `cost_by` dropdown settled on, handed to a custom cost
/// so the fields the player left at their default can come from it.
///
/// `None` IS NO TIER AT ALL, which is [`crate::TechSpec::cost_from`] on its
/// own: there is nothing to defer to, so the three settings are the whole price
/// and the cost is built fresh from them exactly as it always was.
pub(crate) enum CostTier {
    None,
    Chosen {
        /// The tier's own unit map, whatever it holds: a count_formula, a
        /// max_level, fields no version of this library knows about.
        /// Overriding a field rather than rebuilding the map is what keeps
        /// every one of them.
        unit: Value,
        dropdown: String,
        chosen: String,
        /// The technology the tier settled on, and it rides here for ONE
        /// reason: the tier arm may already have said, IN THE LOG, that the
        /// source named no science pack this game has and that the mod's
        /// declared cost applies instead. A typed pack list makes that false,
        /// and taking the line back needs the name it was composed from.
        source: String,
        /// The technology's note slot AS IT STOOD BEFORE THE TIER WAS PRICED,
        /// and it is what a typed pack list puts back instead of retracting the
        /// tier's sentences one at a time. It is the TOOLTIP half of what
        /// `source` used to do, and it covers what naming sentences cannot: see
        /// [`Resolution::note_at`].
        note: String,
    },
}

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
        let mut res = self.resolve(w, &prefix);
        // THE CARRIED REFUSAL, FIRST. Resolution asks the World questions and
        // mostly degrades; the answers it cannot degrade are a stored value
        // that is not one of a dropdown's values (which the engine resets
        // before any stage runs, so only a hand-edited file reaches it) and the
        // two cost-number sentences about a DECLARED default the settings stage
        // would already have refused. Each is carried out of the pass rather
        // than raised inside it, because resolution answers questions and this
        // is where a plan is refused; the FIRST one found wins, and it wins over
        // every refusal below because it is the earliest thing the pass met. The
        // merged amounts that used to be here are CLAMPED now, with a line and a
        // tooltip note each, and the copied research unit whose pack list is in
        // neither engine form now degrades with a line and a note of its own:
        // see `merge_ingredient` and `filter_copied_packs`.
        //
        // NOTHING A PLAYER TYPES REACHES THIS LINE ANY MORE. A refused
        // ingredient list, a setting holding something that is not text and a
        // research number the engine would not take all fall back to the
        // author's own declaration with one log line each. See
        // `player_fallback`.
        //
        // WHICH IS NOT THE SAME AS "A PLAYER CANNOT BE HERE". The declaration a
        // fallback lands on can fail a check further down, in a modpack where
        // the author's own packs are all absent or the author's own ladders
        // collapse onto one item; a player who never typed hits that refusal
        // too.
        //
        // SO EVERY REFUSAL BELOW THIS LINE LEAVES THROUGH `fallback_fact`,
        // which states the FACT and names no screen. The sentence that used to
        // be appended here routed the player to Settings > Mod settings >
        // Startup, and the client cannot get there from an "Error loading
        // mods" dialog: re-measured on 2.0.77, the dialog offers Disable listed
        // mods, Disable all mods, Manage mods, Restart, Exit and a Reset mod
        // settings checkbox; Manage mods has no Mod settings button and its
        // Back returns to the same dialog; Restart relaunches into an identical
        // dialog over a file whose sha256 has not moved. What was WRONG was the
        // route, not the fact: a player who typed something is still owed the
        // knowledge that it was set aside, because the accumulated log ops
        // never reach the host on a refused load. Every refusal left names what
        // the AUTHOR must change, and that one added sentence names what the
        // PLAYER's value did, without advice.
        if let Some(message) = &res.refusal {
            return Err(res.fallback_fact(message.clone()));
        }
        self.check_resolved_craft_times(&res)
            .map_err(|m| res.fallback_fact(m))?;
        // THE CYCLE WALK TAKES THE RESOLUTION MUTABLY because it RESOLVES
        // rather than checks: it drops the edges this plan made until the ring
        // is gone, and the dropped prerequisites, the dropped splices, the
        // lines and the notes it writes are all read by the emit walk below. It
        // is written out rather than chained through `map_err` so the mutable
        // borrow ends before `fallback_fact` reads the resolution again.
        if let Err(m) = self.check_cycles(w, &mut res, &prefix) {
            return Err(res.fallback_fact(m));
        }

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
                &res.recipe_notes[i],
            )));
        }
        for (i, t) in self.techs.iter().enumerate() {
            ops.push(Op::Extend(tech_proto(
                &prefix,
                self,
                w,
                t,
                &res.techs[i],
                &res.tech_notes[i],
            )));
        }
        for rt in &res.techs {
            if rt.rewrite == 0 {
                continue;
            }
            let rw = &res.rewrites[rt.rewrite - 1];
            // A SPLICE THE CYCLE WALK DROPPED IS SKIPPED WHOLE, and skipped
            // rather than written back with the name taken out: what is left in
            // the record is another mod's own prerequisite list, and Setting
            // that back over its prototype is a write this library has no
            // reason to make. See `check_cycles`, which is what marks it.
            if rw.dropped {
                continue;
            }
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
        // loops because a declaration that does not line up with its dropdown
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
                // THE CHOICES COVER THE ALLOWED VALUES EXACTLY, with nothing
                // subtracted: this library adds no value of its own to a
                // dropdown, so every value the author declared needs a plan
                // behind it.
                let allowed = setting.values.clone();
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
                    self.validate_no_duplicates(
                        at,
                        &format!("the recipe {}", r.name),
                        prefix,
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
            self.validate_no_duplicates(
                at,
                &format!("the recipe {}", r.name),
                prefix,
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
            let has_cost_by = t.spec.cost_by.is_some();
            // `cost_by` AND `cost_from` ARE ONE COST, which is why this asks
            // `named_cost_sources` rather than counting the four fields: the
            // dropdown is the tier and the three settings overwrite it field by
            // field. Every other pairing is still two sources and still
            // refused.
            if named_cost_sources(&t.spec) != 1 {
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
                let allowed = setting.values.clone();
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
                None if has_cost_by || t.spec.cost_from.is_some() => {}
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
        // THE NOTE SLOTS ARE SIZED FROM THE DECLARATIONS, before the walk, so
        // every index the walk hands to `note_on` is in range by construction
        // and the emit loop can read one per prototype without asking whether
        // it exists.
        let mut res = Resolution {
            recipe_notes: vec![String::new(); self.recipes.len()],
            tech_notes: vec![String::new(); self.techs.len()],
            ..Default::default()
        };

        // THE PLAN'S OWN ITEMS, BEFORE ANY TEXT IS PARSED. See [`PlanItems`]:
        // the names this plan is about to emit are the ones its own setting
        // descriptions show the player, so the language has to know them.
        let own = self.plan_items(w, prefix);

        for (ri, r) in self.recipes.iter().enumerate() {
            let tgt = NoteTarget {
                tech: false,
                index: ri,
            };
            // The crafting time first, then the ingredients: a recipe's own
            // field before what it is made of, mirroring a technology's
            // enablement before its tree placement.
            let mut ct = CraftTime::default();
            if r.spec.craft_time_from.index != 0 {
                let setting = &self.settings[r.spec.craft_time_from.index - 1];
                let full = setting.emitted_name(prefix);
                ct.bound = true;
                ct.value = setting.def_num;
                // A CRAFTING TIME IS A FIELD THE PLAYER OWNS, so a value the
                // engine would not take falls back to the setting's declared
                // default with one line rather than stopping the load. See
                // `player_fallback`: the generated minimum keeps a player from
                // typing one of these, but a second mod declaring the same
                // setting name can hand one over, and that is not something to
                // lock a player out of their save for.
                //
                // ONLY A VALUE THE SETTING ANSWERED CAN FALL BACK. An
                // unreadable setting was never HOLDING anything, so its
                // declared default is an author bug that
                // `check_resolved_craft_times` names as one rather than a
                // stored value this line can tell a player to go and correct.
                //
                // TWO RECIPES MAY READ ONE SETTING, which nothing else here
                // can do, and that is why the line goes through
                // `note_fallback`: one bad field on the settings screen is one
                // problem, so it says so once however many recipes bind it,
                // naming the FIRST recipe in declaration order.
                match w.startup_setting(&full) {
                    Some(Value::Num(n)) => {
                        ct.value = n;
                        let f = craft_time_fault(n);
                        if f != NumberFault::None {
                            res.note_fallback(
                                tgt,
                                &full,
                                number_fallback(&stored_craft_time_problem(&r.name, &full, f)),
                                // A CRAFTING TIME DESTROYS NOTHING: the
                                // ingredient list this recipe emits is byte for
                                // byte what it would have been and only
                                // `energy_required` moves, so the note carries
                                // no recipe-change sentence. See
                                // `Resolution::note_on`.
                                false,
                            );
                            ct.value = setting.def_num;
                        }
                    }
                    _ => res.logs.push(format!(
                        "fkrecipes: the setting {} was not readable, so its default applies",
                        full
                    )),
                }
                ct.setting = full;
            }
            res.craft_times.push(ct);

            // The product is read ONCE per recipe, out of the same helper the
            // prototype builder reads it out of, and handed to every arm below:
            // four of them add a resolved list and each would otherwise have to
            // remember to compute it. See `Resolution::add_recipe`.
            let product = recipe_product(prefix, self, r);

            // THE TEXT IS THE SWITCH, so it is read first and its answer
            // decides whether anything else is consulted at all. A text that
            // says the reserved word, and a text this library cannot use, both
            // leave the decision exactly where a player who typed nothing left
            // it.
            let mut typed = None;
            let mut text_full = String::new();
            let mut text_default: &[Ingredient] = &r.spec.ingredients;
            if let Some(h) = r.spec.ingredients_from {
                let s = &self.settings[h.index - 1];
                text_full = s.emitted_name(prefix);
                text_default = &s.def_ings;
                typed = self.read_text_ingredients(w, &own, &mut res, r, &text_full, tgt);
            }

            match &r.spec.ingredients_by {
                Some(by) => {
                    let setting = &self.settings[by.setting.index - 1];
                    // READ WHATEVER THE TEXT SAID, because the line that sets
                    // the choice aside has to name it. It is also the read that
                    // refuses a stored value the dropdown does not offer, and a
                    // text in force is no reason to stop asking that question.
                    let chosen = res.read_dropdown(w, setting, prefix);
                    if let Some(list) = typed {
                        let rendered = (self.installed_language().render_list)(&list);
                        res.logs.push(format!(
                            "fkrecipes: {} takes its ingredients from {}: {}; the {} choice {} is set aside",
                            r.emitted_name(prefix),
                            text_full,
                            rendered,
                            setting.emitted_name(prefix),
                            chosen
                        ));
                        res.add_recipe(&r.name, &product, typed_ingredients(&list));
                        continue;
                    }
                    let declared = choice_for(&by.choices, &chosen);
                    let mut list =
                        self.resolve_ingredients(w, &mut res, tgt, prefix, &r.name, declared);
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
                            tgt,
                            prefix,
                            &r.name,
                            choice_for(&by.choices, &setting.def_str),
                        );
                    }
                    res.add_recipe(&r.name, &product, list);
                }
                None => match typed {
                    Some(list) => {
                        let rendered = (self.installed_language().render_list)(&list);
                        res.logs.push(format!(
                            "fkrecipes: {} takes its ingredients from {}: {}",
                            r.emitted_name(prefix),
                            text_full,
                            rendered
                        ));
                        res.add_recipe(&r.name, &product, typed_ingredients(&list));
                    }
                    None => {
                        // Where there is no dropdown, the word `default` means
                        // the SETTING's own declared list rather than the
                        // recipe's `ingredients`, which `validate_bindings`
                        // refused beside it anyway.
                        let declared: Vec<Ingredient> = text_default.to_vec();
                        let list =
                            self.resolve_ingredients(w, &mut res, tgt, prefix, &r.name, &declared);
                        res.add_recipe(&r.name, &product, list);
                    }
                },
            }
        }

        for (ti, t) in self.techs.iter().enumerate() {
            let mut rt = ResolvedTech::default();
            let tgt = NoteTarget {
                tech: true,
                index: ti,
            };

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
                let (packs, tried) = resolve_packs(w, &mut res, tgt, &t.name, &unit.packs);
                if packs.is_empty() {
                    res.packless_at(tgt, &t.name, tried);
                }
                rt.packs = packs;
            }

            // THE COPIED COST, PROBED HERE RATHER THAN COPIED AT EMIT. The unit
            // is taken verbatim, which is what carries a count_formula and
            // everything else this library has never heard of across; what is
            // NOT taken verbatim any more is its science packs, because a pack
            // this game demoted or removed stops the load with a sentence
            // naming neither this mod nor the setting. See
            // [`filter_copied_packs`].
            //
            // THERE IS NOTHING TO FALL BACK ON HERE, WHICH IS THE WHOLE
            // DIFFERENCE FROM A TIER. A `cost_of` source is a bare string with
            // no declared ladder behind it, so a copy that keeps no pack is
            // emitted with an empty ingredient list rather than priced on a
            // cost this library would have to invent. What that costs the
            // player is a free research, and both arms below say so where they
            // look: see [`Resolution::packless_at`].
            if !t.spec.cost_of.is_empty() {
                if let Some(u) = w.tech_unit(&t.spec.cost_of) {
                    let filtered = filter_copied_packs(&mut res, w, &t.name, &t.spec.cost_of, u);
                    if filtered.unreadable {
                        // THE EMPTYING IS THE CALLER'S AND NOT THE FILTER'S,
                        // and the reason is that the two callers of the filter
                        // do different things with the same answer: the tier
                        // arm below throws this unit away and prices the
                        // technology on the author's declared cost, so a unit
                        // emptied inside the filter would be built there and
                        // dropped. The filter answers what it found; what to do
                        // about it is the caller's, which is also why
                        // `out.unit` means one thing (the unit as it arrived)
                        // on every path that does not read `out.kept`.
                        //
                        // NOTHING IS INVENTED BY THE EMPTYING. The count, the
                        // time, a count_formula and every field this library
                        // has never heard of cross untouched; only the list it
                        // could not decode is replaced, and it is replaced by
                        // the empty list rather than by a guess at what was in
                        // it.
                        //
                        // THE NOTE AND THE LINE ARE BOTH THIS ARM'S OWN,
                        // because this arm knows something `packless_at`'s pair
                        // does not say and does NOT know something it does. The
                        // outcome is the same emptied unit, but the REASON is
                        // that the copied list could not be read at all: no
                        // pack was ever put to the game, so "this game has none
                        // of the science packs this research names" would be a
                        // fact this walk never established. See
                        // [`unreadable_copy_note`], which draws the same line
                        // [`unreadable_source_note`] draws for the arm above.
                        res.logs
                            .push(unreadable_copy_line(&t.name, &t.spec.cost_of));
                        res.note_on(tgt, unreadable_copy_note(&t.spec.cost_of));
                        rt.unit = Some(set_unit_field(
                            filtered.unit,
                            "ingredients",
                            Value::Arr(Vec::new()),
                        ));
                    } else if filtered.has_list
                        && filtered.kept == 0
                        && !filtered.dropped.is_empty()
                    {
                        res.packless_at(tgt, &t.name, filtered.dropped.clone());
                        rt.unit = Some(filtered.unit);
                    } else {
                        if let Some(first) = filtered.dropped.first() {
                            res.note_on(tgt, pack_dropped_note(first));
                        }
                        rt.unit = Some(filtered.unit);
                    }
                }
            }

            // THE PLAYER'S OWN UNIT, in the same place a hand-rolled one is
            // resolved: what the technology COSTS before where it sits. With no
            // tier the answer is always the custom cost, because the three
            // settings are the whole price.
            if t.spec.cost_by.is_none() {
                if let Some(cc) = &t.spec.cost_from {
                    if let Some(unit) = (self.installed_custom_cost())(
                        self,
                        w,
                        &own,
                        &mut res,
                        prefix,
                        t,
                        cc,
                        &CostTier::None,
                        tgt,
                    ) {
                        rt.unit = Some(unit);
                    }
                }
            }

            if let Some(by) = &t.spec.cost_by {
                let setting = &self.settings[by.setting.index - 1];
                let chosen = res.read_dropdown(w, setting, prefix);
                // THE NOTE SLOT AS IT STANDS BEFORE ANY PRICING, taken here so
                // a player's typed pack list can put it back in one call instead
                // of retracting the tier arm's sentences by name. Nothing below
                // the dropdown read has written a note yet, so this is the last
                // point at which the slot is still whatever the walk brought in.
                // See [`Resolution::note_at`].
                let note_before = res.note_at(tgt);
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
                    // THE FALLBACK IS RESOLVED IN TWO ARMS AND THIS IS THE
                    // FIRST, which is the whole point of doing it at
                    // resolution: a fallback nobody reaches asks the game
                    // nothing and so can refuse nothing. The SECOND arm is
                    // the packless-source case below, where a source DID
                    // answer and then lost every pack to the tool probe, so
                    // "a source answered" does not end the pack question and
                    // a game with no tool in it at all reaches the fallback
                    // either way. An earlier reading of this comment said
                    // "only here", and a consumer's test built on it went
                    // from green to a refused load; docs/migration.md states
                    // the rule for consumers now.
                    let (packs, tried) =
                        resolve_packs(w, &mut res, tgt, &t.name, &by.fallback.packs);
                    if packs.is_empty() {
                        res.packless_at(tgt, &t.name, tried);
                    }
                    // THE NOTE IS RECORDED AFTER THE FALLBACK IS RESOLVED,
                    // which is how the two arms below do it and is load-bearing
                    // here for the same reason. `note_on` keeps the FIRST note
                    // per prototype, and the fallback just walked can leave this
                    // technology with NO SCIENCE PACK AT ALL (`packless_at`) or
                    // CLAMP two of its own ladders landing on one name above the
                    // item ceiling (`merge_pack`). Offering this sentence last
                    // hands the one slot to whichever of those happened.
                    //
                    // WHAT THE ORDERING BUYS IS THE PACKLESS CASE, and that one
                    // is not a trade at all: a research with no science pack
                    // completes for free the moment it is queued, so a player
                    // reading only "no prerequisite and no copied cost" would be
                    // told about the tree and left to discover the price by
                    // watching it finish. The packless sentence is strictly the
                    // more urgent of the two.
                    //
                    // ON THE CLAMP PATH IT IS A TRADE AND THE PLAYER LOSES
                    // SOMETHING. The clamp sentence names an amount capped at a
                    // ceiling and says nothing about tree position, so a
                    // technology that both lost its prerequisite and clamped a
                    // merged pack discloses only the clamp: the ONE sentence
                    // that would have mentioned the tree is the one that is
                    // dropped. That is ACCEPTED here, and the reason is the slot
                    // rather than the ranking. `note_on` keeps one note per
                    // prototype (see the Fix round 3 open list in
                    // agents/implementation-notes.md, where the one-note rule is
                    // recorded as the thing to revisit), so with two
                    // degradations and one slot SOMETHING is lost whichever
                    // order is chosen, and a wrong price is the one a player
                    // acts on: they build for it. A prerequisite they no longer
                    // need is visible in the technology screen the moment they
                    // open it. Widening this to two disclosures is the fix;
                    // reordering it is not.
                    res.note_on(tgt, unpriced_source_note());
                    rt.unit = Some(unit_value(by.fallback.count, by.fallback.seconds, &packs));
                } else {
                    // THE PREREQUISITE MOVES WITH THE UNIT, and it still does
                    // when the player has written over one of the tier's
                    // numbers: the tier is what named a source, and the
                    // settings beside it price the same rung rather than
                    // choosing another one.
                    rt.prereqs = vec![source.clone()];
                    // AND THE COPIED PACKS ARE PROBED, which is what keeps a
                    // mod set from stopping the load on the default setting.
                    // See [`filter_copied_packs`] for the two engine refusals
                    // this replaces.
                    let copied = rt.unit.clone().expect("a chosen source settled on a unit");
                    let filtered = filter_copied_packs(&mut res, w, &t.name, &source, copied);
                    if filtered.unreadable {
                        // A COPIED LIST THIS LIBRARY CANNOT DECODE, AND A
                        // DECLARED COST BEHIND IT: the same degradation the arm
                        // below makes, for a different reason and in its own
                        // words. Nothing was dropped, because nothing was read;
                        // the author's own fallback unit is what the technology
                        // is priced in, and the prerequisite and the level cap
                        // stay because the tier still chose this rung. NOTHING
                        // IS CARRIED INTO THE FALLBACK'S OWN LADDER either:
                        // there are no names to carry when the list was never
                        // decoded.
                        res.logs.push(unreadable_source_line(&t.name, &source));
                        let (packs, tried) =
                            resolve_packs(w, &mut res, tgt, &t.name, &by.fallback.packs);
                        if packs.is_empty() {
                            res.packless_at(tgt, &t.name, tried);
                        }
                        res.note_on(tgt, unreadable_source_note(&source));
                        rt.unit = Some(unit_value(by.fallback.count, by.fallback.seconds, &packs));
                    } else if filtered.has_list
                        && filtered.kept == 0
                        && !filtered.dropped.is_empty()
                    {
                        // EVERY PACK GONE, AND THERE IS A DECLARED COST BEHIND
                        // THIS ONE: the author's own fallback unit is what the
                        // technology is priced in, resolved through the same
                        // ladder so its own absent rungs drop the same way. The
                        // prerequisite and the level cap stay, because the tier
                        // still chose this rung; only the price moved.
                        res.logs.push(packless_source_line(&t.name, &source));
                        // THE NOTE IS RECORDED AFTER THE FALLBACK IS RESOLVED,
                        // and the Go half does the same. `note_on` keeps the
                        // FIRST note in walk order, and when the declared
                        // fallback ALSO keeps no pack the honest sentence is
                        // the packless one: "this mod's own declared cost
                        // applies" is true and says nothing about the research
                        // being free. Nothing else competes for the slot on
                        // this path, because a unit that kept no pack merged
                        // none and so clamped none.
                        let (packs, tried) =
                            resolve_packs(w, &mut res, tgt, &t.name, &by.fallback.packs);
                        if packs.is_empty() {
                            // EVERY RUNG THE WALK ASKED ABOUT, COPIED PACK
                            // FIRST. The science packs this tier's copied unit
                            // named and lost were asked before the fallback's
                            // own ladders were, and an author reading the line
                            // is one whose ladders all missed: the
                            // whole list of names, in the order they were
                            // asked, is the one thing that says which mod set
                            // this is. ONCE EACH is `packless_at`'s rule and
                            // not this caller's: see there.
                            let mut asked = filtered.dropped.clone();
                            asked.extend(tried);
                            res.packless_at(tgt, &t.name, asked);
                        }
                        res.note_on(tgt, packless_source_note(&source));
                        rt.unit = Some(unit_value(by.fallback.count, by.fallback.seconds, &packs));
                    } else {
                        // A PACK DROPPED OUT OF A PRICE THE PLAYER CANNOT SEE
                        // gets the note, and the FIRST one does: the tooltip
                        // says one thing, and the log has the rest. A unit that
                        // kept everything says nothing at all.
                        if let Some(first) = filtered.dropped.first() {
                            res.note_on(tgt, pack_dropped_note(first));
                        }
                        rt.unit = Some(filtered.unit);
                    }
                }
                // THE THREE SETTINGS OVER THE TIER, where the technology
                // declares them.
                if let Some(cc) = &t.spec.cost_from {
                    let tier = CostTier::Chosen {
                        unit: rt
                            .unit
                            .clone()
                            .expect("a CostBy technology settled on a unit"),
                        dropdown: setting.emitted_name(prefix),
                        chosen: chosen.clone(),
                        source: source.clone(),
                        note: note_before.clone(),
                    };
                    if let Some(unit) = (self.installed_custom_cost())(
                        self, w, &own, &mut res, prefix, t, cc, &tier, tgt,
                    ) {
                        rt.unit = Some(unit);
                    }
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
                // WHAT THE SPLICE REPLACED, recorded beside what it produced:
                // the anchor where one was taken out of the list, and the empty
                // string where the name was appended and nothing was. See
                // [`RewriteRec`].
                let mut anchor = String::from(after);
                if !replaced {
                    anchor = String::new();
                    list.push(new_name.clone());
                    res.logs.push(format!(
                        "fkrecipes: {}: {} does not require {}, so the new technology is appended to its prerequisites",
                        t.name, before, after
                    ));
                }
                res.rewrites.push(RewriteRec {
                    before: String::from(before),
                    list,
                    anchor,
                    dropped: false,
                });
                rt.rewrite = res.rewrites.len();
            }

            res.techs.push(rt);
        }
        res
    }

    /// The post-condition on every bound crafting time, and the AUTHOR-SIDE
    /// twin of the two cost-number checks.
    ///
    /// A VALUE THE PLAYER'S SETTING ANSWERED HAS ALREADY FALLEN BACK by the
    /// time this runs: `resolve` holds it to the same two rules where it is
    /// read and answers with the setting's declared default when it fails one,
    /// logging a line. So the only world left for this loop is a DECLARED
    /// default the engine would not take, which `validate_settings` refuses at
    /// the settings stage (a bound setting's minimum has to clear the floor and
    /// its default has to clear the minimum). The engine runs that stage before
    /// the data stage, so this answers only for a host test that calls
    /// `plan_data` on its own, and it stays because what it protects is the
    /// invariant that no recipe this library emits carries an `energy_required`
    /// the engine refuses.
    ///
    /// THE TWO RULES ARE THE FALLBACK'S, THE TWO SENTENCES ARE NOT.
    /// `craft_time_fault` answers the same two questions in the same order on
    /// both sides, so the sides cannot drift about what is wrong; the wording
    /// says DECLARED DEFAULT here, because that is the number this loop is
    /// looking at. The stored value it replaced is gone, and a sentence saying
    /// the setting "answers" this would be describing a value nothing is
    /// holding any more.
    fn check_resolved_craft_times(&self, res: &Resolution) -> Result<(), String> {
        for (i, ct) in res.craft_times.iter().enumerate() {
            if !ct.bound {
                continue;
            }
            let f = craft_time_fault(ct.value);
            if f != NumberFault::None {
                return Err(declared_craft_time_problem(
                    &self.recipes[i].name,
                    &ct.setting,
                    f,
                ));
            }
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
    /// WHAT THE SPLICE REPLACED: the technology whose place in `before`'s
    /// prerequisite list the new name took, or the empty string where there was
    /// nothing to replace and the name was appended.
    ///
    /// IT IS WHAT MAKES A DROPPED SPLICE UNDOABLE, and without it the drop
    /// destroys an edge of somebody else's tree. A splice REPLACES its anchor,
    /// so a SECOND splice into the same technology builds its record on a list
    /// the anchor is already out of; taking the dropped name back out of that
    /// later record without putting the anchor back leaves the later record
    /// emitting a prerequisite list with a base-game edge silently missing from
    /// it. See `drop_splice`, which is the one reader.
    pub(crate) anchor: String,
    /// A splice the cycle walk took back because it closed a ring. It is
    /// MARKED RATHER THAN REMOVED because `ResolvedTech::rewrite` is a 1-based
    /// index into this vector and renumbering it would point every later
    /// technology at somebody else's record.
    pub(crate) dropped: bool,
}

#[derive(Default)]
pub(crate) struct ResolvedTech {
    pub(crate) prereqs: Vec<String>,
    /// The cost this technology settled on, and the level cap that rode along
    /// with it.
    ///
    /// `unit` is Some for every cost shape that RESOLVED one, which is now all
    /// four: `cost_of` copies its source's unit verbatim and then filters the
    /// science packs out of it, so the unit it settles on is a fact about this
    /// game rather than a value the emit layer can read back for itself. It is
    /// None only where the technology named no cost at all, and for a
    /// `cost_of` whose source carries no unit, which `validate` has already
    /// refused. `max_level` is still a CostBy answer alone: a `cost_of` reads its
    /// source's cap at emit, and a price this plan wrote has no source
    /// technology to read one from.
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

/// The prototype whose emitted `localised_description` carries the trailing
/// line a fallback owes the player, THREADED FROM THE WALK rather than derived
/// from anything the callee can see.
///
/// A DERIVED ONE WOULD BE WRONG IN BOTH DIRECTIONS. The index cannot be read
/// off `res.recipes.len()`, because a recipe's list is handed over at the END
/// of its arm and every read that can fall back happens before it; and it
/// cannot be a cursor the walk sets, because a cursor left stale by one arm
/// silently writes a note onto the previous declaration. The parameter is what
/// makes a caller that forgot it a compile error.
///
/// A RECIPE IS ALSO WHAT SAYS WHICH SENTENCES THE NOTE CARRIES. See
/// [`fallback_note`]: a recipe's note names the engine's input-slot cost and a
/// technology's does not, and the text setting a recipe target reaches is
/// always its ingredient list.
#[derive(Clone, Copy)]
pub(crate) struct NoteTarget {
    pub(crate) tech: bool,
    pub(crate) index: usize,
}

#[derive(Default)]
pub(crate) struct Resolution {
    pub(crate) logs: Vec<String>,
    pub(crate) craft_times: Vec<CraftTime>,
    pub(crate) recipes: Vec<Vec<ResolvedIngredient>>,
    pub(crate) techs: Vec<ResolvedTech>,
    pub(crate) rewrites: Vec<RewriteRec>,

    /// The trailing line each emitted prototype's description carries, indexed
    /// by DECLARATION ORDER rather than by the order the walk filled them, and
    /// empty for a declaration nothing fell back on.
    ///
    /// ONE NOTE PER PROTOTYPE, THE FIRST IN WALK ORDER, so the sentence a
    /// player hovers is the same string every run. Two settings bound to one
    /// recipe can both fall back in one load; the second adds nothing, exactly
    /// as the refusal's added sentence names only the first.
    ///
    /// A VECTOR SIZED UP FRONT, not a map keyed by name: the emit loop reads it
    /// by the same index it reads `recipes` and `craft_times` by, and a map
    /// would be an iteration order this library does not allow anywhere.
    pub(crate) recipe_notes: Vec<String>,
    pub(crate) tech_notes: Vec<String>,
    /// The FIRST answer resolution could not degrade. The producers left are a
    /// stored dropdown value the setting does not offer (the engine resets one
    /// before any stage runs, so only a hand-edited file reaches it) and the two
    /// cost-number answers about a declared default the settings stage would
    /// already have refused. The merged amounts that used to be here are CLAMPED
    /// now, with a line and a tooltip note each, and the copied research unit
    /// whose pack list is in neither engine form now degrades with a line and a
    /// note of its own: see `merge_ingredient` and `filter_copied_packs`.
    /// Carried rather than returned so the pass stays one shape, and
    /// `plan_data` raises this before it reads any of it.
    pub(crate) refusal: Option<String>,

    /// For every technology that went packless, the exact ERROR line it logged.
    /// It exists for the ONE retraction there is: a player who types a pack
    /// list over a tier writes over the very unit that went packless, and the
    /// line and the note it earned a moment ago have to go with it. The line is
    /// composed from names the retracting site never saw, so it is kept rather
    /// than recomposed.
    ///
    /// IT IS NO LONGER A REFUSAL CARRIER. A technology with no science pack the
    /// game has is EMITTED with an empty ingredient list now, because the error
    /// dialog a refusal raises cannot reach the Mod Settings screen (measured
    /// on 2.0.77: see [`Resolution::fallback_fact`]) and a mod set the player
    /// did not choose must not be able to lock them out. What the emptied unit
    /// costs them is a free research, which is disclosed in the technology's
    /// own tooltip and in the log: see [`Resolution::packless_at`].
    pub(crate) packless_said: Vec<PacklessRec>,

    /// Every setting whose STORED value the library could not use, in walk
    /// order and ONCE EACH.
    ///
    /// IT HAS TWO READERS. It is the DEDUPE: a crafting-time setting two
    /// recipes read is one field on the settings screen, so a bad value there
    /// is one problem and gets one line however many declarations reach it, and
    /// this vector is what a second reader is checked against. And it is what
    /// [`Resolution::fallback_fact`] states, because the first element is the
    /// setting a refusal raised after resolution names.
    ///
    /// A VECTOR AND A LINEAR SCAN RATHER THAN A SET: the order is the answer, a
    /// set would not have one, and no plan declares enough settings for the
    /// scan to be worth a second data structure.
    pub(crate) fell_back: Vec<String>,
}

impl Resolution {
    /// Records a refusal, keeping the first: resolution runs in declaration
    /// order, so the first is the one a reader would have met.
    fn refuse(&mut self, message: String) {
        if self.refusal.is_none() {
            self.refusal = Some(message);
        }
    }

    /// Records one recipe's resolved ingredient list, and it is the ONLY place
    /// that writes `recipes`: a source property in `tests::source` refuses a
    /// bare push anywhere else.
    ///
    /// FOUR ARMS REACH IT AND A FIFTH WOULD. `resolve` answers a recipe's
    /// ingredients through a text that takes a dropdown's choice over, through
    /// a dropdown on a preset, through a text with no dropdown beside it and
    /// through a plain declared list; a
    /// check written into any one of them would be missing from the other
    /// three, and from whichever arm is added next. So the check lives here,
    /// where the list is handed over.
    ///
    /// THE COMPILER DOES NOT ENFORCE THAT AND A TEST DOES. A fifth arm that
    /// pushed onto `recipes` itself compiles, plans, and is merely silent. The
    /// only arm the language itself catches is one that pushes NOTHING, and it
    /// catches it at run time rather than at compile time: `plan_data` indexes
    /// `res.recipes[i]` against `self.recipes` and a short vector panics
    /// there. So the guard against a fifth arm going round this hand-over is
    /// `tests::source::only_the_hand_over_writes_the_resolved_recipes`.
    ///
    /// THE POSITION IN THE LOG STREAM IS WHAT THE HAND-OVER POINT BUYS. The
    /// line is evaluated on the FINAL list, so it has to come after every merge
    /// and drop line this recipe wrote, and it has to come before the next
    /// recipe's first line. Pushing here is exactly that, in every arm, without
    /// any arm knowing it.
    ///
    /// SUBJECT IS THE DECLARED NAME, product the EMITTED one. Every other line
    /// about a recipe's list names the recipe as the author declared it (see
    /// `merged_opening`), and the product is a prototype name the game will
    /// hold, so it is compared against resolved ingredient names, which are
    /// emitted too.
    fn add_recipe(&mut self, subject: &str, product: &str, list: Vec<ResolvedIngredient>) {
        // EMPTY IS A RECIPE THAT DECLARES NEITHER SHAPE, which `validate`
        // refuses before resolution runs. The guard is here so this function
        // answers for the whole domain of its argument rather than for the
        // domain some caller upstream happens to hold to.
        if !product.is_empty() {
            for ing in &list {
                // THE KIND IS PART OF THE IDENTITY, as it is in
                // `merge_ingredient`, where an item and a fluid of one name
                // fall through the exhaustive match and stay two entries. A
                // product is always an item (`recipe_proto` writes type="item"
                // and nothing chooses otherwise), and an item and a fluid of
                // one name genuinely coexist: base carries parameter-0 to
                // parameter-9 as both. A fluid ingredient sharing the name is
                // a different thing with the same label and says nothing.
                //
                // POSITIVE, AND NOT `is_fluid`, so it reads as the Go twin's
                // `ing.kind != kindItem` does and so a third `Amount` variant
                // has to be visited here rather than quietly joining the
                // items.
                if !matches!(ing.amount, Amount::Item(_)) || ing.name != product {
                    continue;
                }
                self.logs.push(self_product_line(subject, product));
                // AND THERE IS NO EARLY EXIT, DELIBERATELY. At most one entry
                // can match already, by two independent guards: a declared
                // list that a ladder collapsed is kept unique by
                // `merge_ingredient`, and a list the player typed by the
                // language's own "entries N and M both name" refusal, which
                // `resolve_text_ingredients` relies on because it collects its
                // entries without merging them. Not stopping early is what
                // gives the kind test above a WITNESS: a list holding an item
                // and a fluid of one name writes two lines the moment the kind
                // is dropped from the comparison, and a `break` would hide
                // that. If both guards ever failed, this line appearing twice
                // is a louder symptom than a quieter one.
            }
        }
        self.recipes.push(list);
    }

    /// Records one player-controlled setting falling back: the note the
    /// prototype's own description will carry, and the log line, which is
    /// written once per SETTING however many declarations read it.
    ///
    /// THE NOTE COMES FIRST AND IS NOT DEDUPED BY SETTING, and the two rules
    /// are different on purpose. One bad field on the settings screen is one
    /// problem and gets one line; but two recipes bound to one crafting-time
    /// setting are two tooltips, and a player hovering the second one is owed
    /// the same sentence as the first. So the dedupe below guards the line and
    /// not the note.
    ///
    /// `destroys_inputs` is the caller's answer to "does this fallback change
    /// what the recipe is made of", and it is passed rather than derived: see
    /// [`Resolution::note_on`].
    fn note_fallback(
        &mut self,
        tgt: NoteTarget,
        setting: &str,
        line: String,
        destroys_inputs: bool,
    ) {
        self.note_on(tgt, fallback_note(setting, destroys_inputs));
        if self.fell_back.iter().any(|seen| seen == setting) {
            return;
        }
        self.fell_back.push(String::from(setting));
        self.logs.push(line);
    }

    /// Adds the ONE sentence a refusal owes a player whose stored value was set
    /// aside on the way to it, and it exists because the log ops never reach
    /// the host on a refused load.
    ///
    /// THE OPS ARE LOST, WHICH IS THE WHOLE REASON. `resolve` accumulates its
    /// lines into `logs` and `plan_data` turns them into `Op`s only AFTER every
    /// check has passed, so a plan that refuses hands the host a message and
    /// nothing else: the player would read a refusal about the mod's own
    /// declaration with no hint that the field they edited was set aside at
    /// all.
    ///
    /// IT IS A FACT AND NOT A ROUTE, which is the whole of what changed. The
    /// sentence used to end by sending the player to Settings > Mod settings >
    /// Startup, and the client cannot get there from an "Error loading mods"
    /// dialog: see [`Lib::plan_data`] for the walk that measured it. What the
    /// dialog cannot reach is the SCREEN; the fact that a stored value was set
    /// aside is still true and is still the one thing this message can add. So
    /// the sentence states it and stops there: no screen, no route, no advice.
    ///
    /// IT MAY NOT SAY "THE MOD LOADED" IN ANY FORM, and that is why this
    /// sentence is worded differently from the three the customize layer
    /// writes: this one decorates a REFUSAL, so nothing loaded. What it can say
    /// is what happened to the value, and the honest generic for that is the
    /// library's own rule: a stored value it cannot use behaves exactly as if
    /// the player had left the field alone. It used to say the mod's own
    /// declaration applied, which is false on five of six presets: beside a
    /// dropdown a set-aside text leaves the dropdown's CURRENTLY CHOSEN preset
    /// deciding, and the mod's own declared list is what a field left alone
    /// gives only where no dropdown sits beside it. See [`fallback_note`] for
    /// the same correction on the prototype side.
    ///
    /// THE ADVERBIAL BELONGS TO THE OUTCOME AND NOT TO THE SETTING-ASIDE,
    /// which is the one way this sentence is built differently from its three
    /// siblings. Nothing about the setting-aside is conditional: the value
    /// went, full stop. What "left alone" describes is what then APPLIED, so
    /// the clause hangs off that and the sentence names the two things in the
    /// order they happened.
    ///
    /// IT APPEARS ONLY WHEN A FALLBACK HAPPENED, and it names the FIRST setting
    /// in walk order, so a refusal on a plan nobody typed into carries nothing
    /// and a plan with two fallbacks answers the same way every run.
    fn fallback_fact(&self, message: String) -> String {
        match self.fell_back.first() {
            None => message,
            Some(setting) => format!(
                "{}. The stored value of {} could not be used and was set aside, so what applied is what that field gives when it is left alone.",
                message, setting
            ),
        }
    }

    /// Records the trailing line one prototype's description carries, keeping
    /// the FIRST in walk order so the sentence is the same every run.
    ///
    /// IT TAKES THE WHOLE SENTENCE, not the pieces one composer happens to
    /// need. Two kinds of thing reach it: a stored value the player typed that
    /// the library set aside ([`fallback_note`]), and an ENVIRONMENTAL
    /// degradation nobody typed, where the game is missing something the plan
    /// names ([`pack_dropped_note`] and its two companions). Deciding which is
    /// which here would be a branch on a reason string, which is the shape the
    /// destroyed-inputs correction already refused once.
    ///
    /// THE INDEX IS NEVER CHECKED, deliberately. Both vectors are sized from
    /// the declaration counts at the top of `resolve` and every index comes
    /// from the walk's own loop, so an out-of-range one is this file having
    /// gone wrong rather than anything a consumer can reach, and a panic naming
    /// the line is a better answer than a note silently dropped.
    pub(crate) fn note_on(&mut self, tgt: NoteTarget, note: String) {
        let notes = if tgt.tech {
            &mut self.tech_notes
        } else {
            &mut self.recipe_notes
        };
        if !notes[tgt.index].is_empty() {
            return;
        }
        notes[tgt.index] = note;
    }

    /// `note_at` and [`Resolution::restore_note`] are the SNAPSHOT PAIR, and
    /// they exist for exactly one caller: the tier arm hands
    /// `resolve_custom_cost` the note slot as it stood before the tier was
    /// priced, and a player's typed pack list puts that slot back.
    ///
    /// A SNAPSHOT AND NOT A LIST OF NAMED RETRACTIONS, which is the correction
    /// the adversarial review asked for and which removes code rather than
    /// adding it. Retracting by name can only take back the sentences the
    /// retracting site knows how to compose, and the tier arm can write one it
    /// does not: two of the fallback's own pack ladders landing on one name over
    /// the item ceiling leaves a CLAMP note through `merge_pack`, that note
    /// takes the slot [`Resolution::note_on`] keeps for the first writer, the
    /// named retractions then match nothing, and a technology whose emitted
    /// price is the player's own list with nothing clamped in it carries a
    /// tooltip saying something was capped. The snapshot takes back whatever
    /// the tier arm wrote, and puts back exactly what was there before it.
    ///
    /// THE LOG IS STILL RETRACTED BY NAME, because a log line is a stream and
    /// not a slot: there is nothing to snapshot and put back, and
    /// [`Resolution::retract_log`]'s no-op on a line that was never written is
    /// what makes naming them safe.
    pub(crate) fn note_at(&self, tgt: NoteTarget) -> String {
        if tgt.tech {
            self.tech_notes[tgt.index].clone()
        } else {
            self.recipe_notes[tgt.index].clone()
        }
    }

    pub(crate) fn restore_note(&mut self, tgt: NoteTarget, note: &str) {
        let notes = if tgt.tech {
            &mut self.tech_notes
        } else {
            &mut self.recipe_notes
        };
        notes[tgt.index] = String::from(note);
    }

    /// Takes one accumulated log line back, the FIRST one that is exactly the
    /// given line, so the stream reads as if the walk had never written it.
    ///
    /// THE LINES ARE STILL ONLY ORDERED BY THE WALK. Removing one shifts the
    /// rest up and changes nothing else, which is what keeps the stream
    /// deterministic: the line is composed from the same pieces the writer
    /// used, so a line that was never written matches nothing and the call is a
    /// no-op.
    fn retract_log(&mut self, line: &str) {
        if let Some(i) = self.logs.iter().position(|have| have == line) {
            self.logs.remove(i);
        }
    }

    /// What a technology priced in no science pack at all earns: the unit is
    /// emitted with an empty ingredient list, one ERROR line names every rung
    /// the walk asked the game about, and the technology's own tooltip says
    /// what it costs the player.
    ///
    /// A COST WITH NO PACKS IS NOT A CHEAP RESEARCH, IT IS A FREE ONE, and that
    /// is measured in play rather than only as far as the load. On 2.0.77 build
    /// 84539 a unit of `{count = 10, time = 15, ingredients = {}}` loads with
    /// exit 0 and no engine line, `force.add_research` returns true, the
    /// research queue takes it, progress advances in a lab holding nothing and
    /// the technology COMPLETES after `count * time` ticks. So emitting one is
    /// a balance change the player did not choose, which is exactly why it is
    /// disclosed where they look.
    ///
    /// IT USED TO BE A REFUSAL AND THAT WAS A LOCK-OUT. A mod set that demotes
    /// one science pack could stop the load on the DEFAULT setting, and the
    /// client cannot reach the Mod Settings screen from an "Error loading mods"
    /// dialog (re-measured on 2.0.77: see [`Resolution::fallback_fact`]). A
    /// player whose pack was demoted had no way back into the game that did not
    /// disable the mod. A free research they are told about is worse than the
    /// research they asked for and better than no game, and the threat model
    /// grades it that way.
    ///
    /// IT IS PER TECHNOLOGY, not first-in-plan-order. The old carrier held ONE
    /// technology because a refusal is one sentence; a line and a tooltip are
    /// per prototype, so two packless technologies are two lines and two
    /// tooltips.
    ///
    /// AND THIS IS THE ONE PLACE THE ONCE-EACH RULE IS APPLIED, on the way into
    /// the sentence. It belongs to the single WRITER rather than to any caller
    /// because no caller can see another's list: two declared ladders ending on
    /// one absent rung would print that rung twice, a copied unit naming one
    /// absent pack twice would print it twice, and a fallback rung repeating a
    /// pack the copied unit already lost would print it twice across two
    /// producers. FIRST-SEEN ORDER, because the sentence is the walk's own
    /// order: the copied unit's lost packs first, then the declared ladders in
    /// declaration order with each ladder's rungs in ladder order.
    fn packless_at(&mut self, tgt: NoteTarget, tech: &str, tried: Vec<String>) {
        let mut names: Vec<String> = Vec::with_capacity(tried.len());
        for name in tried {
            append_once(&mut names, &name);
        }
        let line = packless_line(tech, &names);
        self.logs.push(line.clone());
        self.note_on(tgt, packless_note());
        self.packless_said.push(PacklessRec {
            index: tgt.index,
            line,
        });
    }

    /// Takes back the LINE one technology earned from
    /// [`Resolution::packless_at`], for the one caller that can: a player's
    /// typed pack list writing over the very unit that went packless. See
    /// `resolve_custom_cost`.
    ///
    /// THE LINE ONLY, because the note that went with it is taken back by the
    /// snapshot the same caller restores, along with everything else the tier
    /// arm wrote. See [`Resolution::note_at`].
    ///
    /// THE LINE IS READ BACK RATHER THAN RECOMPOSED, because the names in it
    /// were asked by a walk the retracting site never saw.
    fn retract_packless_line(&mut self, tgt: NoteTarget) {
        let Some(i) = self
            .packless_said
            .iter()
            .position(|rec| rec.index == tgt.index)
        else {
            return;
        };
        let rec = self.packless_said.remove(i);
        self.retract_log(&rec.line);
    }
}

/// One technology that went packless and the exact line it logged, so the one
/// site that can take it back has the string the writer used.
///
/// KEYED ON THE DECLARATION INDEX AND NOT ON THE NAME. A declared name is not
/// unique: validate refuses two technologies whose EMITTED names collide, and a
/// legacy declaration keeps its name unprefixed, so a legacy technology and an
/// ordinary one can legally both be called "steel-axes". A retraction matching
/// on the name would take back a line that is still true and belongs to the
/// other one. The index is already threaded to every site through
/// [`NoteTarget`].
pub(crate) struct PacklessRec {
    index: usize,
    line: String,
}

impl Resolution {
    /// The prerequisite list a splice should build on: the one an earlier
    /// splice in this same plan already planned, or the game's own.
    ///
    /// A SPLICE THE CYCLE WALK DROPPED IS NOT THERE ANY MORE, so the walk's
    /// next pass builds its overlay out of what the plan will actually emit.
    /// During resolution nothing is dropped yet and this term costs a
    /// comparison.
    pub(crate) fn current_prereqs(&self, w: &dyn World, tech: &str) -> Vec<String> {
        match self.planned_prereqs(tech) {
            Some(list) => list,
            None => w.tech_prereqs(tech),
        }
    }

    /// `current_prereqs`' PLAN half on its own: the list an earlier splice in
    /// this same plan planned, and `None` where there was none.
    ///
    /// IT IS SPLIT OUT FOR ONE CALLER, `check_cycles`, which asks it once per
    /// technology per pass and must not re-ask the World alongside: the World's
    /// answer is invariant across passes and a host call is not a map lookup.
    /// See the measurement there.
    pub(crate) fn planned_prereqs(&self, tech: &str) -> Option<Vec<String>> {
        for rw in self.rewrites.iter().rev() {
            if rw.dropped {
                continue;
            }
            if rw.before.as_str() == tech {
                // Cloned because the Go mirror clones: an aliased list here is
                // a caller that can rewrite a planned splice through the slice
                // it was handed.
                return Some(rw.list.clone());
            }
        }
        None
    }
}

/// The constant every sentence this library composes begins with, in the
/// language and in this layer alike.
///
/// IT IS A CONSTANT SO IT CAN BE TAKEN BACK OFF. A fallback line carries a
/// refusal's own sentence inside a line that already opens with the prefix, and
/// it is removed with an explicit trim of this constant rather than by counting
/// characters or by cutting at a colon. The corpus asserts the property over
/// every message the language builds and `fallback_sentences_carry_the_prefix`
/// over every one this layer builds, so the trim is total rather than hopeful.
pub(crate) const MESSAGE_PREFIX: &str = "fkrecipes: ";

/// The ONE line a value the PLAYER controls logs when the library cannot use
/// it, and it exists instead of a refusal.
///
/// A VALUE THE PLAYER TYPES NEVER INTRODUCES A REFUSAL A PLAYER WHO TYPED
/// NOTHING WOULD NOT ALSO HAVE HIT; AN INPUT THE AUTHOR DECLARES STILL REFUSES.
/// The claim is that narrow one on purpose: what a fallback lands on is the
/// author's declaration, and a modpack where that declaration cannot produce a
/// legal result stops the load either way. Such a refusal carries the ONE FACT
/// that a stored value was set aside ([`Resolution::fallback_fact`]) and NO
/// route to the settings screen, because the client's error dialog has none:
/// see [`Lib::plan_data`] for the client walk that settled it.
///
/// MEASURED (Factorio 2.0.77, build 84539): the engine
/// rewrites mod-settings.dat on every successful load and on NO failed one
/// (three consecutive failed runs left the file at one sha256), so nothing in a
/// failed run can edit the value that caused it. In the client the refusal is
/// an "Error loading mods" dialog offering Disable listed mods, Disable all
/// mods, Manage mods, Restart, Exit and a Reset mod settings checkbox; Manage
/// mods shows the Mods screen, which offers enable and disable, has no Mod
/// settings button, and whose Back returns to the same dialog rather than to
/// the main menu, so the Mod Settings screen is not reachable. Disabling and
/// re-enabling the mod does not help either: the engine keeps a disabled mod's
/// settings, and a disabled mod's settings are not shown on the Mod Settings
/// screen, so the identical refusal returns. The one escape measured is Reset
/// mod settings plus Disable listed mods: six steps, every startup preference
/// in the file lost, the mod disabled and a restart needed. A refusal on a
/// field the player types into was therefore a lock-out, and the library owns
/// it.
///
/// ERROR IS UPPERCASE DELIBERATELY. Factorio's `log()` has one channel and no
/// severity (verified: `fk_log` maps to the global `log`, and there is no
/// second channel and no level parameter), so the severity has to be in the
/// text. Uppercase also keeps the line out of a case-sensitive grep for the
/// engine's own `Error` lines while a case-insensitive one still finds it.
///
/// FIELD is the word the player looks for on the settings screen: the text of a
/// list, or the number of a slider. It is now in the sentence TWICE, because
/// the line used to say the mod loaded with its own default instead and that is
/// false wherever a preset dropdown sits beside the field: the set-aside value
/// leaves the dropdown's CURRENTLY CHOSEN preset deciding, not the declaration.
/// Saying the mod loaded as though that field had been left alone is true
/// there, true on every preset, true where no dropdown exists, and true of a
/// number as well as a text. The ROUTE is unchanged and stays: this line is
/// written on a load that SUCCEEDED, so the settings screen really is
/// reachable.
///
/// TAIL is what the field costs beyond being wrong, and it is a parameter
/// rather than a branch on the reason so that no sentence here is chosen by
/// reading another sentence. Only a recipe's ingredient text has one: see
/// [`recipe_text_fallback`].
pub(crate) fn player_fallback(reason: &str, field: &str, tail: &str) -> String {
    format!(
        "{}ERROR: {}. The mod loaded as though that {} had been left alone; fix the {} under Settings > Mod settings > Startup, then restart.{}",
        MESSAGE_PREFIX,
        reason.strip_prefix(MESSAGE_PREFIX).unwrap_or(reason),
        field,
        field,
        tail
    )
}

/// The two fields that exist, named once so no caller spells the word.
pub(crate) fn text_fallback(reason: &str) -> String {
    player_fallback(reason, "text", "")
}

pub(crate) fn number_fallback(reason: &str) -> String {
    player_fallback(reason, "number", "")
}

/// [`text_fallback`] for a RECIPE'S INGREDIENT LIST, which is the one fallback
/// whose fix costs the player something the engine will not give back.
///
/// THE PACK TEXT AND THE TWO NUMBERS DO NOT CARRY IT. Repricing a research
/// destroys nothing; changing a recipe empties every assembling machine holding
/// ingredients the new list does not use, measured and irreversible. See
/// [`RECIPE_CHANGE_SENTENCE`], which the recipe's own tooltip note carries too.
pub(crate) fn recipe_text_fallback(reason: &str) -> String {
    player_fallback(reason, "text", &format!(" {}", RECIPE_CHANGE_SENTENCE))
}

/// Whether a TEXT setting bound to this target decides a recipe's ingredient
/// list, which is the one fallback in the library that changes what a recipe is
/// made of.
///
/// ONE PREDICATE, TWO READERS, and that is the whole reason it has a name. The
/// ERROR line's tail and the prototype tooltip's tail are the same measured
/// sentence about the same engine cost, so they must be true of exactly the
/// same set of fallbacks. Deriving either one from the prototype KIND instead
/// would put the sentence on a crafting-time fallback, which moves
/// `energy_required` and leaves the ingredient list byte for byte.
///
/// IT IS ASKED OF A TEXT SETTING ONLY. A crafting time and a research number
/// never move an ingredient list whatever they are bound to, so those callers
/// pass `false` outright rather than asking.
pub(crate) fn moves_ingredients(tgt: NoteTarget) -> bool {
    !tgt.tech
}

/// Which of the two a text setting's fallback line is, decided by whether the
/// text moves a recipe's ingredient list.
pub(crate) fn text_fallback_for(tgt: NoteTarget, reason: &str) -> String {
    if moves_ingredients(tgt) {
        recipe_text_fallback(reason)
    } else {
        text_fallback(reason)
    }
}

/// What changing a recipe costs a player who has already built with it, and it
/// is the engine's doing rather than this library's.
///
/// MEASURED on 2.0.77 by the consumer's second migration assessment: an
/// assembling machine whose recipe changes has its input slots emptied of
/// anything the new ingredient list does not use, up to eighty items destroyed
/// outright rather than spilled on the ground, with no line anywhere. Nothing a
/// mod emits can change it, so the only thing left is to say so before the
/// player acts.
///
/// ONE CONSTANT, TWO READERS. It is the tail of a recipe's tooltip note and the
/// tail of the ERROR line an ingredient text falls back with, and the two must
/// not drift apart.
pub(crate) const RECIPE_CHANGE_SENTENCE: &str = "Changing a recipe empties an assembling machine's input slots of anything the new list does not use.";

/// The trailing line a prototype whose stored value was set aside carries in
/// its own `localised_description`.
///
/// THE LOG IS NOT A DISCLOSURE, which is the whole reason this exists. A player
/// reads the settings screen, the recipe or technology tooltip and the
/// changelog; a `fkrecipes: ` line in factorio-current.log is evidence for a
/// maintainer and a courtesy for the curious. Until this line, a player who
/// typed something the library could not use had nothing where they look saying
/// the game is not what they asked for.
///
/// ENGLISH LITERALS, NEVER A LOCALE KEY, and that is measured rather than
/// preferred. On 2.0.77 a localised string holding an UNDEFINED key drops the
/// whole prototype description silently: no `Unknown key` marker, no empty
/// line, the title and the ingredients still drawn, exit 0, and the engine's
/// own dump holding the description verbatim, so no gate in this repository
/// could see it. A key wrapped as `{"?", {key}, "literal"}` survives, but the
/// library has no localisation channel for prototype prose at all:
/// `append_localised` wraps a consumer's own `description` as a literal too,
/// and inventing a key would make every consumer owe an entry whose absence
/// deletes the sentence it was meant to carry. Every sentence this library
/// composes onto a SETTING is already an English literal for the same reason.
///
/// THE TAIL IS SCOPED BY WHAT MOVED, not by what kind of prototype carries it:
/// `destroys_inputs` is true only where the ingredient list itself changed. See
/// [`Resolution::note_on`].
///
/// IT NAMES NO TARGET, and that is measured rather than tidy. The sentence used
/// to say this mod's own choice applied instead, and on a recipe whose text
/// sits beside a preset dropdown that is false on every preset but the declared
/// one: the consumer measured all six and got six different ingredient lists
/// under one byte-identical note. What is true on all six, and beside a field
/// with no dropdown at all, and of a NUMBER setting as well as a text one, is
/// the library's own rule: a stored value it cannot use behaves exactly as if
/// the player had left the field alone. Naming the preset instead would mean
/// hoisting `read_dropdown` above the composer, which `resolve` keeps
/// deliberately late; the generic is true without it.
pub(crate) fn fallback_note(setting: &str, destroys_inputs: bool) -> String {
    with_destruction(
        format!(
            "The stored value of {} could not be used, so the game loaded as though that setting had been left alone. The reason is in the log.",
            setting
        ),
        destroys_inputs,
    )
}

/// The one place the engine's own permanent cost is joined to a note, so a
/// sentence about an emptied assembling machine cannot be written onto a
/// prototype whose ingredient list did not move. See [`Resolution::note_on`].
pub(crate) fn with_destruction(note: String, destroys_inputs: bool) -> String {
    if destroys_inputs {
        return format!("{} {}", note, RECIPE_CHANGE_SENTENCE);
    }
    note
}

/// The notes an ENVIRONMENTAL degradation leaves in the prototype's own
/// description, in the voice [`fallback_note`] established and for the same
/// reason: THE LOG IS NOT A DISCLOSURE.
///
/// TEN OF THEM, AND THIS IS THE WHOLE LIST. Eight are immediately below, in the
/// order this file defines them; the last two sit further down beside the cycle
/// LINES they go with, because the cycle walk runs after resolution and reads
/// both from `cycle.rs`. (The Go half keeps that pair in `cycle.go` itself,
/// which is the one placement difference between the two lists.) A degradation
/// added without a
/// row here is the defect the consumer's third assessment measured: one arm of
/// this same match logged a line, wrote no note, and moved a technology to the
/// root of the technology tree without saying so anywhere a player looks.
///
/// ```text
/// pack_dropped_note       the chosen source lost SOME of its science packs
/// packless_source_note    the chosen source lost EVERY pack to the tool probe,
///                         and a declared cost sits behind it
/// unpriced_source_note    no source in the chosen ladder handed the library a
///                         cost it could copy, so the declared fallback prices
///                         it and the prerequisite goes with the source that
///                         never answered
/// packless_note           every pack the research names was put to the game
///                         and the game had none of them
/// unreadable_source_note  the chosen source's pack list is in neither engine
///                         form, and a declared cost sits behind it
/// unreadable_copy_note    the same list with nothing declared behind it, so
///                         the unit is emitted with no packs at all
/// clamped_item_note       two ladders landed on one item above 65535
/// clamped_fluid_note      two ladders landed on one fluid above 1e301
/// cycle_prereq_note       a prerequisite this plan made would loop the tree
/// cycle_splice_note       a splice this plan made would loop the tree
/// ```
///
/// THEY ARE NOT FALLBACK NOTES AND MUST NOT READ AS ONE. Nothing was stored, so
/// there is no field to go and fix and no "stored value" to name: the game
/// itself is missing something the plan names, and the only honest thing to say
/// is what is missing and what the library did instead. That is also why they
/// do not go through `player_fallback` on the log side.
///
/// SCOPED TO THE DEGRADATIONS AND NOT TO THE LADDER. A resolve-or-drop
/// ingredient ladder is the library's advertised contract and the dropdown's
/// own composed description already discloses it ("where one names something
/// your mods do not have, the nearest thing they do have is used instead"); a
/// clamped amount, a dropped science pack, an emptied unit, a dropped
/// prerequisite and a technology left hanging off nothing are arithmetic and
/// presence a player cannot check anywhere.
pub(crate) fn pack_dropped_note(name: &str) -> String {
    format!(
        "This game has no {}, so this research was priced without it. The reason is in the log.",
        name
    )
}

/// What a technology carries when the cost it copies named science packs and
/// this game has none of them.
///
/// IT STATES THE ENVIRONMENTAL FACT AND STOPS THERE, and that is a correction
/// rather than a style. The sentence used to end "so this mod's own declared
/// cost applies", which is a claim about the PRICE the player ends up with, and
/// the price is not this branch's to describe: a `cost_from` beside the tier
/// lets the player override the count or the seconds while leaving the pack
/// text at `default`, `restore_note` fires only on a typed PACK LIST, so the
/// note survives and a number in the emitted unit is the player's own. That is
/// finding 19 one arm over, telling a player the mod's default applied where
/// their own choice did, in a tooltip. What IS true whatever the settings
/// beside it say is that the cost this research copies did not price it, so
/// that is the whole sentence.
///
/// THE ERROR LINE BESIDE IT KEEPS ITS OWN WORDING (`packless_source_line`, "so
/// this mod's own declared cost applies instead"), and the two now differ on
/// purpose. The line is AUTHOR-FACING and is written where the decision was
/// made, before any player field is read, so "the declared cost applies" is
/// exactly what the library did next and is true at that point in the walk. The
/// note is PLAYER-FACING and is read after the whole resolution, beside numbers
/// the player may have moved. Same event, two readers, two moments.
pub(crate) fn packless_source_note(source: &str) -> String {
    format!(
        "This game has none of the science packs the {} cost names, so that cost was not used to price this research. The reason is in the log.",
        source
    )
}

/// What a technology carries when NOT ONE source in the chosen tier's ladder
/// handed this library a cost it could copy: the research is priced without one
/// and hangs off nothing at all.
///
/// IT IS THE LARGEST OF THESE DEGRADATIONS AND IT WAS THE ONLY SILENT ONE. The
/// two arms beside it move a PRICE; this one also removes the PREREQUISITE, so
/// the research sits at the root of the technology tree, researchable from the
/// first minute, at whatever it is priced at instead. Measured by the consumer
/// on a pack that renames one base technology: eight rows, every one exit 0, a
/// log line, no note, and a research nobody had to earn. The criterion the
/// other notes are written to, presence a player cannot check anywhere, applies
/// to a missing prerequisite at least as strongly as to a dropped science pack.
///
/// "A COST THIS MOD CAN USE HERE" IS THE WHOLE CLAIM, and each of its two
/// halves is load-bearing. It does not say the sources carry no cost, because
/// one of the ways this branch is reached is a source that EXISTS and carries a
/// unit this library cannot copy: a unit that is not a dictionary, or one
/// holding a subtree the copy drops. [`unreadable_source_note`]'s own comment
/// already refuses that move one level down, where a sentence saying this game
/// has none of those packs "would be stating something the library does not
/// know"; the first draft of this one made exactly that mistake one level up.
/// The phrase as written is true of an ABSENT source, a `research_trigger`
/// source and a PRESENT BUT UNCOPYABLE one alike, which is every way the walk
/// gets here.
///
/// AND IT DOES NOT NAME THE PRICE IT LANDED ON, for the reason
/// [`packless_source_note`] does not: a `cost_from` beside the tier lets the
/// player move the count or the seconds while the pack text stays at `default`,
/// so "priced at this mod's own declared cost" is a claim about a number that
/// may be theirs. "No copied cost" is true whatever the settings beside it say,
/// and the prerequisite half is a fact about tree shape no setting touches.
///
/// IT NAMES NO DROPDOWN VALUE, deliberately. The value the tier is on is a raw
/// setting string an author picked for a settings file, and this sentence goes
/// into a player's tooltip, where every other note is English prose. The ERROR
/// line beside it already carries that value for an author reading a log, which
/// is the reader it is a name for.
///
/// AND IT TAKES NO ARGUMENT AT ALL, for the reason [`packless_note`] takes
/// none: there is no source to name, because not one of them answered.
pub(crate) fn unpriced_source_note() -> String {
    String::from(
        "No technology this research takes its cost from carries a cost this mod can use here, so this research has no prerequisite and no copied cost. The reason is in the log.",
    )
}

/// What a technology priced in NO science pack at all carries when every pack
/// it named was PUT TO THE GAME and the game had none of them.
///
/// IT IS NOT THE ONLY EMPTIED-UNIT NOTE, and the other one is the reason this
/// sentence can say what it says. A copied unit whose pack list this library
/// could not decode is emitted empty too, and there the list was never read, so
/// nothing was asked and this sentence would be stating something the walk
/// never established: that one carries [`unreadable_copy_note`] instead.
///
/// IT TAKES NO ARGUMENT, deliberately. The names the walk asked the game about
/// are in the ERROR line, where an author reading a log can use them; a player
/// hovering a technology cannot act on a list of prototype names their mod set
/// does not have.
pub(crate) fn packless_note() -> String {
    String::from(
        "This game has none of the science packs this research names, so it takes no science pack at all. The reason is in the log.",
    )
}

/// What a technology whose copied cost could not be decoded carries when there
/// IS a declared cost behind it. It is not [`packless_source_note`], because
/// nothing was dropped: the list was never read at all, and a sentence saying
/// this game has none of those packs would be stating something the library
/// does not know.
///
/// IT ENDS ON THE ENVIRONMENTAL FACT for the reason [`packless_source_note`]
/// does, and the reason is worth having in both places: the price this
/// technology ends up at may hold a count or a seconds the player typed beside
/// the tier, so a note claiming the mod's own declared cost applied can be
/// false in a tooltip. What the walk knows, and all it knows, is that the
/// copied cost did not price this research. The ERROR line beside it
/// (`unreadable_source_line`) keeps the author's wording and is unchanged.
pub(crate) fn unreadable_source_note(source: &str) -> String {
    format!(
        "The {} cost this research copies cannot be read in this game, so that cost was not used to price this research. The reason is in the log.",
        source
    )
}

/// [`unreadable_source_note`]'s twin for the arm with NOTHING declared behind
/// it: a `cost_of` whose copied pack list this library cannot decode is emitted
/// with an empty ingredient list, and this is what the player is told where they
/// look.
///
/// IT IS NOT [`packless_note`], and the distinction is the one
/// [`unreadable_source_note`] already draws for the arm beside it.
/// `packless_note` says this game has none of the science packs the research
/// names, which is a fact about the game; on this path the list was never
/// decoded, so the library never asked the game about any pack and does not
/// know that. What it does know is that it could not read the cost it was told
/// to copy, and that is what the sentence says.
pub(crate) fn unreadable_copy_note(source: &str) -> String {
    format!(
        "The {} cost this research copies cannot be read in this game, so it takes no science pack at all. The reason is in the log.",
        source
    )
}

/// `clamped_item_note` and `clamped_fluid_note` are one degradation with two
/// ceilings, and they are two sentences because "what one slot holds" is true
/// of an item stack and false of a fluid. The numbers are spelled here rather
/// than formatted, for the reason `merged_opening`'s own comment gives.
pub(crate) fn clamped_item_note(name: &str) -> String {
    format!(
        "Two ingredients resolved onto {} and the total was above what one slot holds, so it was capped at {}. The reason is in the log.",
        name, MAX_ITEM_AMOUNT
    )
}

pub(crate) fn clamped_fluid_note(name: &str) -> String {
    format!(
        "Two ingredients resolved onto {} and the total was above the largest amount the game can hold, so it was capped at 1e301. The reason is in the log.",
        name
    )
}

/// The one sentence a stored value that is not text is answered with, built in
/// ONE place: two spellings of one sentence is exactly the drift the corpus
/// exists to prevent for the language, and this layer gets the same treatment.
///
/// THE NAME SPELLS OUT SENTENCE because `crate::value::not_text` is a different
/// message about a different question (bytes that are not UTF-8 crossing the
/// emit seam), and two `not_text`s in one crate is a name a reader has to
/// disambiguate by import path. The Go half named its own the same way.
pub(crate) fn not_text_sentence(setting: &str) -> String {
    format!("{}{} is not text", MESSAGE_PREFIX, setting)
}

/// Which rule a number failed, and it exists so that the ORDER the questions
/// are asked in lives in one place while the SENTENCE they are answered with
/// lives in two.
///
/// TWO WORDINGS, BECAUSE THE TWO SIDES ARE ABOUT DIFFERENT VALUES. A stored
/// value is what the SETTING ANSWERED, and the value a fallback lands on is
/// what the PLAN DECLARED; one wording over both would describe the setting's
/// answer while talking about the declaration that replaced it. See
/// `stored_number_problem` and `declared_number_problem`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumberFault {
    None,
    NotFinite,
    CountBelowOne,
    TimeAtOrBelowZero,
    BelowCraftTimeFloor,
}

/// `count_fault`, `seconds_fault` and `craft_time_fault` answer which rule a
/// number failed, or `NumberFault::None`.
///
/// FINITENESS FIRST WITHIN EACH NUMBER, and not only for the message: a floor
/// is a question only a finite number can be asked. A NaN is neither below 1
/// nor at or below zero, so a floor arm reached first would wave it through,
/// and an infinity would be sorted by whichever side of the floor it fell on
/// rather than told the one thing that is actually wrong with it.
///
/// ONE FUNCTION PER NUMBER RATHER THAN ONE ACROSS ALL OF THEM, which is a
/// change from the interleaved order this layer used to have: each number now
/// falls back on its own and logs its own line, so a world where two of them
/// are wrong answers about both instead of picking one. The residual refusal
/// walks them in the same per-number order, so there is one ordering in this
/// file rather than two.
pub(crate) fn count_fault(v: f64) -> NumberFault {
    if !finite(v) {
        NumberFault::NotFinite
    } else if v < 1.0 {
        NumberFault::CountBelowOne
    } else {
        NumberFault::None
    }
}

pub(crate) fn seconds_fault(v: f64) -> NumberFault {
    if !finite(v) {
        NumberFault::NotFinite
    } else if v <= 0.0 {
        NumberFault::TimeAtOrBelowZero
    } else {
        NumberFault::None
    }
}

/// `deferrable_count_fault` and `deferrable_seconds_fault` are the same two
/// rules with 0 let through, which is what a research number beside a `cost_by`
/// dropdown means by 0: the dropdown decides. Everything else the engine would
/// refuse is still refused, so a number that IS in force is one the engine
/// takes.
///
/// A SEPARATE PAIR RATHER THAN A FLAG ON THE ORIGINALS, because the originals
/// are also the AUTHOR-side post-condition over a built unit, where 0 is a
/// research nobody can finish and has to stay a fault.
pub(crate) fn deferrable_count_fault(v: f64) -> NumberFault {
    if v == 0.0 {
        NumberFault::None
    } else {
        count_fault(v)
    }
}

pub(crate) fn deferrable_seconds_fault(v: f64) -> NumberFault {
    if v == 0.0 {
        NumberFault::None
    } else {
        seconds_fault(v)
    }
}

pub(crate) fn craft_time_fault(v: f64) -> NumberFault {
    if !finite(v) {
        NumberFault::NotFinite
    } else if v <= CRAFT_TIME_FLOOR {
        NumberFault::BelowCraftTimeFloor
    } else {
        NumberFault::None
    }
}

/// What a value the SETTING ANSWERED WITH is told, and the reason a fallback
/// line quotes. It names the SETTING rather than the technology, because the
/// technology's own declaration is fine and the value came from outside it.
///
/// A FAULT NO NUMBER OF THIS KIND CAN HAVE ANSWERS WITH THE EMPTY STRING, which
/// the prefix witness catches: a sentence with no prefix on it fails that test
/// rather than reaching a player as a line with a hole in it.
pub(crate) fn stored_number_problem(setting: &str, f: NumberFault) -> String {
    match f {
        NumberFault::NotFinite => format!(
            "{}{} holds a value that is not a finite number",
            MESSAGE_PREFIX, setting
        ),
        NumberFault::CountBelowOne => format!(
            "{}{} holds a research count below 1",
            MESSAGE_PREFIX, setting
        ),
        NumberFault::TimeAtOrBelowZero => format!(
            "{}{} holds a research time at or below zero",
            MESSAGE_PREFIX, setting
        ),
        NumberFault::None | NumberFault::BelowCraftTimeFloor => String::new(),
    }
}

/// What the value a fallback LANDED ON is refused with, and it says declared
/// default out loud: by the time this is reached the stored value is gone and
/// the number being complained about is the one the plan wrote, which is an
/// author's bug rather than a player's typing.
pub(crate) fn declared_number_problem(setting: &str, f: NumberFault) -> String {
    match f {
        NumberFault::NotFinite => format!(
            "{}{} declares a default that is not a finite number",
            MESSAGE_PREFIX, setting
        ),
        NumberFault::CountBelowOne => format!(
            "{}{} declares a default research count below 1",
            MESSAGE_PREFIX, setting
        ),
        NumberFault::TimeAtOrBelowZero => format!(
            "{}{} declares a default research time at or below zero",
            MESSAGE_PREFIX, setting
        ),
        NumberFault::None | NumberFault::BelowCraftTimeFloor => String::new(),
    }
}

/// The same split for a recipe's bound crafting time, which names the recipe as
/// well as the setting because that is the sentence this value has always been
/// answered with.
pub(crate) fn stored_craft_time_problem(recipe: &str, setting: &str, f: NumberFault) -> String {
    match f {
        NumberFault::NotFinite => format!(
            "{}the recipe {} reads its crafting time from {}, which answers a value that is not a finite number",
            MESSAGE_PREFIX, recipe, setting
        ),
        NumberFault::BelowCraftTimeFloor => format!(
            "{}the recipe {} reads its crafting time from {}, which answers at or below the engine floor (energy_required can't be <= 0.001)",
            MESSAGE_PREFIX, recipe, setting
        ),
        NumberFault::None | NumberFault::CountBelowOne | NumberFault::TimeAtOrBelowZero => {
            String::new()
        }
    }
}

pub(crate) fn declared_craft_time_problem(recipe: &str, setting: &str, f: NumberFault) -> String {
    match f {
        NumberFault::NotFinite => format!(
            "{}the recipe {} reads its crafting time from {}, whose declared default is not a finite number",
            MESSAGE_PREFIX, recipe, setting
        ),
        NumberFault::BelowCraftTimeFloor => format!(
            "{}the recipe {} reads its crafting time from {}, whose declared default is at or below the engine floor (energy_required can't be <= 0.001)",
            MESSAGE_PREFIX, recipe, setting
        ),
        NumberFault::None | NumberFault::CountBelowOne | NumberFault::TimeAtOrBelowZero => {
            String::new()
        }
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
    append_localised(&mut pairs, &it.spec.display_name, &it.spec.description, "");
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

/// The EMITTED name of what a recipe makes: an item this plan declares
/// (prefixed or legacy by its own declaration) or one that already exists,
/// named verbatim because it is somebody else's and validation has probed it.
/// Empty only for a recipe that declares neither, which `validate` refuses
/// before resolution runs.
///
/// ONE HELPER RATHER THAN TWO COPIES. The prototype builder writes this name
/// into `results` and `resolve` compares it against the resolved ingredients;
/// two spellings of one rule would let the log line and the prototype disagree
/// about what the recipe makes.
fn recipe_product(prefix: &str, l: &Lib, r: &RecipeDecl) -> String {
    if r.result.index != 0 {
        l.items[r.result.index - 1].emitted_name(prefix)
    } else {
        r.spec.result_named.clone()
    }
}

fn recipe_proto(
    prefix: &str,
    l: &Lib,
    r: &RecipeDecl,
    ings: &[ResolvedIngredient],
    ct: &CraftTime,
    unlocked: bool,
    note: &str,
) -> Value {
    let mut pairs = vec![
        kv("type", Value::string("recipe")),
        kv("name", Value::Str(r.emitted_name(prefix))),
    ];
    append_localised(&mut pairs, &r.spec.display_name, &r.spec.description, note);
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
    let result = recipe_product(prefix, l, r);
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

fn tech_proto(
    prefix: &str,
    l: &Lib,
    w: &dyn World,
    t: &TechDecl,
    rt: &ResolvedTech,
    note: &str,
) -> Value {
    let mut pairs = vec![
        kv("type", Value::string("technology")),
        kv("name", Value::Str(t.emitted_name(prefix))),
    ];
    append_localised(&mut pairs, &t.spec.display_name, &t.spec.description, note);
    if !t.spec.icon.is_empty() {
        pairs.push(kv("icon", Value::string(&t.spec.icon)));
    }
    if t.spec.icon_size != 0 {
        pairs.push(kv("icon_size", Value::Num(t.spec.icon_size as f64)));
    }
    if !rt.prereqs.is_empty() {
        pairs.push(kv("prerequisites", str_arr(&rt.prereqs)));
    }
    pairs.push(kv("unit", tech_unit(t, rt)));
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

fn tech_unit(t: &TechDecl, rt: &ResolvedTech) -> Value {
    match &t.spec.unit {
        // Technology unit ingredients are the SHORT TUPLE form. The dict form
        // is refused here by the engine. The packs are the RESOLVED ones:
        // resolution walked each ladder, and a pack the game does not have is
        // already gone with its log line behind it.
        Some(unit) => unit_value(unit.count, unit.seconds, &rt.packs),
        // EVERY OTHER ARM SETTLED ITS COST DURING RESOLUTION, `cost_of` with
        // the rest: a copied unit's science packs are a question about the
        // game, so they are asked where every other question about the game is
        // asked and where a log line can still be written. A source that
        // answers with no unit at all leaves this None, which is the same nil
        // the Go mirror returns.
        None => rt.unit.clone().unwrap_or(Value::Nil),
    }
}

/// The ONE writer of `localised_name` and `localised_description` on every
/// prototype this library emits, and the note is the trailing line a recipe or
/// a technology carries when a stored value was set aside.
///
/// THREE SHAPES AND NOT FOUR. An author's description with no note is
/// `{"", "<description>"}` byte for byte as it always was, so a golden taken
/// before this line existed does not move for a load nothing fell back on; a
/// description with a note adds the note as a further parameter opening with a
/// newline; and a note with no description is the note alone in the same
/// two-element shape. Nothing is emitted when there is neither. Each of the
/// three is the shape of the SHORT case: a part over the chunk budget is more
/// than one parameter, and the parameters concatenate to the same bytes.
///
/// AN ITEM NEVER CARRIES A NOTE, so `item_proto` passes the empty string. The
/// fallback is about what a recipe makes or what a technology costs, and an
/// item prototype is neither.
///
/// EVERY PARAMETER IS CHUNKED AND THE WHOLE IS GROUPED, because the engine
/// polices ONE STRING ELEMENT at 200 BYTES on a data-stage prototype and the
/// three notes a RECIPE can carry reach 229, 229 and 246 bytes before any name
/// goes into them: before the splitter, no consumer on any mod name could bind
/// a text setting to a recipe's ingredient list and have the resulting fallback
/// load at all. Each figure is a sentence of its own plus one space plus the
/// 100 bytes of `RECIPE_CHANGE_SENTENCE`: 128 + 1 + 100 for [`fallback_note`],
/// 128 + 1 + 100 for [`clamped_item_note`] and 145 + 1 + 100 for
/// [`clamped_fluid_note`].
///
/// THE TAIL IS WHAT MOVED AND NOT WHAT CARRIES IT, which is
/// [`with_destruction`]'s whole rule and is why those three are MAXIMA rather
/// than fixed widths. A recipe whose CRAFTING TIME fell back takes
/// [`fallback_note`] with `destroys_inputs` false (the crafting-time arm of
/// `resolve` above), so its note is the bare 128 bytes and no tail at all: an
/// ingredient list that did not move empties no assembling machine.
/// `fallback_note_shape` pins both halves of that.
///
/// A TECHNOLOGY's notes never take the tail, because a research costs no
/// assembling machine anything, and the longest of them before a name goes in
/// is [`unpriced_source_note`]'s 168, which takes no name at all and so is one
/// element on every mod set; it is chunked all the same, because nothing here
/// decides per sentence. See `LOCALISED_CHUNK_BUDGET` and `chunk_localised` for
/// the measurement and for the properties the split has by construction.
///
/// THE DESCRIPTION AND THE NOTE ARE CHUNKED SEPARATELY, so the newline stays at
/// the head of the note's first chunk; a short description with a short note is
/// two parameters and keeps the shape it always had.
///
/// THE GROUPING HELPER RATHER THAN THE FLAT ONE, because the parameter count
/// here is UNBOUNDED: a consumer's `description` is unbounded, and a long one
/// is as many chunks as it takes. `localised_group` answers the other measured
/// ceiling (20 PARAMETERS PER TABLE and 20 LEVELS OF NESTING DEPTH, measured on
/// 2.0.77: the 21st of either refuses the load by name, and a description
/// holding 421 tables at depth 3 loads, so there is no global table budget) by
/// keeping the first nineteen parameters and handing the rest to a nested group
/// in the twentieth slot. That is 19*(d-1)+20 parameters at depth d, so 381
/// chunks at the measured depth ceiling of 20, which is 68,580 bytes of one
/// description at 180 bytes a chunk. Past that it is a consumer's own declared
/// description that refuses, and it refuses on DEPTH rather than on the element
/// rule.
fn append_localised(
    pairs: &mut Vec<(String, Value)>,
    display_name: &str,
    description: &str,
    note: &str,
) {
    if !display_name.is_empty() {
        pairs.push(kv("localised_name", localised(display_name)));
    }
    match (description.is_empty(), note.is_empty()) {
        (false, false) => {
            let mut params = localised_chunks(description);
            params.extend(localised_chunks(&format!("\n{}", note)));
            pairs.push(kv("localised_description", localised_group(&params)));
        }
        (false, true) => pairs.push(kv("localised_description", localised(description))),
        (true, false) => pairs.push(kv("localised_description", localised(note))),
        (true, true) => {}
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

    /// Refuses a DECLARED list that names one thing twice.
    ///
    /// IT IS THE OTHER HALF OF THE MERGE, and the two are not the same
    /// problem. A ladder that COLLAPSES onto a name the list already carries
    /// is a mod set taking an ingredient away, and the amounts are added so
    /// the recipe still loads for a player who never opened the settings. A
    /// duplicate the AUTHOR WROTE OUT is a bug in the declaration, and adding
    /// it up silently would emit a recipe nobody designed and say nothing:
    /// `4 iron-plate, 2 iron-plate` composed a dropdown description the
    /// player's own custom field refuses, while the recipe quietly said 6.
    /// Refused here, because an author's bug is caught in development and a
    /// player is not locked out of a game.
    ///
    /// AFTER THIS, THE MERGE IS REACHABLE ONLY FROM A LADDER COLLAPSE, which
    /// is what makes the merge line's phrase "after the fallbacks" true
    /// wherever it appears.
    ///
    /// THE HEADS ARE COMPARED AS RESOLUTION WILL SEE THEM, through
    /// `declared_head`: a ladder by its first rung, this plan's own item by
    /// the name it is EMITTED under, which is the pair a description already
    /// shows. Any looser comparison would leave a collision that is plain in
    /// the declaration to be found later by the merge, and the sentence about
    /// fallbacks would be false about it.
    ///
    /// KIND IS PART OF THE IDENTITY. An item and a fluid of one name are two
    /// ingredients, and the engine takes both in one recipe (measured: base
    /// carries parameter-0 to parameter-9 as an item AND a fluid).
    ///
    /// LAST, AND ACROSS THE WHOLE LIST, exactly where the language checks it:
    /// it is the one problem no single entry can see. Every caller runs
    /// `validate_ingredients` first, which is what proves a handle before
    /// `declared_head` reads it.
    pub(crate) fn validate_no_duplicates(
        &self,
        at: &str,
        who: &str,
        prefix: &str,
        ings: &[Ingredient],
    ) -> Result<(), String> {
        let names: Vec<String> = ings
            .iter()
            .map(|ing| self.declared_head(prefix, ing))
            .collect();
        // The SECOND occurrence is what the walk reports, with the earliest
        // partner, which is the order the language's own duplicate rule uses.
        for j in 1..ings.len() {
            for i in 0..j {
                if ings[i].amount.is_fluid() == ings[j].amount.is_fluid() && names[i] == names[j] {
                    return Err(format!(
                        "{}{} names {} twice; each ingredient is taken once",
                        at, who, names[j]
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
        has_tier: bool,
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
        if !self.valid_int_setting(cc.seconds) {
            return Err(format!(
                "{}{} reads its research time from a setting that this plan never declared",
                at, who
            ));
        }
        let count = &self.settings[cc.count.index - 1];
        if has_tier {
            if count.def_num != 0.0 || count.spec.min.map(|m| m != 0.0).unwrap_or(true) {
                return Err(format!(
                    "{}the setting {} backs a research count beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)",
                    at, count.name
                ));
            }
        } else if count.spec.min.map(|m| m < 1.0).unwrap_or(true) {
            return Err(format!(
                "{}the setting {} backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)",
                at, count.name
            ));
        }
        research_number_maximum(at, count)?;
        let seconds = &self.settings[cc.seconds.index - 1];
        if has_tier {
            if seconds.def_num != 0.0 || seconds.spec.min.map(|m| m != 0.0).unwrap_or(true) {
                return Err(format!(
                    "{}the setting {} backs a research time beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)",
                    at, seconds.name
                ));
            }
        } else if seconds.spec.min.map(|m| m < 1.0).unwrap_or(true) {
            return Err(format!(
                "{}the setting {} backs a research time but declares no minimum of at least 1 (the engine refuses a unit time of 0)",
                at, seconds.name
            ));
        }
        research_number_maximum(at, seconds)
    }

    #[allow(clippy::only_used_in_recursion)]
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
            // THE SAME 16 BITS AN ITEM INGREDIENT IS HELD IN, and it is the
            // engine's own rule rather than an analogy. MEASURED (Factorio
            // 2.0.77, build 84539, mac-arm64, steam), on a technology whose
            // unit ingredients carry one pack: 65535 loads and dumps as
            // written; 65536, 2^31 and 2^53 each refuse the load with "Value
            // (<n>) outside of range. The data type allows values from 0 to
            // 65535 in property tree at
            // ROOT.technology.<name>.unit.ingredients[0][1]", exit 1, no dump.
            // A declared pack above it used to reach the engine and fail the
            // whole load, blaming the consumer's mod for a number its author
            // wrote, which is the same defect the ingredient ceiling closed.
            //
            // AFTER THE NAME CHECKS, so the sentence has a name to quote.
            if p.amount > MAX_ITEM_AMOUNT {
                return Err(format!(
                    "{}the technology {} takes {} of {}, and a science pack amount goes up to {}",
                    at, name, p.amount, p.name, MAX_ITEM_AMOUNT
                ));
            }
            // WHETHER THE GAME HAS THE PACK IS NOT ASKED HERE. It is a ladder
            // now, and a ladder is walked at resolution, where a pack the
            // game does not have is dropped with a log line the way an
            // ingredient is.
        }
        validate_no_duplicate_packs(at, &format!("the technology {}", name), &u.packs)
    }
}

/// [`Lib::validate_no_duplicates`] for a declared pack list.
///
/// NO KIND AND NO PREFIX. A science pack is asked for with `tool_exists`, a
/// tool is an item, and this library declares no tools, so a pack list has one
/// namespace and the declared names alone decide identity.
pub(crate) fn validate_no_duplicate_packs(
    at: &str,
    who: &str,
    packs: &[Pack],
) -> Result<(), String> {
    for j in 1..packs.len() {
        for i in 0..j {
            if packs[i].name == packs[j].name {
                return Err(format!(
                    "{}{} names {} twice; each science pack is taken once",
                    at, who, packs[j].name
                ));
            }
        }
    }
    Ok(())
}

/// The ceiling both research numbers need, written once because the sentence is
/// one sentence: the count and the time are the same kind of field to a player
/// and the same kind of hole to a modpack.
///
/// THE SENTENCE NAMES THE WHOLE PREDICATE, which is not "no maximum": a
/// declaration of `between(0.0, 0.0)` carries one and it is a ceiling no legal
/// value can sit under, so a sentence that said the maximum was missing would
/// be untrue of half the declarations that reach this line. "No maximum of at
/// least 1" is true of both arms, and the Go half carries it byte for byte.
fn research_number_maximum(at: &str, s: &SettingDecl) -> Result<(), String> {
    if s.spec.max.map(|m| m < 1.0).unwrap_or(true) {
        return Err(format!(
            "{}the setting {} backs a research number but declares no maximum of at least 1; a number the player types needs a ceiling it can reach",
            at, s.name
        ));
    }
    Ok(())
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
    /// never typed takes. A readable value that is not a string becomes the
    /// word too, with ONE fallback line: the engine resets a wrong-typed stored
    /// value to the default before any stage runs (measured), so it is a
    /// hand-edited file, and the line says so without stopping the game. It
    /// used to refuse; see `player_fallback` for the client measurement that
    /// decided it does not.
    ///
    /// IT YIELDS BYTES, and both string arms are one answer here. Whether a
    /// stored value is text is the LANGUAGE's question, asked once at the top
    /// of its parse and answered the same way in both halves; a second answer
    /// taken here, by treating `Value::Bytes` as "not a string", would say the
    /// wrong sentence and would be a rule only this half has.
    ///
    /// THE TARGET IS WHAT PICKS THE SENTENCE, not the list kind the caller went
    /// on to parse with, and the two agree by construction: a recipe target
    /// reaches this function only for its own ingredient list and a technology
    /// target only for its pack text. Choosing off the target is what keeps the
    /// ERROR line and the prototype note the target also fills saying the same
    /// thing about the same prototype.
    fn read_text_setting(
        &self,
        w: &dyn World,
        res: &mut Resolution,
        full: &str,
        tgt: NoteTarget,
    ) -> Vec<u8> {
        match w.startup_setting(full) {
            Some(Value::Str(text)) => text.into_bytes(),
            Some(Value::Bytes(b)) => b,
            Some(_) => {
                res.note_fallback(
                    tgt,
                    full,
                    text_fallback_for(tgt, &not_text_sentence(full)),
                    moves_ingredients(tgt),
                );
                Vec::from(DEFAULT.as_bytes())
            }
            None => {
                res.logs.push(format!(
                    "fkrecipes: the setting {} was not readable, so its default applies",
                    full
                ));
                Vec::from(DEFAULT.as_bytes())
            }
        }
    }

    /// One recipe's ingredient text, read and parsed, or `None` when the
    /// answer is the reserved word.
    ///
    /// THE WORD `default` IS THE PRE-EXISTING PATH and it logs nothing of its
    /// own: the player who never typed gets exactly what the declaration says,
    /// which beside a dropdown is the dropdown's chosen preset and on its own
    /// is the setting's declared list. A TEXT THE LANGUAGE CANNOT READ TAKES
    /// THE SAME PATH, with one line of its own saying so: see
    /// `player_fallback`. Anything else is taken as written, and the CALLER
    /// writes the line, because only the caller knows whether a choice was set
    /// aside.
    ///
    /// TWO WORLDS, AND THE SPLIT IS THE POINT. What the PLAYER typed is read
    /// against `own`, which knows this plan's own item names; the author's own
    /// ladders behind the word `default` are walked against the game as it
    /// stands, exactly as they were before, because a ladder is a tolerance
    /// for a modpack rather than a lookup of this plan's own prototypes.
    fn read_text_ingredients(
        &self,
        w: &dyn World,
        own: &dyn World,
        res: &mut Resolution,
        r: &RecipeDecl,
        full: &str,
        tgt: NoteTarget,
    ) -> Option<IngredientList> {
        let text = self.read_text_setting(w, res, full, tgt);
        let lang = self.installed_language();
        match (lang.parse)(&text, ListKind::Recipe, &r.spec.category, full, own) {
            // THE MESSAGE IS THE WHOLE DIAGNOSIS, verbatim: the language wrote
            // it for the player, naming the setting, the entry and the problem,
            // and there is nothing this layer can add to it. It rides inside
            // ONE fallback line with the shared prefix trimmed off, because the
            // line it sits in already opens with one.
            Err(message) => {
                res.note_fallback(
                    tgt,
                    full,
                    text_fallback_for(tgt, &message),
                    moves_ingredients(tgt),
                );
                None
            }
            Ok(ListText::Default) => None,
            Ok(ListText::List(list)) => Some(list),
        }
    }

    /// One technology's unit, from two numeric settings and a pack text.
    ///
    /// The three are read in the order the log line names them and the order
    /// the unit carries them: count, time, packs.
    ///
    /// THE THREE ARE ALWAYS READ, whatever the tier says, because reading them
    /// is how this finds out whether any of them is in force. Each one that is
    /// not at its declared default overrides the tier; each one that is comes
    /// FROM the tier, or from the setting's own declared default where there is
    /// no tier. Nothing is ever edited and ignored, which is the whole point of
    /// the shape.
    ///
    /// IT ANSWERS `None` WHEN THE CUSTOM COST DOES NOT APPLY, which is a tier
    /// with nothing non-default beside it: the caller then emits the tier byte
    /// for byte, which is the load a player who never opened the settings
    /// screen gets.
    ///
    /// THE TWO NUMBERS ARE FACTS ABOUT THE WORLD, not about the declaration,
    /// so they are asked the finiteness question the resolved crafting time is
    /// asked and then the bound their own declaration promised. The declared
    /// minima keep the ENGINE from handing back a count below 1 or a time at
    /// or below zero (measured: a stored value outside a setting's own bounds
    /// is reset to that setting's default), but a fixture World can answer
    /// anything at all, and a NaN reaching the decimal rule is a unit rendered
    /// as `NaN` in this half and a trap in the Go mirror.
    #[allow(clippy::too_many_arguments)]
    fn resolve_custom_cost(
        &self,
        w: &dyn World,
        own: &dyn World,
        res: &mut Resolution,
        prefix: &str,
        t: &TechDecl,
        cc: &CustomCost,
        tier: &CostTier,
        tgt: NoteTarget,
    ) -> Option<Value> {
        let lang = self.installed_language();
        let has_tier = matches!(tier, CostTier::Chosen { .. });
        // EACH NUMBER IS HELD TO WHAT THE ENGINE TAKES WHERE IT IS READ, and
        // one that is not takes the setting's DECLARED DEFAULT with a line
        // naming it, which is also how it stops being in force. The rules
        // differ by one value: beside a tier, 0 is the number's reserved word
        // and is legal, and every other number still has to be one the engine
        // would take.
        let (count_rule, seconds_rule): (FaultFn, FaultFn) = if has_tier {
            (deferrable_count_fault, deferrable_seconds_fault)
        } else {
            (count_fault, seconds_fault)
        };
        let (count, count_setting) =
            self.read_cost_number(w, res, prefix, cc.count.index, count_rule, tgt);
        let (seconds, seconds_setting) =
            self.read_cost_number(w, res, prefix, cc.seconds.index, seconds_rule, tgt);
        let s = &self.settings[cc.packs.index - 1];
        let full = s.emitted_name(prefix);
        let text = self.read_text_setting(w, res, &full, tgt);
        let typed = match (lang.parse)(&text, ListKind::Packs, "", &full, own) {
            // ONE LINE AND THE PATH THE RESERVED WORD TAKES, the same shape the
            // recipe path takes: a refused text lands where a player who typed
            // nothing lands.
            Err(message) => {
                res.note_fallback(
                    tgt,
                    &full,
                    text_fallback_for(tgt, &message),
                    moves_ingredients(tgt),
                );
                None
            }
            Ok(ListText::Default) => None,
            Ok(ListText::List(list)) => Some(
                list.entries
                    .iter()
                    .map(|e| ResolvedPack {
                        name: e.name.clone(),
                        // A pack list resolves through tool_exists and a tool
                        // is an item, so every entry the parser returns here
                        // carries an item amount: `resolve_for_packs` answers
                        // "not a fluid" for every name it accepts, and the
                        // fluid arm of an entry is reached only behind that
                        // answer.
                        amount: match e.amount {
                            Amount::Item(n) => n,
                            Amount::Fluid(_) => {
                                unreachable!("a science pack list parsed a fluid entry")
                            }
                        },
                    })
                    .collect::<Vec<ResolvedPack>>(),
            ),
        };

        let count_set = count != self.settings[cc.count.index - 1].def_num;
        let seconds_set = seconds != self.settings[cc.seconds.index - 1].def_num;
        let (tier_unit, dropdown, chosen, tier_source, tier_note) = match tier {
            CostTier::Chosen {
                unit,
                dropdown,
                chosen,
                source,
                note,
            } => (
                Some(unit),
                dropdown.as_str(),
                chosen.as_str(),
                source.as_str(),
                note.as_str(),
            ),
            CostTier::None => (None, "", "", "", ""),
        };
        if has_tier && !count_set && !seconds_set && typed.is_none() {
            return None;
        }

        // THE PACKS, and the three sources in the order the rule names them:
        // what the player typed, then the tier's own ingredients, then the
        // author's declared list with its ladders walked and its drops logged.
        //
        // A TIER'S OWN PACKS ARE NOT HELD TO THE PACKLESS RULE. They are
        // another technology's declaration, and the tier arm has already probed
        // them against the game and degraded where it had to: nothing is left
        // for this arm to mark.
        let packs: Vec<ResolvedPack> = match (&typed, tier_unit) {
            (Some(list), _) => {
                // A TYPED PACK LIST IS WHAT THIS TECHNOLOGY IS PRICED IN, so
                // everything the tier arm said about the TIER'S packs is now
                // about a price nothing emits, and each piece of it is taken
                // back here.
                //
                // THE TOOLTIP FIRST, IN ONE CALL. Everything the tier arm
                // wrote into this technology's note slot is about a price
                // nothing emits now, so the slot goes back to what it held
                // before that arm ran. A snapshot rather than a list of named
                // retractions, because the tier arm can leave a sentence this
                // site cannot compose: its fallback's own pack ladders can land
                // twice on one name and CLAMP, that note takes the slot, and a
                // by-name retraction would find nothing and leave a
                // capped-amount tooltip on a technology whose emitted price is
                // the player's own list. See [`Resolution::note_at`].
                if has_tier {
                    res.restore_note(tgt, tier_note);
                }
                // AND THEN THE LINES, WHICH HAVE NO SLOT TO PUT BACK. A log
                // line is a stream, so each one is named: the packless line the
                // CostBy fallback logged when it lost every pack it declared a
                // moment ago, and the tier's own sentence for a source that
                // lost every pack or carried a list this library could not
                // read. ALL of them are named because only one can have been
                // written, and retracting a line that was never written matches
                // nothing.
                //
                // THE DROP LINES STAY, because they are true: those packs
                // really are absent from this game, and the line says only
                // that.
                if has_tier {
                    res.retract_packless_line(tgt);
                }
                if !tier_source.is_empty() {
                    res.retract_log(&packless_source_line(&t.name, tier_source));
                    res.retract_log(&unreadable_source_line(&t.name, tier_source));
                }
                list.iter()
                    .map(|p| ResolvedPack {
                        name: p.name.clone(),
                        amount: p.amount,
                    })
                    .collect()
            }
            (None, Some(unit)) => tier_pack_list(unit),
            (None, None) => {
                let (packs, tried) = resolve_packs(w, res, tgt, &t.name, &s.def_packs);
                if packs.is_empty() {
                    res.packless_at(tgt, &t.name, tried);
                }
                packs
            }
        };

        // AND THE POST-CONDITION, on whatever the two reads settled on, and
        // ONLY where the unit is built fresh. After a fallback the value IS the
        // declared default, so the only world this can still refuse is a plan
        // whose declared default is itself outside what the engine takes. That
        // is an AUTHOR bug: `validate_settings` refuses it at the settings
        // stage, which the engine runs before the data stage, so it reaches
        // here only through a host test that calls `plan_data` on its own.
        //
        // BESIDE A TIER THERE IS NOTHING FOR IT TO ANSWER: a number that is in
        // force there has already cleared the same rule with 0 excluded, and a
        // number that is not in force is the tier's own, which belongs to
        // whoever declared that technology.
        if !has_tier {
            let count_bad = count_fault(count);
            let seconds_bad = seconds_fault(seconds);
            if count_bad != NumberFault::None {
                res.refuse(declared_number_problem(&count_setting, count_bad));
            } else if seconds_bad != NumberFault::None {
                res.refuse(declared_number_problem(&seconds_setting, seconds_bad));
            }
        }

        // THE NUMBERS THE LINE AND THE UNIT CARRY, which are the settings'
        // where they are in force and the tier's where they are not. A tier
        // that carries no count at all (a count_formula prices it instead)
        // leaves the tier's own field alone in the unit.
        //
        // COUNT BY FORMULA IS A PRICE WITH NO NUMBER IN IT, and the line says
        // so rather than printing one. A count setting that DEFERS beside such
        // a tier leaves nothing for the line to name: printing the setting's
        // declared default there (0, beside a dropdown) is a number nothing in
        // the emitted unit is using. Both terms are load-bearing. The formula
        // test is what keeps the phrase true: a unit with neither field is one
        // the engine refuses outright (measured on 2.0.77: `Key "count_formula"
        // not found in property tree`), so it is reachable only from a fixture
        // World, and there the honest answer is the number rather than a
        // formula that is not there.
        let mut count = count;
        let mut seconds = seconds;
        let mut by_formula = false;
        if let Some(unit) = tier_unit {
            if !count_set {
                match tier_number(unit, "count") {
                    Some(v) => count = v,
                    None => by_formula = has_unit_field(unit, "count_formula"),
                }
            }
            if !seconds_set {
                if let Some(v) = tier_number(unit, "time") {
                    seconds = v;
                }
            }
        }

        let count_text = if by_formula {
            String::from("by formula")
        } else {
            (lang.format_amount)(count)
        };
        // THE EMPTY LIST IS NOT SPELLED WITH THE WORD THE FIELD REFUSES. The
        // renderer's answer for an empty list is the language's reserved word,
        // and in an INGREDIENT field that word is legal and means "the mod's own
        // choice"; in a PACK field the language refuses it, and testdata's
        // corpus pins the sentence that does the refusing. Printing it here
        // would be inviting the player to paste back into the field the one text
        // it will not take. The renderer and the corpus are the language's
        // contract and neither moves for this: the substitution is the LINE'S,
        // at the line's own composer, and nothing reads it back.
        let packs_text = if packs.is_empty() {
            String::from("no science pack")
        } else {
            (lang.render_list)(&resolved_pack_list(&packs))
        };
        let mut line = format!(
            "fkrecipes: {} takes its research cost from {}: count {}, time {}, packs {}",
            t.emitted_name(prefix),
            full,
            count_text,
            (lang.format_amount)(seconds),
            packs_text
        );
        if has_tier {
            line.push_str(&format!(
                "; the {} choice {} supplies what the settings leave at default",
                dropdown, chosen
            ));
        }

        let unit = match tier_unit {
            // The count is emitted as the NUMBER the setting answered with, not
            // as an integer this library rounded: the engine's own field is a
            // double like every other, and the setting's declared bounds are
            // what keep it a whole one.
            None => {
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
                    kv("count", Value::Num(count)),
                    kv("time", Value::Num(seconds)),
                    kv("ingredients", Value::Arr(ings)),
                ])
            }
            // THE TIER'S UNIT WITH THE PLAYER'S FIELDS WRITTEN OVER IT, so a
            // count_formula, a max_level and anything else it carried survive a
            // player who moved one slider.
            Some(tier_unit) => {
                let mut unit = tier_unit.clone();
                if count_set {
                    unit = set_unit_field(unit, "count", Value::Num(count));
                    // ONE WRINKLE, AND THE ENGINE DECIDES IT: a unit carrying
                    // both a count and a count_formula is priced by the
                    // FORMULA, so the number the player typed would be read by
                    // nobody and nothing would say so. The formula goes, and
                    // the line says which setting took it.
                    if has_unit_field(&unit, "count_formula") {
                        unit = without_unit_field(unit, "count_formula");
                        res.logs.push(format!(
                            "fkrecipes: {}: {} replaces the count_formula the {} cost carries",
                            t.name, count_setting, chosen
                        ));
                    }
                }
                if seconds_set {
                    unit = set_unit_field(unit, "time", Value::Num(seconds));
                }
                if typed.is_some() {
                    let ings: Vec<Value> = packs
                        .iter()
                        .map(|p| {
                            Value::Arr(vec![
                                Value::Str(p.name.clone()),
                                Value::Num(p.amount as f64),
                            ])
                        })
                        .collect();
                    unit = set_unit_field(unit, "ingredients", Value::Arr(ings));
                }
                unit
            }
        };
        res.logs.push(line);
        Some(unit)
    }

    /// One of a custom cost's two numbers, held to what the engine takes,
    /// FALLING BACK to the setting's declared default rather than refusing.
    ///
    /// A NUMBER IS A FIELD THE PLAYER OWNS, exactly as the pack text beside it
    /// is, so it takes the same rule: see `player_fallback` for the client
    /// measurement that decided it. The declared minima and the engine's own
    /// reset rule keep a player from producing one of these through the
    /// settings screen, and a `World` is still a trait: a fixture that answers
    /// a NaN used to reach the amount formatter, and one that answers an
    /// infinity used to be rendered into the unit. The line names the SETTING,
    /// because that is the field somebody would go and fix.
    ///
    /// THE DEFAULT IS THE SAME ONE AN UNREADABLE SETTING TAKES, which is what
    /// makes this one shape rather than two: `read_num_setting` already answers
    /// with `def_num` for a value it cannot read, and this answers with it for
    /// a value it cannot use.
    ///
    /// ONLY A VALUE THE SETTING ANSWERED CAN FALL BACK, which is what the third
    /// return of `read_num_setting` is for. An unreadable setting was never
    /// HOLDING anything, so a declared default the engine would not take is an
    /// author bug the post-condition names as one; a fallback line there would
    /// send a player to a field whose stored value was never the problem.
    fn read_cost_number(
        &self,
        w: &dyn World,
        res: &mut Resolution,
        prefix: &str,
        index: usize,
        fault: fn(f64) -> NumberFault,
        tgt: NoteTarget,
    ) -> (f64, String) {
        let (v, full, held) = self.read_num_setting(w, res, prefix, index);
        if !held {
            return (v, full);
        }
        let f = fault(v);
        if f != NumberFault::None {
            res.note_fallback(
                tgt,
                &full,
                number_fallback(&stored_number_problem(&full, f)),
                // A REPRICED RESEARCH DESTROYS NOTHING, so the note carries no
                // recipe-change sentence. See `Resolution::note_on`.
                false,
            );
            return (self.settings[index - 1].def_num, full);
        }
        (v, full)
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
    ) -> (f64, String, bool) {
        let s = &self.settings[index - 1];
        let full = s.emitted_name(prefix);
        match w.startup_setting(&full) {
            Some(Value::Num(n)) => (n, full, true),
            _ => {
                res.logs.push(format!(
                    "fkrecipes: the setting {} was not readable, so its default applies",
                    full
                ));
                (s.def_num, full, false)
            }
        }
    }

    fn resolve_ingredients(
        &self,
        w: &dyn World,
        res: &mut Resolution,
        tgt: NoteTarget,
        prefix: &str,
        recipe: &str,
        ings: &[Ingredient],
    ) -> Vec<ResolvedIngredient> {
        let mut list = Vec::with_capacity(ings.len());
        for ing in ings {
            if ing.candidates.is_empty() {
                // A HANDLE IS ITS OWN HEAD: no ladder, so the rung the fluid
                // sentences name is the name itself. The clone buys the Go
                // mirror's exact argument on a path only a plan's own item
                // takes, and only the fluid arms ever read it, which a handle
                // cannot reach (this library declares items, never fluids).
                let name = self.items[ing.item.index - 1].emitted_name(prefix);
                let from = name.clone();
                merge_ingredient(res, tgt, &mut list, recipe, name, &from, ing.amount);
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
                Some(name) => merge_ingredient(
                    res,
                    tgt,
                    &mut list,
                    recipe,
                    name,
                    &ing.candidates[0],
                    ing.amount,
                ),
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

/// Adds one resolved ingredient to a list that may already carry its name, and
/// is the reason nothing this library emits can name the same item, or the
/// same fluid, twice.
///
/// MEASURED, and it is what this exists for: a recipe whose ingredient list
/// names one item twice refuses the WHOLE LOAD with "Error while running setup
/// for recipe prototype \"bbb-balancer-part\" (recipe): Duplicate item
/// ingredients are not allowed (iron-plate exists 2 or more times)", exit 1, no
/// dump, no line naming a setting or a missing item. A ladder is exactly what
/// produces one: the pilot's ladders all end on iron-plate, so a mod set
/// without transport-belt resolves a third rung onto a name the list already
/// carries and a player who never opened the settings cannot load the game.
///
/// THE AMOUNTS ARE ADDED, IN THE POSITION OF THE FIRST OCCURRENCE, so
/// declaration order survives the merge: the entry is written back where it
/// already sat and nothing is moved.
///
/// AN ITEM AND A FLUID OF ONE NAME DO NOT MERGE, because the kind is part of
/// the identity: the engine takes both in one recipe (measured: base carries
/// parameter-0 to parameter-9 as an item AND a fluid), and adding a count to
/// an amount would be adding two different things. The mismatched pair falls
/// through to the next entry rather than answering, IN EITHER ORDER, which is
/// what lets a list hold one of each whichever the author declared first.
///
/// ONLY A LADDER COLLAPSE REACHES HERE. A list that named one thing twice in
/// the declaration is refused by
/// [`Lib::validate_no_duplicates`](crate::Lib) before any of this runs, which
/// is what makes "after the fallbacks" true in every sentence below.
///
/// `from` is the FIRST RUNG of the ladder being added, and it is what the
/// fluid sentences carry in place of the numbers they cannot print: without it
/// a three-way collapse writes the same line twice and names no declaration
/// the author could go and change.
fn merge_ingredient(
    res: &mut Resolution,
    tgt: NoteTarget,
    list: &mut Vec<ResolvedIngredient>,
    subject: &str,
    name: String,
    from: &str,
    amount: Amount,
) {
    for e in list.iter_mut() {
        if e.name != name {
            continue;
        }
        match (e.amount, amount) {
            (Amount::Item(a), Amount::Item(b)) => {
                res.logs.push(merged_item_line(subject, &name, a, b));
                let mut sum = a.saturating_add(b);
                if sum > MAX_ITEM_AMOUNT {
                    res.logs.push(merged_item_clamp(subject, &name, a, b));
                    res.note_on(tgt, with_destruction(clamped_item_note(&name), true));
                    sum = MAX_ITEM_AMOUNT;
                }
                e.amount = Amount::Item(sum);
                return;
            }
            (Amount::Fluid(a), Amount::Fluid(b)) => {
                res.logs.push(merged_fluid_line(subject, &name, from));
                let mut sum = a + b;
                // THE CEILING IS RE-ASKED HERE AND NOWHERE ELSE. Both amounts
                // crossed validate_ingredients on their own and both were
                // legal; their sum is a number no author wrote, and above the
                // engine's wall it does not refuse, it ABORTS (see
                // MAX_FLUID_AMOUNT).
                //
                // AND IT CLAMPS RATHER THAN REFUSING. Which rungs the ladders
                // landed on is a fact about the mod set, so this is an
                // ENVIRONMENTAL check: a modpack that removed two first rungs
                // must not stop the load on a number arithmetic can pin at the
                // ceiling the engine itself takes.
                if sum > MAX_FLUID_AMOUNT {
                    res.logs.push(merged_fluid_clamp(subject, &name, from));
                    res.note_on(tgt, with_destruction(clamped_fluid_note(&name), true));
                    sum = MAX_FLUID_AMOUNT;
                }
                e.amount = Amount::Fluid(sum);
                return;
            }
            // THE MISMATCHED PAIR, IN BOTH ORDERS. An item arriving at a fluid
            // of the same name and a fluid arriving at an item are one rule,
            // and this arm is the whole of it: adding a count to an amount
            // would be adding two different things, so the walk carries on to
            // the next entry and the list keeps one of each.
            (Amount::Item(_), Amount::Fluid(_)) | (Amount::Fluid(_), Amount::Item(_)) => continue,
        }
    }
    list.push(ResolvedIngredient { name, amount });
}

/// `merge_ingredient` for a science pack, whose subject is the technology and
/// whose kind is never in question: the ladder asks `tool_exists`, a tool is an
/// item, so a pack list has one namespace and the names alone decide identity.
fn merge_pack(
    res: &mut Resolution,
    tgt: NoteTarget,
    list: &mut Vec<ResolvedPack>,
    subject: &str,
    p: ResolvedPack,
) {
    for e in list.iter_mut() {
        if e.name != p.name {
            continue;
        }
        res.logs
            .push(merged_item_line(subject, &p.name, e.amount, p.amount));
        // THE ITEM CEILING, ON A NUMBER NO AUTHOR DECLARED, and it is the
        // engine's own rule for a unit ingredient too. MEASURED on 2.0.77: a
        // technology whose unit ingredients carry 65535 loads and dumps, and
        // 65536, 2^31 and 2^53 each refuse with "The data type allows values
        // from 0 to 65535" at ROOT.technology.<name>.unit.ingredients[0][1].
        // `validate_unit` holds a DECLARED pack to the same number, so this is
        // the only pack amount left that no author wrote, and it is CLAMPED
        // rather than refused for the reason `merge_ingredient`'s twin is. A
        // research costs no assembling machine anything, so the note carries no
        // destruction sentence.
        let mut sum = e.amount.saturating_add(p.amount);
        if sum > MAX_ITEM_AMOUNT {
            res.logs
                .push(merged_item_clamp(subject, &p.name, e.amount, p.amount));
            res.note_on(tgt, clamped_item_note(&p.name));
            sum = MAX_ITEM_AMOUNT;
        }
        e.amount = sum;
        return;
    }
    list.push(p);
}

/// The clause every merge sentence opens with, so a reader who meets the merge
/// line and the clamp line beside it recognises the pair.
fn merged_opening(subject: &str, name: &str) -> String {
    format!(
        "fkrecipes: {}: {} is in the list twice after the fallbacks",
        subject, name
    )
}

/// AN ITEM PRINTS ITS NUMBERS AND A FLUID NAMES THE RUNG INSTEAD. Rendering a
/// fluid amount is the ingredient language's job (`format_amount`), and the
/// language is reached only through the two text-setting constructors so that a
/// plan declaring no text setting links none of it; a call from here would link
/// it into every consumer, which `tests::source` refuses by name. So the fluid
/// sentences carry the first rung of the ladder that landed on the name: it
/// costs no formatter, it is what an author goes and edits, and it is what
/// makes two lines of a three-way collapse different lines.
///
/// AND DO NOT REACH FOR THE STANDARD LIBRARY HERE. MEASURED on this machine:
/// `format!("{}", 5e300f64)` is a 5 with 300 zeros after it and
/// `format!("{:e}", 5e300f64)` is "5e300", while Go's
/// `strconv.FormatFloat(5e300, 'g', -1, 64)` is "5e+300". No two of those are
/// the same string, so a fluid amount printed the convenient way in each half
/// splits the mirror on a line that is compared byte for byte. `format_amount`
/// exists because of exactly this, and it is the one thing this file may not
/// call.
///
/// THE SUM SATURATES RATHER THAN WRAPPING, and the reason is THIS half rather
/// than the answer. Nothing about the answer needs it: every declared amount is
/// validated at 1 or more, the running sum only ever grows, and
/// `Resolution::refuse` keeps the FIRST sentence it was handed, so the first sum
/// over the ceiling is what the plan is refused with (measured in the Go mirror,
/// where replacing its saturating helper with `a + b` changes no output at all).
/// What it is for is that resolution keeps merging AFTER a refusal is recorded,
/// and an unbounded sum here is a DEBUG PANIC rather than a wrap. MEASURED, with
/// this round's declared-pack ceiling taken back out: 1025 rungs of 2^53 landing
/// on one pack panic with "attempt to add with overflow".
///
/// AND THAT CASE IS NOW UNREACHABLE, which is why this stays rather than growing
/// a test. Every amount that reaches a merge is at most 65535, held there by
/// `validate_ingredients`, by `validate_unit` and, for a packs setting's
/// declared default, by the language's own round trip; overflowing an i64 at
/// that size takes about 1.4e14 rungs in one list. The saturation is the guard
/// that keeps the two halves computing the same number, and the one that keeps a
/// debug build standing if a declared amount is ever admitted above the ceiling
/// again.
fn merged_item_line(subject: &str, name: &str, a: i64, b: i64) -> String {
    format!(
        "{}, so the amounts are added: {} plus {} is {}",
        merged_opening(subject, name),
        a,
        b,
        a.saturating_add(b)
    )
}

/// See the note above: the rung is here in place of the two amounts, and
/// printing them instead is what may not be done.
fn merged_fluid_line(subject: &str, name: &str, from: &str) -> String {
    format!(
        "{}, so the amounts are added{}",
        merged_opening(subject, name),
        merged_from(from)
    )
}

fn merged_item_clamp(subject: &str, name: &str, a: i64, b: i64) -> String {
    format!(
        "{}, and {} plus {} is above the item ceiling of {}, so it is capped there",
        merged_opening(subject, name),
        a,
        b,
        MAX_ITEM_AMOUNT
    )
}

/// The same, for the same reason: the ceiling is a constant this file may
/// spell, and the amount that crossed it is not.
fn merged_fluid_clamp(subject: &str, name: &str, from: &str) -> String {
    format!(
        "{}, and the added amount is above the fluid ceiling of 1e301, so it is capped there{}",
        merged_opening(subject, name),
        merged_from(from)
    )
}

/// What a recipe whose resolved list names its own product says, and it is a
/// LOG LINE rather than a refusal.
///
/// THE SHAPE IS LEGAL AND THE BASE GAME SHIPS IT. MEASURED on 2.0.77 (build
/// 84539, mac-arm64, steam), base alone: kovarex-enrichment-process takes 40
/// uranium-235 and 5 uranium-238 and gives back 41 uranium-235 and 2
/// uranium-238, and coal-liquefaction takes 25 heavy-oil and gives back 90. A
/// sweep of the same dump found exactly those two among base's 217 recipes, so
/// a library that refused this would be refusing something the game itself
/// does.
///
/// IT IS NOT AN `fkrecipes: ERROR: ` LINE EITHER. That prefix belongs to a
/// stored value the library set aside; nothing is set aside here, and the plan
/// emits exactly what it resolved.
///
/// WHAT IT IS FOR IS THE SIGNAL. A player's typed text and an author's declared
/// ladder can both land on the product, because the text world overlays the
/// plan's own items and a ladder's last rung is whatever the author wrote. The
/// result loads and then does nothing anybody expects: an assembler fed the
/// recipe consumes the product to make the product, and the first one has to
/// come from somewhere else entirely.
fn self_product_line(subject: &str, name: &str) -> String {
    format!(
        "fkrecipes: {}: {} is in the list and is also what this recipe makes, so nothing can craft the first one unless something else produces it",
        subject, name
    )
}

/// The clause both fluid sentences end with, naming the ladder whose collapse
/// produced the second occurrence.
fn merged_from(from: &str) -> String {
    format!("; the ladder from {} resolved onto it", from)
}

/// Walks every pack's ladder and reports what the game actually has.
///
/// A PACK IS DROPPED, NOT REFUSED, exactly as an ingredient is, and the log
/// line is the ingredient's line with one word changed: the author who wrote
/// a ladder asked for tolerance, and a modpack that renamed the science packs
/// is the case the ladder is for. The unit that ends up with nothing is emitted
/// EMPTY, with a line and a tooltip of its own: see `Resolution::packless_at`.
///
/// THROUGH `tool_exists`, NEVER `item_exists`: the engine takes tool-type
/// items in a research unit and nothing else (measured: "Invalid research
/// unit (iron-plate). Research unit(s) can only be tool type items at the
/// moment."), so a rung that is an item but not a tool is not a rung.
/// IT ALSO ANSWERS WHAT IT TRIED, in declaration order, because the line a unit
/// that kept nothing gets has to name the names: see
/// `Resolution::packless_at`. REPEATS ARE LEFT IN, because this walk is one of
/// four callers feeding that sentence and none of them can see the others:
/// `packless_at` is the one place the once-each rule is applied. Collected on
/// every walk rather than only on the empty one, so the answer costs the same
/// branch whatever the game holds.
fn resolve_packs(
    w: &dyn World,
    res: &mut Resolution,
    tgt: NoteTarget,
    tech: &str,
    packs: &[Pack],
) -> (Vec<ResolvedPack>, Vec<String>) {
    let mut list = Vec::with_capacity(packs.len());
    let mut tried: Vec<String> = Vec::with_capacity(packs.len());
    for p in packs {
        let mut picked: Option<String> = None;
        if w.tool_exists(&p.name) {
            picked = Some(p.name.clone());
        } else {
            tried.push(p.name.clone());
            for f in &p.fallbacks {
                if w.tool_exists(f) {
                    picked = Some(f.clone());
                    break;
                }
                tried.push(String::from(f));
            }
        }
        match picked {
            // TWO LADDERS CAN LAND ON ONE PACK, exactly as two ingredient
            // ladders can, and the emitted form here is the SHORT TUPLE rather
            // than the recipe's dict. See merge_pack.
            Some(name) => merge_pack(
                res,
                tgt,
                &mut list,
                tech,
                ResolvedPack {
                    name,
                    amount: p.amount,
                },
            ),
            None => res.logs.push(format!(
                "fkrecipes: {}: none of {} is present, so the science pack is dropped",
                tech,
                pack_ladder(p)
            )),
        }
    }
    (list, tried)
}

/// Keeps a vector in first-seen order with no repeats. Its one caller is
/// `packless_at`, which is where the reason it is needed is written.
fn append_once(list: &mut Vec<String>, name: &str) {
    if list.iter().any(|seen| seen == name) {
        return;
    }
    list.push(String::from(name));
}

/// What a verbatim-copied research unit came to after every science pack this
/// game does not have was taken out of it.
///
/// A COPIED UNIT IS SOMEBODY ELSE'S DECLARATION AND THE ENGINE DOES NOT FORGIVE
/// IT. MEASURED on 2.0.77 build 84539, headless: a unit priced in a plain item
/// refuses the load with `Error while running setup for technology prototype
/// "tprobe-t" (technology): Invalid research unit (iron-plate). Research
/// unit(s) can only be tool type items at the moment.`, and a name the game
/// does not have at all fails earlier and more coarsely, naming neither the
/// technology nor the property: `Error in assignID: item with name 'water' does
/// not exist.` So a pack that a modpack demoted from tool to item, or removed,
/// stops the load on the DEFAULT setting with no `fkrecipes: ` line anywhere.
/// The filter asks BOTH questions at once, because `tool_exists` is the one
/// probe that answers them: a name that is not a tool-type item this game has
/// is not a rung.
struct CopiedPacks {
    /// The copied unit with its ingredients array filtered, the rest of it in
    /// the order and the shape it arrived in.
    unit: Value,
    /// Whether the unit carried an ingredients array at all. A unit that
    /// carries none is left exactly as it was: there is nothing to filter and
    /// nothing to say.
    has_list: bool,
    /// How many packs survived, and the names that did not, in the copied
    /// unit's own order.
    kept: usize,
    dropped: Vec<String>,
    /// An entry in NEITHER engine form, which is a unit this library cannot
    /// copy faithfully. A form it cannot decode is not a licence to pass it
    /// through: the name might be one the game does not have, and the refusal
    /// it would earn names neither the technology nor the property.
    unreadable: bool,
}

/// Drops the science packs a copied research unit names that this game does not
/// have, one log line each, and answers what became of it.
///
/// THE DROP LINE IS THE LADDER'S VOICE with the source named, because the
/// author did not write this list: the technology it was copied out of did.
///
/// BOTH ENGINE FORMS ARE DECODED. The engine takes the short tuple
/// `{"automation-science-pack", 1}` and the long `{name = ..., amount = ...}`
/// alike and base writes the short one; only the NAME is read out, and the
/// entry itself is what is kept, so an amount this library does not model and a
/// field no version of it has heard of survive the filter untouched.
fn filter_copied_packs(
    res: &mut Resolution,
    w: &dyn World,
    tech: &str,
    source: &str,
    unit: Value,
) -> CopiedPacks {
    let pairs = match &unit {
        Value::Map(pairs) => pairs.clone(),
        _ => {
            return CopiedPacks {
                unit,
                has_list: false,
                kept: 0,
                dropped: Vec::new(),
                unreadable: false,
            }
        }
    };
    for (k, v) in &pairs {
        if k.as_str() != "ingredients" {
            continue;
        }
        let items = match v {
            Value::Arr(items) => items,
            // AN `ingredients` KEY THAT IS NOT AN ARRAY IS A FORM THIS LIBRARY
            // CANNOT DECODE, and a form it cannot decode is not a licence to
            // pass it through: the caller degrades on it, exactly as it does for
            // an ENTRY in neither form, to the declared cost behind a tier or to
            // an emptied unit where there is none. What would otherwise happen
            // is worse: the unit crosses unfiltered and the engine answers about
            // a science pack, naming neither this mod nor the property.
            _ => {
                return CopiedPacks {
                    unit,
                    has_list: true,
                    kept: 0,
                    dropped: Vec::new(),
                    unreadable: true,
                }
            }
        };
        let mut kept: Vec<Value> = Vec::with_capacity(items.len());
        let mut dropped: Vec<String> = Vec::new();
        for item in items {
            let name = match copied_pack_name(item) {
                Some(n) => n,
                None => {
                    return CopiedPacks {
                        unit,
                        has_list: true,
                        kept: 0,
                        dropped,
                        unreadable: true,
                    }
                }
            };
            // A NAME THIS LIBRARY CANNOT PUT TO THE WORLD IS KEPT UNASKED, and
            // the empty name is that answer. Another mod's science pack can be
            // named with bytes that are not UTF-8; fkdata hands those over
            // unchanged, they arrive as `Value::Bytes`, and `tool_exists` takes
            // a `&str`, so this half cannot ask. The Go mirror could, and does
            // not, because the two halves answer alike.
            if name.is_empty() {
                kept.push(item.clone());
                continue;
            }
            if w.tool_exists(&name) {
                kept.push(item.clone());
                continue;
            }
            res.logs.push(format!(
                "fkrecipes: {}: {} is not a science pack this game has, so it is left out of the {} cost",
                tech, name, source
            ));
            dropped.push(name);
        }
        return CopiedPacks {
            kept: kept.len(),
            unit: set_unit_field(unit, "ingredients", Value::Arr(kept)),
            has_list: true,
            dropped,
            unreadable: false,
        };
    }
    CopiedPacks {
        unit,
        has_list: false,
        kept: 0,
        dropped: Vec::new(),
        unreadable: false,
    }
}

/// The name out of one entry of a copied unit's ingredients, in either of the
/// two forms the engine takes.
///
/// THREE ANSWERS IN ONE OPTION. `None` is an entry in NEITHER form, which the
/// caller degrades on; the EMPTY name is an entry in a form whose name is not text
/// this library can put to the World, which the caller keeps unasked. No
/// prototype is named by the empty string, so the sentinel names nothing real.
fn copied_pack_name(v: &Value) -> Option<String> {
    match v {
        Value::Arr(items) => match items.first() {
            Some(Value::Str(name)) => Some(name.clone()),
            Some(Value::Bytes(_)) => Some(String::new()),
            _ => None,
        },
        Value::Map(pairs) => {
            for (k, val) in pairs {
                if k == "name" {
                    return match val {
                        Value::Str(name) => Some(name.clone()),
                        Value::Bytes(_) => Some(String::new()),
                        _ => None,
                    };
                }
            }
            None
        }
        _ => None,
    }
}

/// The FACT both lines about an undecodable copied unit open with, and it is
/// one composer because the two differ only in what the library did next. It
/// keeps the `cost_of` family's own wording, because it is the same fact about
/// the same value: a table that cannot be copied faithfully.
///
/// IT NAMES THE TECHNOLOGY RATHER THAN WEARING THE `CostOf(` PREFIX, because
/// the same filter runs over a `cost_by` tier's chosen source, where there is
/// no `CostOf` to name.
fn unreadable_unit_phrase(tech: &str, source: &str) -> String {
    format!(
        "{}: the unit of {} holds a table this library cannot copy faithfully",
        tech, source
    )
}

/// What an undecodable copied unit logs where there IS a declared cost behind
/// it, and [`unreadable_copy_line`] what it logs where there is not. Both are
/// ERROR lines because the technology is not priced the way anybody declared
/// it, and neither goes through `player_fallback`: nothing was stored and
/// nothing was typed, so there is no field to send anybody to.
fn unreadable_source_line(tech: &str, source: &str) -> String {
    format!(
        "{}ERROR: {}, so this mod's own declared cost applies instead",
        MESSAGE_PREFIX,
        unreadable_unit_phrase(tech, source)
    )
}

fn unreadable_copy_line(tech: &str, source: &str) -> String {
    format!(
        "{}ERROR: {}, so the research is emitted with no science pack and completes for free",
        MESSAGE_PREFIX,
        unreadable_unit_phrase(tech, source)
    )
}

/// The two lines and the two notes a dropped edge earns. They are ERROR lines
/// because the technology is not placed the way anybody declared it, and they
/// are NOT a player's fallback: nothing was stored and nothing was typed, so
/// there is no field on the settings screen to send anybody to.
///
/// THE WHOLE RING IS IN THE LINE AND NOT IN THE NOTE. An author reading the log
/// needs the path to see which mod closed it; a player hovering a technology
/// cannot act on a hundred prototype names, and the ceiling every composition
/// is held to (200 bytes per element, measured on 2.0.77) is a further reason
/// not to put one there.
pub(crate) fn cycle_prereq_line(tech: &str, name: &str, path: &[String]) -> String {
    format!(
        "{}ERROR: {}: requiring {} would loop this game's technology tree ({}), so the prerequisite is dropped",
        MESSAGE_PREFIX,
        tech,
        name,
        render_cycle_path(path)
    )
}

pub(crate) fn cycle_prereq_note(name: &str) -> String {
    format!(
        "Requiring {} would loop this game's technology tree, so this research was left without that prerequisite. The reason is in the log.",
        name
    )
}

pub(crate) fn cycle_splice_line(tech: &str, before: &str, path: &[String]) -> String {
    format!(
        "{}ERROR: {}: making it a prerequisite of {} would loop this game's technology tree ({}), so the splice is dropped",
        MESSAGE_PREFIX,
        tech,
        before,
        render_cycle_path(path)
    )
}

pub(crate) fn cycle_splice_note(before: &str) -> String {
    format!(
        "Making this research a prerequisite of {} would loop this game's technology tree, so it was left out of it. The reason is in the log.",
        before
    )
}

/// What a technology priced in no science pack at all logs, naming every rung
/// the walk asked the game about.
///
/// THE NAMES ARE PART OF THE ANSWER. "this research names no science pack this
/// game has" on its own tells an author that something is absent and not which
/// thing, and the author reading it is one whose ladders all missed: the names
/// are the rungs they wrote and the one thing that says which mod set this is.
fn packless_line(tech: &str, names: &[String]) -> String {
    format!(
        "{}ERROR: {}: none of {} is a science pack this game has, so the research is emitted with no science pack and completes for free",
        MESSAGE_PREFIX,
        tech,
        names.join(", ")
    )
}

/// What a copied cost that named no science pack this game has logs, and it is
/// an ERROR because the technology is not priced the way anybody declared it.
///
/// IT IS NOT A PLAYER'S FALLBACK AND HAS ITS OWN COMPOSER FOR THAT REASON.
/// Nothing was stored and nothing was typed: there is no field on the settings
/// screen to send anybody to, so `player_fallback`'s tail would be advice about
/// a value that does not exist. The two cannot drift, because neither reads the
/// other.
fn packless_source_line(tech: &str, source: &str) -> String {
    format!(
        "{}ERROR: {}: the {} cost names no science pack this game has, so this mod's own declared cost applies instead",
        MESSAGE_PREFIX, tech, source
    )
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

/// A list the player wrote, in the typed order, ready for a recipe. The
/// language has already resolved every name against the overlay, so nothing
/// here walks a ladder or drops an entry.
fn typed_ingredients(list: &IngredientList) -> Vec<ResolvedIngredient> {
    list.entries
        .iter()
        .map(|e| ResolvedIngredient {
            name: e.name.clone(),
            amount: e.amount,
        })
        .collect()
}

/// One numeric field of a tier's unit map.
///
/// A SLICE AND A SCAN, like every other lookup in this crate: a unit is a
/// handful of fields and nothing here may depend on an iteration order.
fn tier_number(unit: &Value, key: &str) -> Option<f64> {
    let pairs = match unit {
        Value::Map(pairs) => pairs,
        _ => return None,
    };
    for (k, v) in pairs {
        if k == key {
            if let Value::Num(n) = v {
                return Some(*n);
            }
        }
    }
    None
}

/// Whether a tier's unit carries a field at all, whatever its shape.
/// `count_formula` is a STRING in every unit the engine ships, so the numeric
/// reader above cannot answer this question.
fn has_unit_field(unit: &Value, key: &str) -> bool {
    match unit {
        Value::Map(pairs) => pairs.iter().any(|(k, _)| k == key),
        _ => false,
    }
}

/// Replaces a field of a tier's unit IN PLACE IN THE ORDER IT ALREADY HAD, or
/// appends it at the end when the unit does not carry one.
///
/// THE ORDER IS PART OF THE EMITTED VALUE, so a field that moved would be a
/// prototype that differs between a plan that overrode it and one that did not,
/// and the two halves would have to agree about the move as well as about the
/// value.
fn set_unit_field(unit: Value, key: &str, val: Value) -> Value {
    let pairs = match unit {
        Value::Map(pairs) => pairs,
        other => {
            let _ = other;
            return Value::Map(vec![kv(key, val)]);
        }
    };
    let mut out = Vec::with_capacity(pairs.len() + 1);
    let mut replaced = false;
    for (k, v) in pairs {
        if k == key {
            out.push((String::from(key), val.clone()));
            replaced = true;
            continue;
        }
        out.push((k, v));
    }
    if !replaced {
        out.push((String::from(key), val));
    }
    Value::Map(out)
}

/// Drops a field of a tier's unit, keeping the rest in order.
fn without_unit_field(unit: Value, key: &str) -> Value {
    match unit {
        Value::Map(pairs) => Value::Map(pairs.into_iter().filter(|(k, _)| k != key).collect()),
        other => other,
    }
}

/// A tier unit's science packs, so a cost line can say what the tier is paying
/// with.
///
/// BOTH SPELLINGS, because a unit this library copies is somebody else's
/// declaration: the engine takes the short tuple `{"name", amount}` and the
/// long `{name = ..., amount = ...}` alike, and base writes the short one. An
/// entry in neither shape is skipped rather than guessed at; it is the tier's
/// own ingredients that are emitted, so nothing this reader misses changes the
/// prototype, only the line that describes it.
fn tier_pack_list(unit: &Value) -> Vec<ResolvedPack> {
    let mut out = Vec::new();
    let pairs = match unit {
        Value::Map(pairs) => pairs,
        _ => return out,
    };
    for (k, v) in pairs {
        if k != "ingredients" {
            continue;
        }
        let items = match v {
            Value::Arr(items) => items,
            _ => continue,
        };
        for item in items {
            if let Some(p) = tier_pack(item) {
                out.push(p);
            }
        }
    }
    out
}

fn tier_pack(v: &Value) -> Option<ResolvedPack> {
    if let Value::Arr(items) = v {
        if items.len() >= 2 {
            if let (Value::Str(name), Value::Num(amount)) = (&items[0], &items[1]) {
                return Some(ResolvedPack {
                    name: name.clone(),
                    amount: *amount as i64,
                });
            }
        }
        return None;
    }
    let pairs = match v {
        Value::Map(pairs) => pairs,
        _ => return None,
    };
    let mut name = None;
    let mut amount = None;
    for (k, val) in pairs {
        if k == "name" {
            if let Value::Str(n) = val {
                name = Some(n.clone());
            }
        }
        if k == "amount" {
            if let Value::Num(n) = val {
                amount = Some(*n as i64);
            }
        }
    }
    match (name, amount) {
        (Some(name), Some(amount)) => Some(ResolvedPack { name, amount }),
        _ => None,
    }
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
