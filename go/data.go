package fkrecipes

import (
	"errors"
	"strconv"
	"strings"
)

// PlanData turns the declared items, recipes and technologies into an Op
// stream: validate first and refuse with the FIRST problem found, then resolve
// what the game actually has, then emit.
//
// The stream's order is fixed, because the two language halves are compared
// through it: every degradation log at the point the planner decided it, then
// the item prototypes in declaration order, then the recipes, then the
// technologies, then the prerequisite splices into other mods' technologies.
//
// This is the seam the emit layer stands on at the data stage; consumers call
// Emit and never this.
func (l *Lib) PlanData(w World) ([]Op, error) {
	// A nil World is a Go-only hazard: the Rust mirror takes &dyn World,
	// which cannot be null, so it needs no guard. Here the alternative is a
	// nil dereference inside somebody's data stage.
	if w == nil {
		return nil, errors.New("fkrecipes: PlanData was given a nil World")
	}
	if l.id == 0 {
		return nil, errors.New("fkrecipes: this Lib was built without New, so its handles cannot be validated")
	}
	modName := w.ModName()
	if modName == "" {
		return nil, errors.New("fkrecipes: the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName")
	}
	prefix := modName + "-"

	if err := l.validate(w, prefix); err != nil {
		return nil, err
	}
	res := l.resolve(w, prefix)
	if err := l.checkResolvedCraftTimes(res); err != nil {
		return nil, err
	}
	if err := checkResolvedPacks(res); err != nil {
		return nil, err
	}
	if err := l.checkCycles(w, res, prefix); err != nil {
		return nil, err
	}

	ops := make([]Op, 0, len(res.logs)+len(l.items)+len(l.recipes)+2*len(l.techs))
	for _, line := range res.logs {
		ops = append(ops, logOp(line))
	}
	for _, it := range l.items {
		ops = append(ops, extendOp(itemProto(prefix, it)))
	}
	unlocked := l.unlockedRecipes()
	for i, r := range l.recipes {
		ops = append(ops, extendOp(recipeProto(prefix, l, r, res.recipes[i], res.craftTimes[i], unlocked[i])))
	}
	for i, t := range l.techs {
		ops = append(ops, extendOp(techProto(prefix, l, w, t, res.techs[i])))
	}
	for i := range l.techs {
		rt := res.techs[i]
		if rt.rewrite == 0 {
			continue
		}
		rw := res.rewrites[rt.rewrite-1]
		// A Set op's value is ALWAYS a real value, never Nil: the emit layer
		// hands it to fkdata.Set, and a nil there DELETES the key rather than
		// writing one. Nothing plans a deletion today, and the invariant is
		// asserted in the pure half so it cannot start silently.
		ops = append(ops, setOp([]PathEl{pathKey("technology"), pathKey(rw.before), pathKey("prerequisites")}, strArr(rw.list)))
	}
	return ops, nil
}

// validate returns the first refusal in a fixed scan order: items, then
// recipes, then technologies, each in declaration order. The cycle overlay is
// checked separately, after resolution, because it needs to know which
// splices actually survived.
//
// Every handle is checked here, not where it is read: an index that reaches
// resolve unchecked is a panic in somebody's data stage, and a handle from
// another plan is in range for this one.
func (l *Lib) validate(w World, prefix string) error {
	at := "fkrecipes: "

	for i, it := range l.items {
		if it.name == "" {
			return errors.New(at + "an item was declared with an empty name")
		}
		// Compared on the EMITTED names, which is the namespace the engine
		// keeps: a legacy name and a generated one can arrive at the same
		// string from different declarations, and only one survives.
		for j := 0; j < i; j++ {
			if l.items[j].emittedName(prefix) == it.emittedName(prefix) {
				return errors.New(at + "two items share the name " + it.emittedName(prefix) + "; the second would overwrite the first")
			}
		}
		// V1 never rewrites another mod's prototype except to splice a
		// prerequisite, and the cycle overlay counts on every planned name
		// being new: two nodes with one name is a walk that misses the ring.
		if w.ItemExists(it.emittedName(prefix)) {
			return errors.New(at + "the item " + it.emittedName(prefix) + " already exists in data.raw; this plan would overwrite it")
		}
		if it.spec.StackSize < 0 {
			return errors.New(at + "the item " + it.name + " has a negative stack size, which the engine refuses")
		}
		if it.spec.StackSize > maxExactInt {
			return errors.New(at + "the item " + it.name + " declares a stack size a Lua double cannot hold exactly: " + strconv.FormatInt(it.spec.StackSize, 10))
		}
		if it.spec.IconSize < 0 {
			return errors.New(at + "the item " + it.name + " has a negative icon size, which the engine refuses")
		}
		if it.spec.IconSize > maxExactInt {
			return errors.New(at + "the item " + it.name + " declares an icon size a Lua double cannot hold exactly: " + strconv.FormatInt(it.spec.IconSize, 10))
		}
		if it.spec.PlaceResult != "" && !w.EntityExists(it.spec.PlaceResult) {
			return errors.New(at + "the item " + it.name + " names a place_result " + it.spec.PlaceResult + " that does not exist")
		}
		if err := checkExtra(at, "the item "+it.name, it.spec.Extra, itemOwnFields); err != nil {
			return err
		}
	}

	for i, r := range l.recipes {
		// The result handle first: a recipe with no result has no name to
		// report either, and "two recipes share the name" with an empty name
		// is a worse answer than the one that says what is actually wrong.
		switch {
		case r.result.index != 0:
			// A handle was given: it has to be one of this plan's.
			if !l.validItem(r.result) {
				return errors.New(at + "a recipe was declared with no result item; Recipe needs an item this plan declared")
			}
			if r.spec.ResultNamed != "" {
				return errors.New(at + "the recipe " + r.name + " names both a result item and ResultNamed; pick one")
			}
		case r.spec.ResultNamed != "":
			// THIS PLAN'S OWN ITEM FIRST, and the order is the whole point.
			// ResultNamed is probed against the World, and the World is
			// data.raw as it stands BEFORE this plan runs, so an item this
			// plan declares two lines up is not there yet and the probe would
			// refuse it as absent. That refusal is correct and its sentence is
			// not: the consumer can see the item. Compared on the EMITTED
			// names, so it catches a legacy declaration and a generated one
			// alike.
			for _, it := range l.items {
				if it.emittedName(prefix) == r.spec.ResultNamed {
					return errors.New(at + "the recipe " + r.name + " produces " + r.spec.ResultNamed +
						" through ResultNamed, which this plan declares; use the item's handle instead")
				}
			}
			// An existing item, so it is probed exactly as an ingredient is.
			if !w.ItemExists(r.spec.ResultNamed) {
				return errors.New(at + "the recipe " + r.name + " produces " + r.spec.ResultNamed + ", which does not exist")
			}
			if r.name == "" {
				return errors.New(at + "a recipe producing an existing item was declared with no name; there is no declared item to take one from")
			}
		default:
			return errors.New(at + "a recipe was declared with no result item; Recipe needs an item this plan declared")
		}
		// A recipe with no name of its own inherits the item's, which is the
		// common shape and stays legal. This fires only when that name is
		// empty too, which today the item check above has already caught: it
		// is the guard that keeps the two checks independent.
		if r.name == "" {
			return errors.New(at + "a recipe was declared with an empty name")
		}
		for j := 0; j < i; j++ {
			if l.recipes[j].emittedName(prefix) == r.emittedName(prefix) {
				return errors.New(at + "two recipes share the name " + r.emittedName(prefix) + "; the second would overwrite the first")
			}
		}
		if !finite(r.spec.CraftTime) {
			return errors.New(at + "the recipe " + r.name + " declares a crafting time that is not a finite number")
		}
		if r.spec.CraftTime != 0 && r.spec.CraftTimeFrom.index != 0 {
			return errors.New(at + "the recipe " + r.name + " names both CraftTime and CraftTimeFrom; pick one")
		}
		if err := checkExtra(at, "the recipe "+r.name, r.spec.Extra, recipeOwnFields); err != nil {
			return err
		}
		if len(r.spec.Ingredients) > 0 && r.spec.IngredientsBy != nil {
			return errors.New(at + "the recipe " + r.name + " names both Ingredients and IngredientsBy; pick one")
		}
		if r.spec.IngredientsBy != nil {
			by := r.spec.IngredientsBy
			if !l.validDropdownSetting(by.Setting) {
				return errors.New(at + "the recipe " + r.name + " names an ingredients setting that this plan never declared")
			}
			values := l.settings[by.Setting.index-1].values
			offered := make([]string, 0, len(by.Choices))
			for _, c := range by.Choices {
				offered = append(offered, c.Value)
			}
			if err := matchesAllowedValues(at, "the recipe "+r.name,
				l.settings[by.Setting.index-1].emittedName(prefix), offered, values); err != nil {
				return err
			}
			for _, c := range by.Choices {
				if err := l.validateIngredients(at, "the recipe "+r.name, r.spec.Category, c.Ingredients); err != nil {
					return err
				}
			}
		}
		if r.spec.CraftTimeFrom.index != 0 && !l.validDoubleSetting(r.spec.CraftTimeFrom) {
			return errors.New(at + "the recipe " + r.name + " names a crafting-time setting that this plan never declared")
		}
		// A zero crafting time means the engine's own default and is emitted
		// as no field at all; a negative one is a refusal, not a default.
		if r.spec.CraftTime < 0 {
			return errors.New(at + "the recipe " + r.name + " has a negative crafting time, which the engine refuses")
		}
		// Zero still means "say nothing and let the engine default apply". A
		// positive value below the floor is a load failure the consumer would
		// read as their own mod being broken, so it is refused here by name.
		if r.spec.CraftTime > 0 && r.spec.CraftTime <= craftTimeFloor {
			return errors.New(at + "the recipe " + r.name + " declares a crafting time the engine refuses (energy_required can't be <= 0.001)")
		}
		if r.spec.ResultCount < 0 {
			return errors.New(at + "the recipe " + r.name + " has a negative result count, which the engine refuses")
		}
		if r.spec.ResultCount > maxExactInt {
			return errors.New(at + "the recipe " + r.name + " declares a result count a Lua double cannot hold exactly: " + strconv.FormatInt(r.spec.ResultCount, 10))
		}
		if w.RecipeExists(r.emittedName(prefix)) {
			return errors.New(at + "the recipe " + r.emittedName(prefix) + " already exists in data.raw; this plan would overwrite it")
		}
		if err := l.validateIngredients(at, "the recipe "+r.name, r.spec.Category, r.spec.Ingredients); err != nil {
			return err
		}
	}

	for i, t := range l.techs {
		if t.name == "" {
			return errors.New(at + "a technology was declared with an empty name")
		}
		for j := 0; j < i; j++ {
			if l.techs[j].emittedName(prefix) == t.emittedName(prefix) {
				return errors.New(at + "two technologies share the name " + t.emittedName(prefix) + "; the second would overwrite the first")
			}
		}
		if w.TechExists(t.emittedName(prefix)) {
			return errors.New(at + "the technology " + t.emittedName(prefix) + " already exists in data.raw; this plan would overwrite it")
		}
		if err := checkExtra(at, "the technology "+t.name, t.spec.Extra, techOwnFields); err != nil {
			return err
		}
		hasCost := t.spec.CostOf != ""
		hasUnit := t.spec.Unit != nil
		hasCostBy := t.spec.CostBy != nil
		named := 0
		for _, set := range []bool{hasCost, hasUnit, hasCostBy} {
			if set {
				named++
			}
		}
		if named != 1 {
			return errors.New(at + "the technology " + t.name + " must name exactly one of CostOf, Unit or CostBy")
		}
		// CostBy carries the prerequisite with the unit, so it is the thing
		// that places the technology. A second placement would be a second
		// opinion about the same edge.
		if hasCostBy && (t.spec.After != "" || t.spec.Before != "" || t.spec.AfterTech.index != 0) {
			return errors.New(at + "the technology " + t.name + " names CostBy with a placement; the prerequisite moves with the unit, so CostBy places the technology itself")
		}
		if hasCostBy {
			by := t.spec.CostBy
			if !l.validDropdownSetting(by.Setting) {
				return errors.New(at + "the technology " + t.name + " names a cost setting that this plan never declared")
			}
			values := l.settings[by.Setting.index-1].values
			offered := make([]string, 0, len(by.Choices))
			for _, c := range by.Choices {
				offered = append(offered, c.Value)
			}
			if err := matchesAllowedValues(at, "the technology "+t.name,
				l.settings[by.Setting.index-1].emittedName(prefix), offered, values); err != nil {
				return err
			}
			if err := l.validateUnit(at, t.name, &by.Fallback); err != nil {
				return err
			}
		}
		if t.spec.After != "" && t.spec.AfterTech.index != 0 {
			return errors.New(at + "the technology " + t.name + " names both After and AfterTech; pick one anchor")
		}
		if t.spec.Before != "" && t.spec.AfterTech.index != 0 {
			return errors.New(at + "the technology " + t.name + " names Before with AfterTech; InsertBetween splices around a technology that already exists")
		}
		if t.spec.Before != "" && t.spec.After == "" {
			return errors.New(at + "the technology " + t.name + " names Before without After; InsertBetween needs both ends")
		}
		if t.spec.AfterTech.index != 0 && !l.validTech(t.spec.AfterTech) {
			return errors.New(at + "the technology " + t.name + " names an AfterTech technology that this plan never declared")
		}
		if t.spec.IconSize < 0 {
			return errors.New(at + "the technology " + t.name + " has a negative icon size, which the engine refuses")
		}
		if t.spec.IconSize > maxExactInt {
			return errors.New(at + "the technology " + t.name + " declares an icon size a Lua double cannot hold exactly: " + strconv.FormatInt(t.spec.IconSize, 10))
		}
		if hasUnit {
			if err := l.validateUnit(at, t.name, t.spec.Unit); err != nil {
				return err
			}
		} else if hasCost {
			if !w.TechExists(t.spec.CostOf) {
				return errors.New(at + "CostOf(" + t.spec.CostOf + "): no technology of that name exists")
			}
			if w.TechHasResearchTrigger(t.spec.CostOf) {
				return errors.New(at + "CostOf(" + t.spec.CostOf + "): " + t.spec.CostOf + " is a research_trigger technology with no unit to copy; name a unit-carrying technology instead")
			}
			u, ok := w.TechUnit(t.spec.CostOf)
			if !ok {
				return errors.New(at + "CostOf(" + t.spec.CostOf + "): " + t.spec.CostOf + " carries no unit to copy")
			}
			// The unit is copied verbatim into a prototype, so it has to BE a
			// prototype's field map. Anything else is another mod's mistake
			// arriving as this mod's load failure. "Dictionary", not "table":
			// a Lua sequence is a table as well, and an array-shaped unit is
			// exactly one of the things this refuses.
			if u.Kind != KindMap {
				return errors.New(at + "CostOf(" + t.spec.CostOf + "): " + t.spec.CostOf + " has a unit that is not a dictionary")
			}
			if holdsDroppedSubtree(u) {
				return errors.New(at + "CostOf(" + t.spec.CostOf + "): the unit of " + t.spec.CostOf + " holds a table this library cannot copy faithfully")
			}
			// The level cap rides along the same way and truncates the same
			// way: a map-shaped max_level that lost a subtree would be
			// emitted with a hole in it.
			if level, ok := w.TechMaxLevel(t.spec.CostOf); ok && holdsDroppedSubtree(level) {
				return errors.New(at + "CostOf(" + t.spec.CostOf + "): the max_level of " + t.spec.CostOf + " holds a table this library cannot copy faithfully")
			}
		}
		for _, u := range t.spec.Unlocks {
			if !l.validRecipe(u) {
				return errors.New(at + "the technology " + t.name + " unlocks a recipe that this plan never declared")
			}
		}
		if t.spec.EnabledBy.index != 0 && !l.validBoolSetting(t.spec.EnabledBy) {
			return errors.New(at + "the technology " + t.name + " names an EnabledBy setting that this plan never declared")
		}
	}
	return nil
}

// resolvedIngredient is one ingredient after the ladder answered: the name the
// game actually has, and the amount in the shape its kind takes.
type resolvedIngredient struct {
	kind   ingredientKind
	name   string
	amount int64   // kindItem
	fluid  float64 // kindFluid
}

// rewriteRec is one planned rewrite of another technology's prerequisite
// list. A second splice into the same technology builds on the first: two
// Set ops on one path would otherwise mean the last one silently undoes the
// earlier splice.
type rewriteRec struct {
	before string
	list   []string
}

type resolvedTech struct {
	// The cost this technology settled on, and the level cap that rode along
	// with it. Resolved rather than emitted straight from the spec, because
	// which source answered, and which rung of each pack's ladder the game
	// actually has, are facts about the game.
	//
	// hasUnit is false only for CostOf, whose unit is copied verbatim out of
	// the World at emit and so has nothing to resolve.
	unit        Value
	hasUnit     bool
	maxLevel    Value
	hasMaxLevel bool

	prereqs      []string
	hasEnabledBy bool
	on           bool
	rewrite      int // 1-based index into resolution.rewrites; 0 is none
}

// craftTime is what a bound recipe's energy_required resolved to, and which
// setting answered, so a refusal can name it.
type craftTime struct {
	bound   bool
	setting string
	value   float64
}

type resolution struct {
	logs       []string
	craftTimes []craftTime
	recipes    [][]resolvedIngredient
	techs      []resolvedTech
	rewrites   []rewriteRec

	// packless is the first technology, in declaration order, whose every
	// declared science pack dropped. It is carried out of resolve rather than
	// raised inside it because resolve answers with facts and PlanData decides
	// which of them is a refusal, exactly as the crafting-time floor does.
	packless string
}

// resolve asks the World everything the plan needs to know and records what
// degraded. The pass order IS the log order, and it is part of the contract
// the Rust mirror holds to: recipes in declaration order, then technologies
// in declaration order, and within a technology its enablement before its
// tree placement.
func (l *Lib) resolve(w World, prefix string) resolution {
	var res resolution

	for _, r := range l.recipes {
		// The crafting time first, then the ingredients: a recipe's own field
		// before what it is made of, mirroring a technology's enablement
		// before its tree placement.
		var ct craftTime
		if r.spec.CraftTimeFrom.index != 0 {
			setting := l.settings[r.spec.CraftTimeFrom.index-1]
			full := setting.emittedName(prefix)
			ct.bound = true
			ct.setting = full
			ct.value = setting.defNum
			if v, ok := w.StartupSetting(full); ok && v.Kind == KindNum {
				ct.value = v.Num
			} else {
				res.logs = append(res.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
			}
		}
		res.craftTimes = append(res.craftTimes, ct)

		declared := r.spec.Ingredients
		if by := r.spec.IngredientsBy; by != nil {
			setting := l.settings[by.Setting.index-1]
			chosen := res.readDropdown(w, setting, prefix)
			declared = choiceFor(by.Choices, chosen)
			list := l.resolveIngredients(w, &res, prefix, r.name, declared)
			// A plan that named things and got none of them is a recipe made
			// of nothing. The DEFAULT option is what applies then, because it
			// is the one the mod ships as its own answer.
			if len(declared) > 0 && len(list) == 0 && chosen != setting.defStr {
				res.logs = append(res.logs, "fkrecipes: "+r.name+": the "+chosen+
					" ingredients name nothing this game has, so the "+setting.defStr+" ingredients apply")
				list = l.resolveIngredients(w, &res, prefix, r.name, choiceFor(by.Choices, setting.defStr))
			}
			res.recipes = append(res.recipes, list)
		} else {
			res.recipes = append(res.recipes, l.resolveIngredients(w, &res, prefix, r.name, declared))
		}
	}

	for _, t := range l.techs {
		var rt resolvedTech

		if t.spec.EnabledBy.index != 0 {
			s := l.settings[t.spec.EnabledBy.index-1]
			full := s.emittedName(prefix)
			rt.hasEnabledBy = true
			rt.on = s.defBool
			if v, ok := w.StartupSetting(full); ok && v.Kind == KindBool {
				rt.on = v.Bool
			} else {
				res.logs = append(res.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
			}
		}

		if by := t.spec.CostBy; by != nil {
			setting := l.settings[by.Setting.index-1]
			chosen := res.readDropdown(w, setting, prefix)
			source := ""
			for _, name := range sourcesFor(by.Choices, chosen) {
				if w.TechHasResearchTrigger(name) {
					continue
				}
				u, ok := w.TechUnit(name)
				// NO PRESENCE PROBE HERE, and that is deliberate rather than
				// an omission: a technology the game does not have carries no
				// unit either, so this arm steps past an absent rung and a
				// unit-less one by the same test. A TechExists call in front
				// of it would be a branch no test could make load-bearing,
				// which is a liability wearing the costume of a defence.
				//
				// THE TWO TERMS BELOW ARE DISTINCT GUARDS, each with its own
				// witness, and neither is redundant on the other:
				//
				//   !ok            honours the ABSENT FLAG over whatever value
				//                  rides with it. Every World here returns Nil
				//                  beside false, so the shape check would hide
				//                  this term; a consumer's own World can hand
				//                  back a map beside false, and the fixture
				//                  does exactly that on purpose.
				//   Kind/subtree   rejects a unit that is PRESENT but not one
				//                  this library can copy faithfully: not a
				//                  dictionary, or holding a subtree dropped on
				//                  the way in.
				if !ok || u.Kind != KindMap || holdsDroppedSubtree(u) {
					continue
				}
				rt.unit = u
				if level, ok := w.TechMaxLevel(name); ok && level.Kind != KindNil && !holdsDroppedSubtree(level) {
					rt.maxLevel = level
					rt.hasMaxLevel = true
				}
				source = name
				break
			}
			if source == "" {
				// THE FALLBACK IS RESOLVED ONLY HERE, which is the point: its
				// packs are probed when the fallback is what applies, and never
				// when a source answered. The line saying why comes first, so
				// the drops that follow read as consequences of it.
				res.logs = append(res.logs, "fkrecipes: "+t.name+": no source for the "+chosen+
					" cost carries a unit, so the fallback cost applies and the technology has no prerequisite")
				rt.unit = resolveUnit(w, &res, t.name, &by.Fallback)
			} else {
				// THE PREREQUISITE MOVES WITH THE UNIT.
				rt.prereqs = []string{source}
			}
			rt.hasUnit = true
			res.techs = append(res.techs, rt)
			continue
		}

		// The hand-rolled cost, with each pack's ladder walked here rather than
		// at emit: which rung the game has is a fact about the game, and the
		// drops belong in the log stream at the point they were decided, before
		// this technology's tree placement.
		if t.spec.Unit != nil {
			rt.unit = resolveUnit(w, &res, t.name, t.spec.Unit)
			rt.hasUnit = true
		}

		after, before := t.spec.After, t.spec.Before
		newName := t.emittedName(prefix)
		switch {
		case t.spec.AfterTech.index != 0:
			// An anchor this plan declares itself needs no presence probe and
			// cannot degrade: the prototype is emitted by the same plan.
			//
			// THE OVERLAY IS WHAT CATCHES A RING, not the argument below.
			// This edge joins it like any other prerequisite and must never
			// be left out of it on the argument's strength. The argument is
			// defence in depth and it only covers handles THIS plan issued:
			// such a handle exists only after the technology it names, so
			// those edges run backwards through declaration order, and a
			// technology anchored this way takes no Before, so nothing in the
			// game's tree is rewritten to require it. A handle from anywhere
			// else can point forwards or at itself, and the walk is what
			// answers that.
			rt.prereqs = []string{l.techs[t.spec.AfterTech.index-1].emittedName(prefix)}
		case after == "":
			// Before without After was refused in validate; nothing to place.
		case before == "":
			if w.TechExists(after) {
				rt.prereqs = []string{after}
			} else {
				res.logs = append(res.logs, dropLine(t.name, after))
			}
		case !w.TechExists(before):
			res.logs = append(res.logs, "fkrecipes: "+t.name+": "+before+" is absent, so InsertBetween degrades to After("+after+")")
			if w.TechExists(after) {
				rt.prereqs = []string{after}
			} else {
				res.logs = append(res.logs, dropLine(t.name, after))
			}
		case !w.TechExists(after):
			// The anchor is gone, so there is nothing to splice between and
			// nothing to replace in the other technology's list. Emit the
			// technology unattached rather than guess a substitute.
			res.logs = append(res.logs, dropLine(t.name, after))
		default:
			rt.prereqs = []string{after}
			base := res.currentPrereqs(w, before)
			list := make([]string, 0, len(base)+1)
			replaced := false
			for _, p := range base {
				if p == after {
					list = append(list, newName)
					replaced = true
					continue
				}
				list = append(list, p)
			}
			if !replaced {
				list = append(list, newName)
				res.logs = append(res.logs, "fkrecipes: "+t.name+": "+before+" does not require "+after+", so the new technology is appended to its prerequisites")
			}
			res.rewrites = append(res.rewrites, rewriteRec{before: before, list: list})
			rt.rewrite = len(res.rewrites)
		}

		res.techs = append(res.techs, rt)
	}
	return res
}

// currentPrereqs is the prerequisite list a splice should build on: the one an
// earlier splice in this same plan already planned, or the game's own.
func (r *resolution) currentPrereqs(w World, tech string) []string {
	for i := len(r.rewrites) - 1; i >= 0; i-- {
		if r.rewrites[i].before == tech {
			// Cloned because the Rust mirror clones: an aliased list here is
			// a caller that can rewrite a planned splice through the slice it
			// was handed.
			return copyStrings(r.rewrites[i].list)
		}
	}
	return w.TechPrereqs(tech)
}

func dropLine(tech, after string) string {
	return "fkrecipes: " + tech + ": " + after + " is absent, so the prerequisite is dropped"
}

// unlockedRecipes marks the recipes some technology unlocks. Those are emitted
// disabled, because the research is what turns them on.
// checkResolvedCraftTimes refuses a bound crafting time the engine would not
// take. It runs after resolution because the value is a fact about what the
// World answered, not about what the plan declared.
//
// The setting is generated with a minimum above the floor, so the ordinary way
// to reach this is another mod: setting names are a global namespace and the
// engine keeps the last declaration of a same-type name, silently. A refusal
// naming the setting beats the engine's load failure blaming the consumer.
func (l *Lib) checkResolvedCraftTimes(res resolution) error {
	for i, ct := range res.craftTimes {
		if !ct.bound {
			continue
		}
		// Finiteness FIRST, and not only for the message: an infinity is
		// above the floor, so the floor arm would wave it through and ship a
		// recipe that never completes. This is the one float in the library
		// that arrives from outside and so never crossed the declaration
		// checks.
		if !finite(ct.value) {
			return errors.New("fkrecipes: the recipe " + l.recipes[i].name +
				" reads its crafting time from " + ct.setting +
				", which answers a value that is not a finite number")
		}
		if ct.value > craftTimeFloor {
			continue
		}
		return errors.New("fkrecipes: the recipe " + l.recipes[i].name +
			" reads its crafting time from " + ct.setting +
			", which answers at or below the engine floor (energy_required can't be <= 0.001)")
	}
	return nil
}

// checkResolvedPacks refuses a technology whose every declared science pack
// dropped. It runs after resolution for the same reason the crafting-time check
// does: which packs the game has is a fact about the World, not about the plan,
// and a fallback that never applies is never asked about at all.
//
// A COST WITH NO PACKS IS NOT A CHEAP RESEARCH, it is a free one. The engine
// loads such a unit (measured), which is exactly why this half refuses: a
// modpack missing every pack a technology was priced in would otherwise hand
// the player a research they finish instantly and nobody would see a refusal.
//
// ONE SENTENCE, NAMING THE FIRST SUCH TECHNOLOGY IN DECLARATION ORDER, because
// resolution walks the plan in that order and records only the first. A plan
// with two costs the game cannot pay answers the same way every run, in both
// languages.
func checkResolvedPacks(res resolution) error {
	if res.packless == "" {
		return nil
	}
	return errors.New("fkrecipes: the technology " + res.packless +
		" has no science pack the game has; research takes at least one")
}

func (l *Lib) unlockedRecipes() []bool {
	marks := make([]bool, len(l.recipes))
	for _, t := range l.techs {
		for _, u := range t.spec.Unlocks {
			marks[u.index-1] = true
		}
	}
	return marks
}

func itemProto(prefix string, it itemDecl) Value {
	pairs := []KV{
		kv("type", Str("item")),
		kv("name", Str(it.emittedName(prefix))),
	}
	pairs = appendLocalised(pairs, it.spec.DisplayName, it.spec.Description)
	if it.spec.Icon != "" {
		pairs = append(pairs, kv("icon", Str(it.spec.Icon)))
	}
	if it.spec.IconSize != 0 {
		pairs = append(pairs, kv("icon_size", Num(float64(it.spec.IconSize))))
	}
	stack := it.spec.StackSize
	if stack == 0 {
		stack = 50
	}
	pairs = append(pairs, kv("stack_size", Num(float64(stack))))
	if it.spec.Subgroup != "" {
		pairs = append(pairs, kv("subgroup", Str(it.spec.Subgroup)))
	}
	if it.spec.Order != "" {
		pairs = append(pairs, kv("order", Str(it.spec.Order)))
	}
	if it.spec.PlaceResult != "" {
		pairs = append(pairs, kv("place_result", Str(it.spec.PlaceResult)))
	}
	return Obj(append(pairs, it.spec.Extra...)...)
}

func recipeProto(prefix string, l *Lib, r recipeDecl, ings []resolvedIngredient, ct craftTime, unlocked bool) Value {
	pairs := []KV{
		kv("type", Str("recipe")),
		kv("name", Str(r.emittedName(prefix))),
	}
	pairs = appendLocalised(pairs, r.spec.DisplayName, r.spec.Description)
	if r.spec.Category != "" {
		pairs = append(pairs, kv("category", Str(r.spec.Category)))
	}
	// energy_required is omitted rather than sent as zero: an absent field is
	// the engine's own default, and a zero is a crafting time the engine
	// refuses. A bound recipe carries whatever the player's setting answered,
	// which the floor check has already cleared.
	if ct.bound {
		pairs = append(pairs, kv("energy_required", Num(ct.value)))
	} else if r.spec.CraftTime > 0 {
		pairs = append(pairs, kv("energy_required", Num(r.spec.CraftTime)))
	}
	pairs = append(pairs, kv("enabled", Bool(!unlocked)))

	// Recipe ingredients are the LONG DICT form. The technology unit's short
	// tuple form is REFUSED here and the other way round; measured, not
	// generalised from one to the other.
	//
	// A FLUID CARRIES ITS OWN TYPE AND ITS OWN AMOUNT. type="fluid" is what
	// makes the engine read it out of data.raw.fluid, and the amount rides as
	// the double it was declared as: the engine dumped 0.5 and 1000000000
	// unchanged (measured), so nothing here rounds one.
	items := make([]Value, 0, len(ings))
	for _, ing := range ings {
		if ing.kind == kindFluid {
			items = append(items, Obj(
				kv("type", Str("fluid")),
				kv("name", Str(ing.name)),
				kv("amount", Num(ing.fluid)),
			))
			continue
		}
		items = append(items, Obj(
			kv("type", Str("item")),
			kv("name", Str(ing.name)),
			kv("amount", Num(float64(ing.amount))),
		))
	}
	pairs = append(pairs, kv("ingredients", Arr(items...)))

	count := r.spec.ResultCount
	if count == 0 {
		count = 1
	}
	// The result is either an item this plan declares (prefixed or legacy by
	// its own declaration) or one that already exists, named verbatim because
	// it is somebody else's and validation has probed it.
	result := r.spec.ResultNamed
	if r.result.index != 0 {
		result = l.items[r.result.index-1].emittedName(prefix)
	}
	pairs = append(pairs, kv("results", Arr(Obj(
		kv("type", Str("item")),
		kv("name", Str(result)),
		kv("amount", Num(float64(count))),
	))))
	if r.spec.Order != "" {
		pairs = append(pairs, kv("order", Str(r.spec.Order)))
	}
	return Obj(append(pairs, r.spec.Extra...)...)
}

func techProto(prefix string, l *Lib, w World, t techDecl, rt resolvedTech) Value {
	pairs := []KV{
		kv("type", Str("technology")),
		kv("name", Str(t.emittedName(prefix))),
	}
	pairs = appendLocalised(pairs, t.spec.DisplayName, t.spec.Description)
	if t.spec.Icon != "" {
		pairs = append(pairs, kv("icon", Str(t.spec.Icon)))
	}
	if t.spec.IconSize != 0 {
		pairs = append(pairs, kv("icon_size", Num(float64(t.spec.IconSize))))
	}
	if len(rt.prereqs) > 0 {
		pairs = append(pairs, kv("prerequisites", strArr(rt.prereqs)))
	}
	pairs = append(pairs, kv("unit", techUnit(w, t, rt)))
	// max_level lives on the TECHNOLOGY, not in its unit, so copying the unit
	// verbatim carries a count_formula but leaves the level cap behind. CostOf
	// is one named point for cost AND position, so it reads the cap too: an
	// infinite source technology produces an infinite copy. A hand-rolled
	// UnitSpec has no source to read, and gets no cap.
	if level, ok := techMaxLevel(w, t, rt); ok {
		pairs = append(pairs, kv("max_level", level))
	}
	if len(t.spec.Unlocks) > 0 {
		effects := make([]Value, 0, len(t.spec.Unlocks))
		for _, u := range t.spec.Unlocks {
			effects = append(effects, Obj(
				kv("type", Str("unlock-recipe")),
				kv("recipe", Str(l.recipes[u.index-1].emittedName(prefix))),
			))
		}
		pairs = append(pairs, kv("effects", Arr(effects...)))
	}
	// HIDDEN, NOT ABSENT. A technology researched in an existing save whose
	// prototype vanishes is dropped from that save, and flipping a startup
	// setting is exactly the mid-save event this library invites, so a
	// switched-off technology keeps its prototype and loses its visibility.
	if rt.hasEnabledBy {
		if rt.on {
			pairs = append(pairs, kv("enabled", Bool(true)))
		} else {
			pairs = append(pairs, kv("enabled", Bool(false)), kv("hidden", Bool(true)))
		}
	}
	if t.spec.Order != "" {
		pairs = append(pairs, kv("order", Str(t.spec.Order)))
	}
	return Obj(append(pairs, t.spec.Extra...)...)
}

func techUnit(w World, t techDecl, rt resolvedTech) Value {
	// Unit and CostBy both settled their cost during resolution, because both
	// of them had a question to ask the game. CostOf is the one arm left.
	if rt.hasUnit {
		return rt.unit
	}
	// Verbatim, whatever it holds: a count_formula is a string and copying one
	// needs no evaluator, so multi-level and infinite technologies come along
	// for free. Validation proved the unit is there; the flag is still read
	// rather than dropped, because the Rust mirror maps its absent case to the
	// same nil.
	u, ok := w.TechUnit(t.spec.CostOf)
	if !ok {
		return Nil()
	}
	return u
}

// techMaxLevel is the level cap a technology carries, if any. A hand-rolled
// cost has none; a copied one carries the source's.
func techMaxLevel(w World, t techDecl, rt resolvedTech) (Value, bool) {
	if t.spec.CostBy != nil {
		return rt.maxLevel, rt.hasMaxLevel
	}
	if t.spec.Unit != nil {
		return Nil(), false
	}
	// A present-but-nil read is a value this library could not carry (a
	// LuaObject, or a table with a key it drops); emitting it would write a
	// nil max_level into the prototype.
	level, ok := w.TechMaxLevel(t.spec.CostOf)
	return level, ok && level.Kind != KindNil
}

// resolveUnit walks each pack's ladder and drops what the game does not have,
// then builds the hand-rolled cost's wire shape. Technology unit ingredients
// are the SHORT TUPLE form; the dict form is refused here by the engine.
//
// THE LADDER ASKS ToolExists AND NOTHING ELSE. MEASURED: a research unit priced
// in a plain item refuses the load with "Invalid research unit (iron-plate).
// Research unit(s) can only be tool type items at the moment", and a fluid name
// in a unit refuses with "Error in assignID: item with name 'water' does not
// exist". Asking ItemExists would let either of those through as a rung.
//
// A DROP RATHER THAN A REFUSAL, which is the change this ladder is here to
// make: a modpack that renamed or removed a science pack used to fail the load
// with the author's name on it, and now loses that pack from the cost and says
// so. A unit that loses ALL of them is refused instead, because a research with
// no pack at all is not something to emit on an author's behalf.
func resolveUnit(w World, res *resolution, tech string, u *UnitSpec) Value {
	packs := make([]Value, 0, len(u.Packs))
	for _, p := range u.Packs {
		candidates := p.ladder()
		picked := ""
		for _, c := range candidates {
			if w.ToolExists(c) {
				picked = c
				break
			}
		}
		if picked == "" {
			res.logs = append(res.logs, "fkrecipes: "+tech+": none of "+strings.Join(candidates, ", ")+
				" is present, so the science pack is dropped")
			continue
		}
		packs = append(packs, Arr(Str(picked), Num(float64(p.Amount))))
	}
	// A cost that named packs and got none of them. A unit that DECLARED none
	// never reaches here: plan validation refused it before any of this was
	// asked, which is what leaves this sentence to the world's answer alone.
	//
	// The FIRST technology in declaration order is the one reported, because
	// the walk runs in that order and the refusal is one sentence.
	if len(packs) == 0 && res.packless == "" {
		res.packless = tech
	}
	return Obj(
		kv("count", Num(float64(u.Count))),
		kv("time", Num(u.Seconds)),
		kv("ingredients", Arr(packs...)),
	)
}

func appendLocalised(pairs []KV, displayName, description string) []KV {
	if displayName != "" {
		pairs = append(pairs, kv("localised_name", localised(displayName)))
	}
	if description != "" {
		pairs = append(pairs, kv("localised_description", localised(description)))
	}
	return pairs
}

// holdsDroppedSubtree reports the marker a lossy read leaves behind.
//
// A value this library cannot carry faithfully converts to Nil as a WHOLE
// subtree rather than being partly kept, and fkdata never delivers a nil map
// value or array element of its own (its write side skips nils), so a Nil
// anywhere inside a value that came out of data.raw means exactly one thing:
// a table was dropped on the way in. A copied unit that lost a subtree is a
// technology researchable for free, which is why this is a refusal rather
// than a log line.
func holdsDroppedSubtree(v Value) bool {
	switch v.Kind {
	case KindMap:
		for _, p := range v.Map {
			if p.Val.Kind == KindNil || holdsDroppedSubtree(p.Val) {
				return true
			}
		}
	case KindArr:
		for _, item := range v.Arr {
			if item.Kind == KindNil || holdsDroppedSubtree(item) {
				return true
			}
		}
	}
	return false
}

// validateIngredients is the ordinary ingredient check, shared by a fixed
// ingredient list and by every plan a dropdown can select.
//
// THE CATEGORY IS A PARAMETER because the fluid rule is about the recipe and
// not about the ingredient: the same FluidIngredient is legal in a chemistry
// recipe and a load failure in a crafting one, so the check needs both halves.
func (l *Lib) validateIngredients(at, who, category string, ings []Ingredient) error {
	for _, ing := range ings {
		// A RUNG WITH NO NAME IS A LADDER THAT CAN NEVER ANSWER, exactly as it
		// is for a science pack, and it is refused here for the same reason:
		// ItemExists("") and FluidExists("") are questions no World has a
		// useful answer to, and an ingredient that reached resolution would
		// either be dropped with a log line reading "none of , iron-plate is
		// present" or be named by a sentence with a hole where a name goes.
		//
		// BEFORE THE KIND ARMS, so it covers a first rung and a fallback of
		// either kind in one place, and so the fluid sentences below can name
		// candidates[0] knowing it is a name.
		for _, c := range ing.candidates {
			if c == "" {
				return errors.New(at + who + " names an ingredient with an empty name")
			}
		}
		if ing.kind == kindFluid {
			if !finite(ing.fluidAmount) {
				return errors.New(at + who + " declares a fluid amount that is not a finite number")
			}
			// MEASURED: a fluid ingredient with amount 0 refuses the load with
			// "amount must be larger than 0", and 0.5 and 1000000000 both
			// load. So the floor is the only bound a fluid has, and it is
			// exclusive.
			if ing.fluidAmount <= 0 {
				return errors.New(at + who + " has a fluid amount at or below zero, which the engine refuses")
			}
			// The engine's own rule, and categoryTakesItemsOnly is where it is
			// written: the language asks the same question of a text the
			// player typed, and one predicate is what keeps the two answers
			// the same rule rather than the same sentence twice.
			//
			// The FIRST candidate is what the sentence names: a ladder of
			// fluids is wrong in a crafting recipe whichever rung answers, and
			// naming the one the consumer wrote first points at the
			// declaration rather than at the game. That candidate is a NAME by
			// construction: the empty-name check at the top of this loop has
			// already refused a ladder whose first rung is nothing, so no
			// refusal can come out reading "takes the fluid , and".
			if categoryTakesItemsOnly(category) {
				return errors.New(at + who + " takes the fluid " + ing.candidates[0] +
					", and a recipe in the crafting category takes items only")
			}
			// THE CEILING IS THE ENGINE'S, and above it the engine does not
			// refuse, it ABORTS (measured: 1e301 loads and dumps, 1e302 dies in
			// FixedPointNumber.hpp with the crash handler). A crash is not
			// something a player can read, so the declared path refuses here
			// exactly as the typed path does, and AFTER the category rule,
			// because a fluid in a crafting recipe is wrong at any amount.
			if ing.fluidAmount > maxFluidAmount {
				return errors.New(at + who + " takes the fluid " + ing.candidates[0] +
					" at an amount above 1e301, which the game cannot hold")
			}
			continue
		}
		if ing.amount < 1 {
			return errors.New(at + who + " has an ingredient amount below 1, which the engine refuses")
		}
		if ing.amount > maxExactInt {
			return errors.New(at + who + " declares an ingredient amount a Lua double cannot hold exactly: " + strconv.FormatInt(ing.amount, 10))
		}
		if len(ing.candidates) == 0 && !l.validItem(ing.item) {
			return errors.New(at + who + " names an ingredient item that this plan never declared")
		}
	}
	return nil
}

// validateUnit is the hand-rolled cost check, shared by Unit and by CostBy's
// fallback: a fallback the engine would refuse is not a fallback.
//
// IT ASKS THE WORLD NOTHING, which is what lets the fallback be checked here
// while its packs are probed only when it is used. Every rule below is about
// what the plan DECLARED, so a fallback nobody reaches is still held to the
// engine's numbers without a single presence question being asked about a cost
// that never applies.
func (l *Lib) validateUnit(at string, name string, u *UnitSpec) error {
	if u.Count < 1 {
		return errors.New(at + "the technology " + name + " has a unit count below 1, which the engine refuses")
	}
	// A DECLARED PACK LIST THAT IS EMPTY IS REFUSED HERE, before a single
	// question is asked about the world, because it is a fact about the plan:
	// the author priced a research in nothing. The engine LOADS such a unit
	// (measured), so nothing downstream would complain and the player would
	// get a research that completes instantly.
	//
	// It is a different sentence from the one a cost that named packs and lost
	// them all gets, and it has to be, because the two have different answers:
	// this one is the author's to fix, and that one is about a game that does
	// not have what the author named.
	if len(u.Packs) == 0 {
		return errors.New(at + "the technology " + name + " declares no science pack; research takes at least one")
	}
	if u.Count > maxExactInt {
		return errors.New(at + "the technology " + name + " declares a unit count a Lua double cannot hold exactly: " + strconv.FormatInt(u.Count, 10))
	}
	if !finite(u.Seconds) {
		return errors.New(at + "the technology " + name + " declares a research time that is not a finite number")
	}
	if u.Seconds <= 0 {
		return errors.New(at + "the technology " + name + " has a research time at or below zero, which the engine refuses")
	}
	for _, p := range u.Packs {
		if p.Amount < 1 {
			return errors.New(at + "the technology " + name + " has a science pack amount below 1, which the engine refuses")
		}
		if p.Amount > maxExactInt {
			return errors.New(at + "the technology " + name + " declares a science pack amount a Lua double cannot hold exactly: " + strconv.FormatInt(p.Amount, 10))
		}
		if p.Name == "" {
			return errors.New(at + "the technology " + name + " prices itself in a pack with an empty name")
		}
		// A rung with no name is a ladder that can never answer, and it would
		// otherwise reach a log line reading "none of a, , b is present".
		for _, c := range p.Fallbacks {
			if c == "" {
				return errors.New(at + "the technology " + name + " prices itself in a pack with an empty name")
			}
		}
	}
	return nil
}

// matchesAllowedValues checks a choice list against the dropdown it is driven
// by. They have to line up exactly, in order: a plan for a value the setting
// does not offer is unreachable, and a value with no plan is a player choice
// with nothing behind it.
func matchesAllowedValues(at, who, setting string, offered, allowed []string) error {
	for i := 0; i < len(offered) || i < len(allowed); i++ {
		switch {
		case i >= len(offered):
			return errors.New(at + who + " offers nothing for the value " + allowed[i] + " that the setting " + setting + " allows")
		case i >= len(allowed):
			return errors.New(at + who + " offers something for " + offered[i] + ", which the setting " + setting + " does not allow")
		case offered[i] != allowed[i]:
			return errors.New(at + who + " offers something for " + offered[i] + " where the setting " + setting + " allows " + allowed[i])
		}
	}
	return nil
}

// readDropdown answers with the value a dropdown setting holds, or with the
// declared default and the ordinary unreadable line.
func (r *resolution) readDropdown(w World, s settingDecl, prefix string) string {
	full := s.emittedName(prefix)
	if v, ok := w.StartupSetting(full); ok && v.Kind == KindStr {
		return v.Str
	}
	r.logs = append(r.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
	return s.defStr
}

func choiceFor(choices []IngredientChoice, value string) []Ingredient {
	for _, c := range choices {
		if c.Value == value {
			return c.Ingredients
		}
	}
	return nil
}

func sourcesFor(choices []CostChoice, value string) []string {
	for _, c := range choices {
		if c.Value == value {
			return c.Sources
		}
	}
	return nil
}

// resolveIngredients walks each ladder and drops what the game does not have.
//
// A FLUID LADDER ASKS A DIFFERENT QUESTION. Items and fluids are separate
// namespaces, so a fluid candidate is probed with FluidExists: asking
// ItemExists about water would drop every fluid ingredient in the game, and
// asking both would let an item answer for a fluid and emit a recipe the
// engine refuses.
func (l *Lib) resolveIngredients(w World, res *resolution, prefix, recipe string, ings []Ingredient) []resolvedIngredient {
	list := make([]resolvedIngredient, 0, len(ings))
	for _, ing := range ings {
		if len(ing.candidates) == 0 {
			list = append(list, resolvedIngredient{name: l.items[ing.item.index-1].emittedName(prefix), amount: ing.amount})
			continue
		}
		picked := ""
		for _, c := range ing.candidates {
			present := w.ItemExists(c)
			if ing.kind == kindFluid {
				present = w.FluidExists(c)
			}
			if present {
				picked = c
				break
			}
		}
		if picked == "" {
			res.logs = append(res.logs, "fkrecipes: "+recipe+": none of "+strings.Join(ing.candidates, ", ")+" is present, so the ingredient is dropped")
			continue
		}
		if ing.kind == kindFluid {
			list = append(list, resolvedIngredient{kind: kindFluid, name: picked, fluid: ing.fluidAmount})
			continue
		}
		list = append(list, resolvedIngredient{name: picked, amount: ing.amount})
	}
	return list
}

// The field names each prototype builder writes itself. A key in Extra that
// collides with one is REFUSED rather than merged: two writers of one field is
// a silent last-writer, and the loser would be whichever order this library
// happens to append in. Listed rather than derived, because a builder emits a
// field CONDITIONALLY and the answer must not depend on which arms fired for
// this particular declaration: an Extra key that collides only when a sibling
// field happens to be set would be a refusal a consumer could not reproduce.
var (
	itemOwnFields = []string{
		"type", "name", "localised_name", "localised_description",
		"icon", "icon_size", "stack_size", "subgroup", "order", "place_result",
	}
	recipeOwnFields = []string{
		"type", "name", "localised_name", "localised_description",
		"category", "energy_required", "enabled", "ingredients", "results", "order",
	}
	techOwnFields = []string{
		"type", "name", "localised_name", "localised_description",
		"icon", "icon_size", "prerequisites", "unit", "max_level", "effects",
		"enabled", "hidden", "order",
	}
)

func checkExtra(at, who string, extra []KV, own []string) error {
	for i, e := range extra {
		if e.Key == "" {
			return errors.New(at + who + " sets a field through Extra with an empty name")
		}
		for _, o := range own {
			if o == e.Key {
				return errors.New(at + who + " sets " + e.Key + " through Extra, which this library emits")
			}
		}
		// Two Extra keys writing one field is the same silent last-writer,
		// and this one is entirely the consumer's own doing.
		for j := 0; j < i; j++ {
			if extra[j].Key == e.Key {
				return errors.New(at + who + " sets " + e.Key + " through Extra twice")
			}
		}
	}
	return nil
}
