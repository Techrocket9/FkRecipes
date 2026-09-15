package fkrecipes

import (
	"errors"
	"strconv"
	"strings"
	"unicode/utf8"
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
	if err := l.afterResolution(w, &res, prefix); err != nil {
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
		ops = append(ops, extendOp(recipeProto(prefix, l, r, res.recipes[i], res.craftTimes[i], unlocked[i], res.recipeNotes[i])))
	}
	for i, t := range l.techs {
		ops = append(ops, extendOp(techProto(prefix, l, w, t, res.techs[i], res.techNotes[i])))
	}
	for i := range l.techs {
		rt := res.techs[i]
		if rt.rewrite == 0 {
			continue
		}
		rw := res.rewrites[rt.rewrite-1]
		// A SPLICE THE CYCLE WALK DROPPED IS SKIPPED WHOLE, and skipped rather
		// than written back with the name taken out: what is left in the record
		// is another mod's own prerequisite list, and Setting that back over
		// its prototype is a write this library has no reason to make. See
		// checkCycles, which is what marks the record.
		if rw.dropped {
			continue
		}
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

	// THE BINDINGS FIRST, and the text settings after them, both shared with
	// the settings planner. They come before the three declaration loops
	// because a text setting or a research number whose declaration does not
	// line up would otherwise be answered by a later rule that points at the
	// wrong thing. Neither of them asks the World anything.
	if err := l.validateBindings(prefix); err != nil {
		return err
	}
	if err := l.validateTextSettings(prefix); err != nil {
		return err
	}

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
		// enabled is the ONE field of a recipe a consumer may write, and only
		// while nothing in this plan unlocks the recipe. A recipe some
		// technology unlocks is emitted disabled because the research is what
		// turns it on, and a second writer of that field would be exactly the
		// silent last-writer every other collision is refused for. With no
		// unlock in the plan the library has no opinion to lose, and a mod
		// migrating its recipe a commit before its technology needs to say
		// enabled = false by hand in the meantime.
		if _, ok := extraField(r.spec.Extra, "enabled"); ok {
			if tech := l.unlockingTech(i); tech != "" {
				return errors.New(at + "the recipe " + r.name + " puts enabled in Extra, but the technology " +
					tech + " unlocks it, so the library owns that field")
			}
		}
		if len(r.spec.Ingredients) > 0 && r.spec.IngredientsBy != nil {
			return errors.New(at + "the recipe " + r.name + " names both Ingredients and IngredientsBy; pick one")
		}
		if r.spec.IngredientsBy != nil {
			by := r.spec.IngredientsBy
			if !l.validDropdownSetting(by.Setting) {
				return errors.New(at + "the recipe " + r.name + " names an ingredients setting that this plan never declared")
			}
			// THE CHOICES COVER THE ALLOWED VALUES EXACTLY, with nothing
			// subtracted: this library adds no value of its own to a dropdown,
			// so every value the author declared needs a plan behind it.
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
				if err := l.validateNoDuplicates(at, "the recipe "+r.name, prefix, c.Ingredients); err != nil {
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
		if err := l.validateNoDuplicates(at, "the recipe "+r.name, prefix, r.spec.Ingredients); err != nil {
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
		// CostBy AND CostFrom ARE ONE COST, which is why this asks
		// namedCostSources rather than counting the four fields: the dropdown
		// is the tier and the three settings overwrite it field by field. Every
		// other pairing is still two sources and still refused.
		if namedCostSources(&t.spec) != 1 {
			return errors.New(at + "the technology " + t.name + " must name exactly one of CostOf, Unit, CostBy or CostFrom")
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
	// anchor is WHAT THE SPLICE REPLACED: the technology whose place in
	// before's prerequisite list the new name took, or the empty string where
	// there was nothing to replace and the name was appended.
	//
	// IT IS WHAT MAKES A DROPPED SPLICE UNDOABLE, and without it the drop
	// destroys an edge of somebody else's tree. A splice REPLACES its anchor,
	// so a SECOND splice into the same technology builds its record on a list
	// the anchor is already out of; taking the dropped name back out of that
	// later record without putting the anchor back leaves the later record
	// emitting a prerequisite list with a base-game edge silently missing from
	// it. See dropSplice, which is the one reader.
	anchor string
	// dropped is a splice the cycle walk took back because it closed a ring.
	// It is MARKED RATHER THAN REMOVED because resolvedTech.rewrite is a
	// 1-based index into this slice and renumbering it would point every later
	// technology at somebody else's record.
	dropped bool
}

type resolvedTech struct {
	// The cost this technology settled on, and the level cap that rode along
	// with it. Resolved rather than emitted straight from the spec, because
	// which source answered, and which rung of each pack's ladder the game
	// actually has, are facts about the game.
	//
	// hasUnit is true for every cost shape that RESOLVED one, which is now all
	// four: CostOf copies its source's unit verbatim and then filters the
	// science packs out of it, so the unit it settles on is a fact about this
	// game rather than a value the emit layer can read back for itself. It is
	// false only where the technology named no cost at all, and for a CostOf
	// whose source carries no unit, which validate has already refused.
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

// noteTarget names the prototype whose emitted localised_description carries
// the trailing line a fallback owes the player, and it is THREADED FROM THE
// WALK rather than derived from anything the callee can see.
//
// A DERIVED ONE WOULD BE WRONG IN BOTH DIRECTIONS. The index cannot be read off
// len(res.recipes), because a recipe's list is handed over at the END of its
// arm and every read that can fall back happens before it; and it cannot be a
// cursor the walk sets, because a cursor left stale by one arm silently writes
// a note onto the previous declaration. The parameter is what makes a caller
// that forgot it a compile error.
//
// A RECIPE IS ALSO WHAT SAYS WHICH SENTENCES THE NOTE CARRIES. See
// fallbackNote: a recipe's note names the engine's input-slot cost and a
// technology's does not, and the text setting a recipe target reaches is
// always its ingredient list.
type noteTarget struct {
	tech  bool
	index int
}

type resolution struct {
	logs       []string
	craftTimes []craftTime
	recipes    [][]resolvedIngredient
	techs      []resolvedTech
	rewrites   []rewriteRec

	// recipeNotes and techNotes hold the trailing line each emitted prototype's
	// description carries, indexed by DECLARATION ORDER rather than by the
	// order the walk filled them, and empty for a declaration nothing fell back
	// on.
	//
	// ONE NOTE PER PROTOTYPE, THE FIRST IN WALK ORDER, so the sentence a player
	// hovers is the same string every run. Two settings bound to one recipe can
	// both fall back in one load; the second adds nothing, exactly as the
	// refusal's added sentence names only the first.
	//
	// A SLICE SIZED UP FRONT, not a map keyed by name: the emit loop reads it
	// by the same index it reads res.recipes and res.craftTimes by, and a map
	// would be an iteration order this library does not allow anywhere.
	recipeNotes []string
	techNotes   []string

	// packlessSaid is, for every technology that went packless, the exact ERROR
	// line it logged. It exists for the ONE retraction there is: a player who
	// types a pack list over a tier writes over the very unit that went
	// packless, and the line it earned a moment ago has to go with it. The
	// note goes back with the snapshot the same caller restores, which is why
	// only the line is kept here. The line is composed from names the
	// retracting site never saw, so it is kept rather than recomposed.
	//
	// IT IS NO LONGER A REFUSAL CARRIER. A technology with no science pack the
	// game has is EMITTED with an empty ingredient list now, because the error
	// dialog a refusal raises cannot reach the Mod Settings screen (measured on
	// 2.0.77: see fallbackFact) and a mod set the player did not choose must
	// not be able to lock them out. What the emptied unit costs them is a free
	// research, which is disclosed in the technology's own tooltip and in the
	// log: see packlessAt.
	packlessSaid []packlessRec

	// refusal is the FIRST sentence the walk found that stops the load. The
	// producers left are a dropdown holding a value it does not offer (which
	// the engine resets before any stage runs, so only a hand-edited file
	// reaches it) and refuseCostNumbers' answers about a declared default the
	// settings stage would already have refused. The merged amounts that used
	// to be here are CLAMPED now, with a line and a tooltip note each, and the
	// copied research unit whose pack list is in neither engine form now
	// degrades with a line and a note of its own: see mergeIngredient and
	// filterCopiedPacks. Carried out rather than raised because resolve answers
	// with facts and PlanData decides which of them stops a load.
	refusal string

	// fellBack names every setting whose STORED value the library could not
	// use, in walk order and ONCE EACH.
	//
	// IT HAS TWO READERS. It is the DEDUPE: a crafting-time setting two recipes
	// read is one field on the settings screen, so a bad value there is one
	// problem and gets one line however many declarations reach it, and this
	// slice is what a second reader is checked against. And it is what
	// fallbackFact states, because the first element is the setting a refusal
	// raised after resolution names.
	//
	// A SLICE AND A LINEAR SCAN RATHER THAN A SET: the order is the answer, a
	// map would not have one, and no plan declares enough settings for the
	// scan to be worth a second data structure.
	fellBack []string
}

// addRecipe records one recipe's resolved ingredient list, and it is the ONLY
// place that writes res.recipes: a source property in source_test.go refuses a
// bare append anywhere else.
//
// FOUR ARMS REACH IT AND A FIFTH WOULD. resolve answers a recipe's ingredients
// through a text with no dropdown beside it, through a text that takes a
// dropdown's choice over, through a dropdown on a preset and through a plain
// declared list; a check written into any one
// of them would be missing from the other three, and from whichever arm is
// added next. So the check lives here, where the list is handed over.
//
// THE COMPILER DOES NOT ENFORCE THAT AND A TEST DOES. A fifth arm that
// appended to res.recipes itself compiles, plans, and is merely silent. The
// only arm the language itself catches is one that adds NO entry, and it
// catches it at run time rather than at compile time: PlanData indexes
// res.recipes[i] against l.recipes and a short slice panics there. So the
// guard against a fifth arm going round this hand-over is
// TestOnlyTheHandOverWritesTheResolvedRecipes in source_test.go.
//
// THE POSITION IN THE LOG STREAM IS WHAT THE HAND-OVER POINT BUYS. The line is
// evaluated on the FINAL list, so it has to come after every merge and drop
// line this recipe wrote, and it has to come before the next recipe's first
// line. Appending here is exactly that, in every arm, without any arm knowing
// it.
//
// SUBJECT IS THE DECLARED NAME, product the EMITTED one. Every other line about
// a recipe's list names the recipe as the author declared it (see
// mergedOpening), and the product is a prototype name the game will hold, so it
// is compared against resolved ingredient names, which are emitted too.
//
// resolvedFrom IS THE DECLARATION THE LIST WAS RESOLVED FROM, and it is EMPTY
// on the two arms where the player typed the list: nothing walked a ladder
// there, so nothing could have been dropped by one. It is what lets the
// emptied-list check below tell an environment's doing from a deliberate one.
func (r *resolution) addRecipe(tgt noteTarget, subject, product string, resolvedFrom []Ingredient, list []resolvedIngredient) {
	// EMPTY IS A RECIPE THAT DECLARES NEITHER SHAPE, which validate refuses
	// before resolution runs. The guard is here so this function answers for
	// the whole domain of its argument rather than for the domain some caller
	// upstream happens to hold to.
	if product != "" {
		for _, ing := range list {
			// THE KIND IS PART OF THE IDENTITY, exactly as it is in
			// mergeIngredient. A product is always an item (recipeProto
			// writes type="item" and nothing chooses otherwise), and an item
			// and a fluid of one name genuinely coexist: base carries
			// parameter-0 to parameter-9 as both. A fluid ingredient sharing
			// the name is a different thing with the same label and says
			// nothing.
			if ing.kind != kindItem || ing.name != product {
				continue
			}
			r.logs = append(r.logs, selfProductLine(subject, product))
			// AND THERE IS NO EARLY EXIT, DELIBERATELY. At most one entry can
			// match already, by two independent guards: a declared list that a
			// ladder collapsed is kept unique by mergeIngredient, and a list
			// the player typed by the language's own "entries N and M both
			// name" refusal, which resolveIngredientsFrom relies on because it
			// collects its entries without merging them. Not stopping early is
			// what gives the kind test above a WITNESS: a list holding an item
			// and a fluid of one name writes two lines the moment the kind is
			// dropped from the comparison, and a break would hide that. If
			// both guards ever failed, this line appearing twice is a louder
			// symptom than a quieter one.
		}
	}
	// A RECIPE THE ENVIRONMENT EMPTIED IS A FREE CRAFT, and it says so.
	// Everything this plan named was put to the game, every ladder ran out,
	// and what is emitted is a recipe with no ingredients at all: the same
	// balance change packlessNote covers on a technology, one prototype kind
	// over. The resolve-or-drop ladder's own disclosure stops short of it,
	// because "a shorter list than the one shown" reads straight past "there
	// is nothing left to craft it from".
	//
	// IT FIRES ONLY WHERE THE ENVIRONMENT EMPTIED THE LIST, never where the
	// list was MEANT to be empty, and the two are different in kind rather
	// than in degree. A declared empty list and the word none a player typed
	// are DELIBERATE: somebody chose a free craft and is told so where they
	// chose it, by ingredientNoneClause on the setting's own description and
	// by the author's own declaration. Both of those arrive here with
	// resolvedFrom empty, because nothing was resolved from anything; this
	// arm is the one where a non-empty declaration came in and nothing came
	// out.
	//
	// THE DESTRUCTION TAIL RIDES ON IT for the reason clampedItemNote's does:
	// the ingredient list itself is what moved, and it moved as far as it can.
	if len(resolvedFrom) > 0 && len(list) == 0 {
		r.logs = append(r.logs, ingredientlessLine(subject))
		r.noteOn(tgt, withDestruction(ingredientlessNote(), true))
	}
	r.recipes = append(r.recipes, list)
}

// noteFallback records one player-controlled setting falling back: the note the
// prototype's own description will carry, and the log line, which is written
// once per SETTING however many declarations read it.
//
// THE NOTE COMES FIRST AND IS NOT DEDUPED BY SETTING, and the two rules are
// different on purpose. One bad field on the settings screen is one problem and
// gets one line; but two recipes bound to one crafting-time setting are two
// tooltips, and a player hovering the second one is owed the same sentence as
// the first. So the dedupe below guards the line and not the note.
//
// destroysInputs is the caller's answer to "does this fallback change what the
// recipe is made of", and it is passed rather than derived: see noteOn.
func (r *resolution) noteFallback(tgt noteTarget, setting, line string, destroysInputs bool) {
	r.noteOn(tgt, fallbackNote(setting, destroysInputs))
	for _, seen := range r.fellBack {
		if seen == setting {
			return
		}
	}
	r.fellBack = append(r.fellBack, setting)
	r.logs = append(r.logs, line)
}

// afterResolution is the whole post-resolution gate: the refusal resolution
// carried, then the two checks over what it settled on, in the ORDER the design
// record fixes so that the two languages answer one declaration with one
// sentence. It answers nil when the plan may be emitted.
//
// IT TAKES THE RESOLUTION BY POINTER because the last of the two RESOLVES
// rather than checks: checkCycles drops the edges this plan made until the ring
// is gone, and the dropped prerequisites, the dropped splices, the lines and
// the notes it writes are all read by the emit walk below. A copy here would
// answer nil over a plan that still holds the cycle.
//
// THE PACK CHECK THAT USED TO SIT THIRD IS GONE ENTIRELY. A technology with no
// science pack this game has is emitted with an empty unit now: see packlessAt
// for the measurement and for what the player is told.
//
// THE CARRIED REFUSAL FIRST, before any check about what the plan declared,
// because it was found earliest in the walk and its "first one found wins" rule
// is what makes a plan with two problems answer the same way every run.
//
// NOTHING A PLAYER TYPES REACHES THAT LINE ANY MORE. A refused text and a
// number the engine would not take fall back to the author's own declaration
// with one log line each; what is left in the slot is an author's declaration
// or a hand-edited file. See resolution.refusal.
//
// WHICH IS NOT THE SAME AS "A PLAYER CANNOT BE HERE". The declaration a
// fallback lands on can fail a check further down, in a modpack where the
// author's own packs are all absent or the author's own ladders collapse onto
// one item; a player who never typed hits that refusal too.
//
// SO EVERY REFUSAL HERE LEAVES THROUGH fallbackFact, which states the FACT and
// names no screen. The sentence that used to be appended routed the player to
// Settings > Mod settings > Startup, and the client cannot get there from an
// "Error loading mods" dialog: re-measured on 2.0.77, the dialog offers Disable
// listed mods, Disable all mods, Manage mods, Restart, Exit and a Reset mod
// settings checkbox; Manage mods has no Mod settings button and its Back
// returns to the same dialog; Restart relaunches into an identical dialog over
// a file whose sha256 has not moved. What was WRONG was the route, not the
// fact: a player who typed something is still owed the knowledge that it was
// set aside, because the accumulated log ops never reach the host on a refused
// load. Every refusal left names what the AUTHOR must change, and that one
// added sentence names what the PLAYER's value did, without advice.
//
// IT IS A FUNCTION OF ITS OWN RATHER THAN FOUR STATEMENTS IN PlanData, and the
// reason is the LOWERING and not the prose. PlanData is the widest function
// this library compiles to (the packaging report's jump row names it), resolve
// inlines into it, and a Lua 5.2 function has a register ceiling as well as the
// jump ceiling the report tracks: four decorated calls in that body is what
// pushed the packaged module past "function or expression too complex" and made
// the mirror gate refuse to load it. Keeping this gate whole and calling it once
// costs nothing and gives PlanData its registers back. Keep it out of line.
//
//go:noinline
func (l *Lib) afterResolution(w World, res *resolution, prefix string) error {
	if res.refusal != "" {
		return res.fallbackFact(errors.New(res.refusal))
	}
	if err := res.fallbackFact(l.checkResolvedCraftTimes(*res)); err != nil {
		return err
	}
	return res.fallbackFact(l.checkCycles(w, res, prefix))
}

// fallbackFact adds the ONE sentence a refusal owes a player whose stored value
// was set aside on the way to it, and it exists because the log ops never reach
// the host on a refused load.
//
// THE OPS ARE LOST, WHICH IS THE WHOLE REASON. resolve accumulates its lines
// into res.logs and PlanData turns them into Ops only AFTER every check has
// passed, so a plan that refuses hands the host a message and nothing else: the
// player would read a refusal about the mod's own declaration with no hint that
// the field they edited was set aside at all.
//
// IT IS A FACT AND NOT A ROUTE, which is the whole of what changed. The
// sentence used to end by sending the player to Settings > Mod settings >
// Startup, and the client cannot get there from an "Error loading mods" dialog:
// see PlanData for the walk that measured it. What the dialog cannot reach is
// the SCREEN; the fact that a stored value was set aside is still true and is
// still the one thing this message can add. So the sentence states it and stops
// there: no screen, no route, no advice.
//
// IT MAY NOT SAY "THE MOD LOADED" IN ANY FORM, and that is why this sentence is
// worded differently from the three the customize layer writes: this one
// decorates a REFUSAL, so nothing loaded. What it can say is what happened to
// the value, and the honest generic for that is the library's own rule: a
// stored value it cannot use behaves exactly as if the player had left the
// field alone. It used to say the mod's own declaration applied, which is false
// on five of six presets: beside a dropdown a set-aside text leaves the
// dropdown's CURRENTLY CHOSEN preset deciding, and the mod's own declared list
// is what a field left alone gives only where no dropdown sits beside it. See
// fallbackNote for the same correction on the prototype side.
//
// THE ADVERBIAL BELONGS TO THE OUTCOME AND NOT TO THE SETTING-ASIDE, which is
// the one way this sentence is built differently from its three siblings.
// Nothing about the setting-aside is conditional: the value went, full stop.
// What "left alone" describes is what then APPLIED, so the clause hangs off
// that and the sentence names the two things in the order they happened.
//
// IT APPEARS ONLY WHEN A FALLBACK HAPPENED, and it names the FIRST setting in
// walk order, so a refusal on a plan nobody typed into carries nothing and a
// plan with two fallbacks answers the same way every run.
func (r *resolution) fallbackFact(err error) error {
	if err == nil || len(r.fellBack) == 0 {
		return err
	}
	return errors.New(err.Error() + ". The stored value of " + r.fellBack[0] +
		" could not be used and was set aside, so what applied is what that field gives when it is left alone.")
}

// noteOn records the trailing line one prototype's description carries, keeping
// the FIRST in walk order so the sentence is the same every run.
//
// IT TAKES THE WHOLE SENTENCE, not the pieces one composer happens to need.
// Two kinds of thing reach it: a stored value the player typed that the library
// set aside (fallbackNote), and an ENVIRONMENTAL degradation nobody typed, where
// the game is missing something the plan names (degradeNote and its two
// callers). Deciding which is which here would be a branch on a reason string,
// which is the shape the destroyed-inputs correction already refused once.
//
// THE INDEX IS NEVER CHECKED, deliberately. Both slices are sized from the
// declaration counts at the top of resolve and every index comes from the walk's
// own loop, so an out-of-range one is this file having gone wrong rather than
// anything a consumer can reach, and a panic naming the line is a better answer
// than a note silently dropped.
func (r *resolution) noteOn(tgt noteTarget, note string) {
	notes := r.recipeNotes
	if tgt.tech {
		notes = r.techNotes
	}
	if notes[tgt.index] != "" {
		return
	}
	notes[tgt.index] = note
}

// noteAt and restoreNote are the SNAPSHOT PAIR, and they exist for exactly one
// caller: the tier arm hands resolveCustomCost the note slot as it stood before
// the tier was priced, and a player's typed pack list puts that slot back.
//
// A SNAPSHOT AND NOT A LIST OF NAMED RETRACTIONS, which is the correction the
// adversarial review asked for and which removes code rather than adding it.
// Retracting by name can only take back the sentences the retracting site
// knows how to compose, and the tier arm can write one it does not: two of the
// fallback's own pack ladders landing on one name over the item ceiling leaves
// a CLAMP note through mergePack, that note takes the slot noteOn keeps for the
// first writer, the named retractions then match nothing, and a technology
// whose emitted price is the player's own list with nothing clamped in it
// carries a tooltip saying something was capped. The snapshot takes back
// whatever the tier arm wrote, and puts back exactly what was there before it.
//
// THE LOG IS STILL RETRACTED BY NAME, because a log line is a stream and not a
// slot: there is nothing to snapshot and put back, and retractLog's no-op on a
// line that was never written is what makes naming them safe.
func (r *resolution) noteAt(tgt noteTarget) string {
	if tgt.tech {
		return r.techNotes[tgt.index]
	}
	return r.recipeNotes[tgt.index]
}

func (r *resolution) restoreNote(tgt noteTarget, note string) {
	if tgt.tech {
		r.techNotes[tgt.index] = note
		return
	}
	r.recipeNotes[tgt.index] = note
}

// retractLog takes one accumulated log line back, the FIRST one that is exactly
// the given line, so the stream reads as if the walk had never written it.
//
// THE LINES ARE STILL ONLY ORDERED BY THE WALK. Removing one shifts the rest up
// and changes nothing else, which is what keeps the stream deterministic: the
// line is composed from the same pieces the writer used, so a line that was
// never written matches nothing and the call is a no-op.
func (r *resolution) retractLog(line string) {
	for i, have := range r.logs {
		if have == line {
			r.logs = append(r.logs[:i], r.logs[i+1:]...)
			return
		}
	}
}

// recipeChangeSentence is what changing a recipe costs a player who has already
// built with it, and it is the engine's doing rather than this library's.
//
// MEASURED on 2.0.77 by the consumer's second migration assessment: an
// assembling machine whose recipe changes has its input slots emptied of
// anything the new ingredient list does not use, up to eighty items destroyed
// outright rather than spilled on the ground, with no line anywhere. Nothing a
// mod emits can change it, so the only thing left is to say so before the
// player acts.
//
// ONE CONSTANT, TWO READERS. It is the tail of a recipe's tooltip note and the
// tail of the ERROR line an ingredient text falls back with, and the two must
// not drift apart.
const recipeChangeSentence = "Changing a recipe empties an assembling machine's input slots of anything the new list does not use."

// fallbackNote is the trailing line a prototype whose stored value was set
// aside carries in its own localised_description.
//
// THE LOG IS NOT A DISCLOSURE, which is the whole reason this exists. A player
// reads the settings screen, the recipe or technology tooltip and the
// changelog; a fkrecipes: line in factorio-current.log is evidence for a
// maintainer and a courtesy for the curious. Until this line, a player who
// typed something the library could not use had nothing where they look saying
// the game is not what they asked for.
//
// ENGLISH LITERALS, NEVER A LOCALE KEY, and that is measured rather than
// preferred. On 2.0.77 a localised string holding an UNDEFINED key drops the
// whole prototype description silently: no Unknown key marker, no empty line,
// the title and the ingredients still drawn, exit 0, and the engine's own dump
// holding the description verbatim, so no gate in this repository could see it.
// A key wrapped as {"?", {key}, "literal"} survives, but the library has no
// localisation channel for prototype prose at all: appendLocalised wraps a
// consumer's own Description as a literal too, and INVENTING a key would make
// every consumer owe an entry whose absence deletes the sentence it was meant
// to carry. Every sentence this library composes onto a SETTING is already an
// English literal for the same reason.
//
// THE ONE KEY A PROTOTYPE DOES CARRY IS NOT INVENTED AND IS NOT THIS SENTENCE.
// A note with no declared Description opens with descriptionRef's wrapper over
// the prototype's own [recipe-description] or [technology-description] entry,
// which is a key the author may already have written and owes nothing for; the
// note itself is still the literal after it. See descriptionRef for the shape
// and for the measurement that makes it safe.
//
// THE TAIL IS SCOPED BY WHAT MOVED, not by what kind of prototype carries it:
// destroysInputs is true only where the ingredient list itself changed. See
// noteOn.
//
// IT NAMES NO TARGET, and that is measured rather than tidy. The sentence used
// to say this mod's own choice applied instead, and on a recipe whose text sits
// beside a preset dropdown that is false on every preset but the declared one:
// the consumer measured all six and got six different ingredient lists under
// one byte-identical note. What is true on all six, and beside a field with no
// dropdown at all, and of a NUMBER setting as well as a text one, is the
// library's own rule: a stored value it cannot use behaves exactly as if the
// player had left the field alone. Naming the preset instead would mean
// hoisting readDropdown above the composer, which resolve keeps deliberately
// late; the generic is true without it.
func fallbackNote(setting string, destroysInputs bool) string {
	note := "The stored value of " + setting +
		" could not be used, so the game loaded as though that setting had been left alone. The reason is in the log."
	return withDestruction(note, destroysInputs)
}

// withDestruction is the one place the engine's own permanent cost is joined to
// a note, so a sentence about an emptied assembling machine cannot be written
// onto a prototype whose ingredient list did not move. See noteOn.
func withDestruction(note string, destroysInputs bool) string {
	if destroysInputs {
		return note + " " + recipeChangeSentence
	}
	return note
}

// The notes an ENVIRONMENTAL degradation leaves in the prototype's own
// description, in the voice fallbackNote established and for the same reason:
// THE LOG IS NOT A DISCLOSURE.
//
// ELEVEN OF THEM, AND THIS IS THE WHOLE LIST. Nine are below, in the order this
// file defines them; the last two are cycle.go's, beside the cycle LINES they
// go with, because the cycle walk runs after resolution. (The Rust half keeps
// that pair in data.rs, which is the one placement difference between the two
// lists.) A degradation added without a row here
// is the defect the consumer's third assessment measured: one arm of this same
// switch logged a line, wrote no note, and moved a technology to the root of the
// technology tree without saying so anywhere a player looks.
//
//	packDroppedNote       the chosen source lost SOME of its science packs
//	packlessSourceNote    the chosen source lost EVERY pack to the tool probe,
//	                      and a declared cost sits behind it
//	unpricedSourceNote    no source in the chosen ladder handed the library a
//	                      cost it could copy, so the declared fallback prices it
//	                      and the prerequisite goes with the source that never
//	                      answered
//	packlessNote          every pack the research names was put to the game and
//	                      the game had none of them
//	ingredientlessNote    every ingredient the RECIPE names was put to the game
//	                      and every ladder ran out, so it costs nothing to craft
//	unreadableSourceNote  the chosen source's pack list is in neither engine
//	                      form, and a declared cost sits behind it
//	unreadableCopyNote    the same list with nothing declared behind it, so the
//	                      unit is emitted with no packs at all
//	clampedItemNote       two ladders landed on one item above 65535
//	clampedFluidNote      two ladders landed on one fluid above 1e301
//	cyclePrereqNote       a prerequisite this plan made would loop the tree
//	cycleSpliceNote       a splice this plan made would loop the tree
//
// THEY ARE NOT FALLBACK NOTES AND MUST NOT READ AS ONE. Nothing was stored, so
// there is no field to go and fix and no "stored value" to name: the game
// itself is missing something the plan names, and the only honest thing to say
// is what is missing and what the library did instead. That is also why they do
// not go through playerFallback on the log side.
//
// SCOPED TO THE DEGRADATIONS AND NOT TO THE LADDER, AND THE SCOPE IS NARROWER
// THAN IT READS. A resolve-or-drop ingredient ladder THAT LEAVES THE RECIPE
// WITH SOMETHING TO CRAFT is the library's advertised contract, and the
// SETTINGS SCREEN is where that contract is disclosed: dropdownLadderLine on an
// ingredient dropdown's composed description and textLadderLine on a text
// setting's, both composed by this library. A ladder that leaves it with
// NOTHING is not that contract and carries ingredientlessNote, because "a
// shorter list than the one shown" reads straight past a recipe there is
// nothing left to craft. A clamped amount, a dropped science pack, an emptied
// unit, an emptied recipe, a dropped prerequisite and a technology left hanging
// off nothing are arithmetic and presence a player cannot check anywhere, so
// those get a note on the prototype instead.
//
// THAT JUSTIFICATION USED TO REST ON A SENTENCE THIS LIBRARY DID NOT WRITE. It
// quoted "the dropdown's own composed description already discloses it (where
// one names something your mods do not have, the nearest thing they do have is
// used instead)", and no composition here wrote that: the live text was the
// pilot consumer's own locale entry, which a consumer is free to write
// differently or not at all (their third migration assessment, finding 22). The
// three lines named above are that sentence, composed here, so the exclusion is
// now backed by the library's own output.
func packDroppedNote(name string) string {
	return "This game has no " + name + ", so this research was priced without it. The reason is in the log."
}

// packlessSourceNote is what a technology carries when the cost it copies named
// science packs and this game has none of them.
//
// IT STATES THE ENVIRONMENTAL FACT AND STOPS THERE, and that is a correction
// rather than a style. The sentence used to end "so this mod's own declared
// cost applies", which is a claim about the PRICE the player ends up with, and
// the price is not this branch's to describe: a CostFrom beside the tier lets
// the player override the count or the seconds while leaving the pack text at
// default, restoreNote fires only on a typed PACK LIST, so the note survives
// and a number in the emitted unit is the player's own. That is finding 19 one
// arm over, telling a player the mod's default applied where their own choice
// did, in a tooltip. What IS true whatever the settings beside it say is that
// the cost this research copies did not price it, so that is the whole
// sentence.
//
// THE ERROR LINE BESIDE IT KEEPS ITS OWN WORDING (packlessSourceLine, "so this
// mod's own declared cost applies instead"), and the two now differ on purpose.
// The line is AUTHOR-FACING and is written where the decision was made, before
// any player field is read, so "the declared cost applies" is exactly what the
// library did next and is true at that point in the walk. The note is
// PLAYER-FACING and is read after the whole resolution, beside numbers the
// player may have moved. Same event, two readers, two moments.
func packlessSourceNote(source string) string {
	return "This game has none of the science packs the " + source +
		" cost names, so that cost was not used to price this research. The reason is in the log."
}

// unpricedSourceNote is what a technology carries when NOT ONE source in the
// chosen tier's ladder handed this library a cost it could copy: the research
// is priced without one and hangs off nothing at all.
//
// IT IS THE LARGEST OF THESE DEGRADATIONS AND IT WAS THE ONLY SILENT ONE. The
// two arms beside it move a PRICE; this one also removes the PREREQUISITE, so
// the research sits at the root of the technology tree, researchable from the
// first minute, at whatever it is priced at instead. Measured by the consumer
// on a pack that renames one base technology: eight rows, every one exit 0, a
// log line, no note, and a research nobody had to earn. The criterion the other
// notes are written to, presence a player cannot check anywhere, applies to a
// missing prerequisite at least as strongly as to a dropped science pack.
//
// "A COST THIS MOD CAN USE HERE" IS THE WHOLE CLAIM, and each of its two halves
// is load-bearing. It does not say the sources carry no cost, because one of
// the ways this branch is reached is a source that EXISTS and carries a unit
// this library cannot copy: a unit that is not a dictionary, or one holding a
// subtree the copy drops. unreadableSourceNote's own comment already refuses
// that move one level down, where a sentence saying this game has none of those
// packs "would be stating something the library does not know"; the first draft
// of this one made exactly that mistake one level up. The phrase as written is
// true of an ABSENT source, a research_trigger source and a PRESENT BUT
// UNCOPYABLE one alike, which is every way the walk gets here.
//
// AND IT DOES NOT NAME THE PRICE IT LANDED ON, for the reason packlessSourceNote
// does not: a CostFrom beside the tier lets the player move the count or the
// seconds while the pack text stays at default, so "priced at this mod's own
// declared cost" is a claim about a number that may be theirs. "No copied cost"
// is true whatever the settings beside it say, and the prerequisite half is a
// fact about tree shape no setting touches.
//
// IT NAMES NO DROPDOWN VALUE, deliberately. The value the tier is on is a raw
// setting string an author picked for a settings file, and this sentence goes
// into a player's tooltip, where every other note is English prose. The ERROR
// line beside it already carries that value for an author reading a log, which
// is the reader it is a name for.
//
// AND IT TAKES NO ARGUMENT AT ALL, for the reason packlessNote takes none:
// there is no source to name, because not one of them answered.
func unpricedSourceNote() string {
	return "No technology this research takes its cost from carries a cost this mod can use here," +
		" so this research has no prerequisite and no copied cost. The reason is in the log."
}

// packlessNote is what a technology priced in NO science pack at all carries
// when every pack it named was PUT TO THE GAME and the game had none of them.
//
// IT IS NOT THE ONLY EMPTIED-UNIT NOTE, and the other one is the reason this
// sentence can say what it says. A copied unit whose pack list this library
// could not decode is emitted empty too, and there the list was never read, so
// nothing was asked and this sentence would be stating something the walk never
// established: that one carries unreadableCopyNote instead.
//
// IT TAKES NO ARGUMENT, deliberately. The names the walk asked the game about
// are in the ERROR line, where an author reading a log can use them; a player
// hovering a technology cannot act on a list of prototype names their mod set
// does not have.
func packlessNote() string {
	return "This game has none of the science packs this research names, so it takes no science pack at all." +
		" The reason is in the log."
}

// ingredientlessNote is packlessNote one prototype kind over: a RECIPE whose
// every declared entry was put to the game and dropped, so what is emitted is a
// recipe with no ingredients at all.
//
// A FREE CRAFT IS A BALANCE CHANGE NOBODY CHOSE, which is the whole of why it
// is here. The resolve-or-drop ladder is disclosed on the settings screen and
// its closing clause stops one step short of this: "a shorter list than the one
// shown" is true of a list that lost an entry and reads straight past a list
// that lost all of them.
//
// IT IS NOT THE DELIBERATE EMPTY LIST, and the distinction is the one
// unreadableCopyNote already draws against packlessNote. A list an author
// declared empty and the word none a player typed are both somebody's choice,
// and both are disclosed where the choice was made; this sentence is about a
// list that was NOT empty and became one. See addRecipe, which is the one place
// that can tell them apart.
//
// IT TAKES NO ARGUMENT, for the reason packlessNote takes none: the names the
// walk asked the game about are in the ERROR line, where an author reading a
// log can use them, and a player hovering a recipe cannot act on a list of
// prototype names their mod set does not have.
func ingredientlessNote() string {
	return "This game has none of the ingredients this recipe names, so it costs nothing to craft." +
		" The reason is in the log."
}

// unreadableSourceNote is what a technology whose copied cost could not be
// decoded carries when there IS a declared cost behind it. It is not
// packlessSourceNote, because nothing was dropped: the list was never read at
// all, and a sentence saying this game has none of those packs would be stating
// something the library does not know.
//
// IT ENDS ON THE ENVIRONMENTAL FACT for the reason packlessSourceNote does, and
// the reason is worth having in both places: the price this technology ends up
// at may hold a count or a seconds the player typed beside the tier, so a note
// claiming the mod's own declared cost applied can be false in a tooltip. What
// the walk knows, and all it knows, is that the copied cost did not price this
// research. The ERROR line beside it (unreadableSourceLine) keeps the author's
// wording and is unchanged.
func unreadableSourceNote(source string) string {
	return "The " + source + " cost this research copies cannot be read in this game," +
		" so that cost was not used to price this research. The reason is in the log."
}

// unreadableCopyNote is unreadableSourceNote's twin for the arm with NOTHING
// declared behind it: a CostOf whose copied pack list this library cannot
// decode is emitted with an empty ingredient list, and this is what the player
// is told where they look.
//
// IT IS NOT packlessNote, and the distinction is the one unreadableSourceNote
// already draws for the arm beside it. packlessNote says this game has none of
// the science packs the research names, which is a fact about the game; on this
// path the list was never decoded, so the library never asked the game about
// any pack and does not know that. What it does know is that it could not read
// the cost it was told to copy, and that is what the sentence says.
func unreadableCopyNote(source string) string {
	return "The " + source + " cost this research copies cannot be read in this game," +
		" so it takes no science pack at all. The reason is in the log."
}

// clampedItemNote and clampedFluidNote are one degradation with two ceilings,
// and they are two sentences because "what one slot holds" is true of an item
// stack and false of a fluid. The numbers are spelled here rather than
// formatted, for the reason mergedOpening's own comment gives.
func clampedItemNote(name string) string {
	return "Two ingredients resolved onto " + name +
		" and the total was above what one slot holds, so it was capped at " +
		strconv.FormatInt(maxItemAmount, 10) + ". The reason is in the log."
}

func clampedFluidNote(name string) string {
	return "Two ingredients resolved onto " + name +
		" and the total was above the largest amount the game can hold, so it was capped at 1e301." +
		" The reason is in the log."
}

// resolve asks the World everything the plan needs to know and records what
// degraded. The pass order IS the log order, and it is part of the contract
// the Rust mirror holds to: recipes in declaration order, then technologies
// in declaration order, and within a technology its enablement before its
// tree placement.
func (l *Lib) resolve(w World, prefix string) resolution {
	var res resolution

	// THE NOTE SLOTS ARE SIZED FROM THE DECLARATIONS, before the walk, so every
	// index the walk hands to noteOn is in range by construction and the emit
	// loop can read one per prototype without asking whether it exists.
	res.recipeNotes = make([]string, len(l.recipes))
	res.techNotes = make([]string, len(l.techs))

	// THE OVERLAY IS BUILT BEFORE ANY TEXT IS PARSED, which is the whole point
	// of it: a player's text may name an item THIS plan emits, and data.raw
	// does not have one yet. Every text path below takes it; the declared
	// ladders keep the real World. See planItemWorld.
	text := l.ownItemWorld(w, prefix)

	for ri, r := range l.recipes {
		tgt := noteTarget{index: ri}
		// The crafting time first, then the ingredients: a recipe's own field
		// before what it is made of, mirroring a technology's enablement
		// before its tree placement.
		var ct craftTime
		if r.spec.CraftTimeFrom.index != 0 {
			setting := l.settings[r.spec.CraftTimeFrom.index-1]
			ct.bound = true
			ct.setting = setting.emittedName(prefix)
			// A CRAFTING TIME IS A FIELD THE PLAYER OWNS, so a value the engine
			// would not take falls back to the setting's declared default with
			// one line rather than stopping the load. See playerFallback: the
			// generated minimum keeps a player from typing one of these, but a
			// second mod declaring the same setting name can hand one over, and
			// that is not something to lock a player out of their save for.
			ct.value = res.craftTimeNumber(w, setting, prefix, r.name, tgt)
		}
		res.craftTimes = append(res.craftTimes, ct)

		// The product is read ONCE per recipe, out of the same helper the
		// prototype builder reads it out of, and handed to every arm below:
		// four of them add a resolved list and each would otherwise have to
		// remember to compute it. See resolution.addRecipe.
		product := recipeProduct(prefix, l, r)

		// THE TEXT IS THE SWITCH, so it is read first and its answer decides
		// whether anything else is consulted at all. A text that says the
		// reserved word, and a text this library cannot use, both leave the
		// decision exactly where a player who typed nothing left it.
		declared := r.spec.Ingredients
		usedText := false
		var parsed parsedList
		if r.spec.IngredientsFrom.index != 0 {
			from := l.settings[r.spec.IngredientsFrom.index-1]
			parsed = l.resolveTextList(text, &res, from, prefix, r.spec.Category, listRecipe, tgt)
			usedText = !parsed.isDefault
			// Where there is no dropdown, the word default means the SETTING's
			// own declared list rather than the recipe's Ingredients, which
			// validateBindings refused beside it anyway.
			declared = from.defIngredients
		}
		if by := r.spec.IngredientsBy; by != nil {
			setting := l.settings[by.Setting.index-1]
			// READ WHATEVER THE TEXT SAID, because the line that sets the
			// choice aside has to name it. It is also the read that refuses a
			// stored value the dropdown does not offer, and a text in force is
			// no reason to stop asking that question.
			chosen := res.readDropdown(w, setting, prefix)
			if usedText {
				res.logs = append(res.logs, l.ingredientsFromLine(r.emittedName(prefix),
					l.settings[r.spec.IngredientsFrom.index-1].emittedName(prefix),
					setting.emittedName(prefix), chosen, parsed))
				// NOTHING WAS RESOLVED FROM ANYTHING: the list is the
				// player's own, already resolved by the language, so no
				// ladder could have emptied it. See addRecipe.
				res.addRecipe(tgt, r.name, product, nil, typedIngredients(parsed))
				continue
			}
			declared = choiceFor(by.Choices, chosen)
			list := l.resolveIngredients(w, &res, tgt, prefix, r.name, declared)
			// A plan that named things and got none of them is a recipe made
			// of nothing. The DEFAULT option is what applies then, because it
			// is the one the mod ships as its own answer.
			if len(declared) > 0 && len(list) == 0 && chosen != setting.defStr {
				res.logs = append(res.logs, "fkrecipes: "+r.name+": the "+chosen+
					" ingredients name nothing this game has, so the "+setting.defStr+" ingredients apply")
				// AND THE DECLARATION MOVES WITH THE LIST. What addRecipe is
				// handed has to be the declaration the FINAL list came from:
				// a default preset an author declared empty is a free craft
				// by choice, and reporting the chosen preset's entries there
				// would call it the environment's doing.
				declared = choiceFor(by.Choices, setting.defStr)
				list = l.resolveIngredients(w, &res, tgt, prefix, r.name, declared)
			}
			res.addRecipe(tgt, r.name, product, declared, list)
		} else if usedText {
			res.logs = append(res.logs, l.ingredientsFromLine(r.emittedName(prefix),
				l.settings[r.spec.IngredientsFrom.index-1].emittedName(prefix), "", "", parsed))
			res.addRecipe(tgt, r.name, product, nil, typedIngredients(parsed))
		} else {
			res.addRecipe(tgt, r.name, product, declared,
				l.resolveIngredients(w, &res, tgt, prefix, r.name, declared))
		}
	}

	for ti, t := range l.techs {
		var rt resolvedTech
		tgt := noteTarget{tech: true, index: ti}

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
			// THE NOTE SLOT AS IT STANDS BEFORE ANY PRICING, taken here so a
			// player's typed pack list can put it back in one call instead of
			// retracting the tier arm's sentences by name. Nothing below the
			// dropdown read has written a note yet, so this is the last point
			// at which the slot is still whatever the walk brought in. See
			// resolution.noteAt.
			noteBefore := res.noteAt(tgt)
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
				// THE FALLBACK IS RESOLVED IN TWO ARMS AND THIS IS THE
				// FIRST, which is the point of resolving it at resolution
				// rather than eagerly: its packs are probed when the fallback
				// is what applies and never otherwise. The SECOND arm is the
				// packless-source case below, where a source DID answer and
				// then lost every pack to the tool probe, so "a source
				// answered" does not end the pack question and a game with no
				// tool in it at all reaches the fallback either way. An
				// earlier reading of this comment said "only here" and "never
				// when a source answered", and a consumer's test built on it
				// went from green to a refused load; docs/migration.md states
				// the rule for consumers now. The line saying why comes first,
				// so the drops that follow read as consequences of it.
				res.logs = append(res.logs, "fkrecipes: "+t.name+": no source for the "+chosen+
					" cost carries a unit, so the fallback cost applies and the technology has no prerequisite")
				rt.unit = resolveUnit(w, &res, tgt, t.name, &by.Fallback, nil)
				// THE NOTE IS RECORDED AFTER THE FALLBACK IS RESOLVED, which
				// is how the two arms below do it and is load-bearing here for
				// the same reason. noteOn keeps the FIRST note per prototype,
				// and the fallback resolveUnit has just walked can leave this
				// technology with NO SCIENCE PACK AT ALL (packlessAt) or CLAMP
				// two of its own ladders landing on one name above the item
				// ceiling (mergePack). Offering this sentence last hands the
				// one slot to whichever of those happened.
				//
				// WHAT THE ORDERING BUYS IS THE PACKLESS CASE, and that one is
				// not a trade at all: a research with no science pack completes
				// for free the moment it is queued, so a player reading only
				// "no prerequisite and no copied cost" would be told about the
				// tree and left to discover the price by watching it finish.
				// The packless sentence is strictly the more urgent of the two.
				//
				// ON THE CLAMP PATH IT IS A TRADE AND THE PLAYER LOSES SOMETHING.
				// The clamp sentence names an amount capped at a ceiling and
				// says nothing about tree position, so a technology that both
				// lost its prerequisite and clamped a merged pack discloses only
				// the clamp: the ONE sentence that would have mentioned the tree
				// is the one that is dropped. That is ACCEPTED here, and the
				// reason is the slot rather than the ranking. noteOn keeps one
				// note per prototype (see the Fix round 3 open list in
				// agents/implementation-notes.md, where the one-note rule is
				// recorded as the thing to revisit), so with two degradations
				// and one slot SOMETHING is lost whichever order is chosen, and
				// a wrong price is the one a player acts on: they build for it.
				// A prerequisite they no longer need is visible in the
				// technology screen the moment they open it. Widening this to
				// two disclosures is the fix; reordering it is not.
				res.noteOn(tgt, unpricedSourceNote())
			} else {
				// THE PREREQUISITE MOVES WITH THE UNIT, and it still does when
				// the player has written over one of the tier's numbers: the
				// tier is what named a source, and the settings beside it price
				// the same rung rather than choosing another one.
				rt.prereqs = []string{source}
				// AND THE COPIED PACKS ARE PROBED, which is what keeps a mod
				// set from stopping the load on the default setting. See
				// filterCopiedPacks for the two engine refusals this replaces.
				filtered := filterCopiedPacks(&res, w, t.name, source, rt.unit)
				switch {
				case filtered.unreadable:
					// A COPIED LIST THIS LIBRARY CANNOT DECODE, AND A DECLARED
					// COST BEHIND IT: the same degradation the arm below makes,
					// for a different reason and in its own words. Nothing was
					// dropped, because nothing was read; the author's own
					// fallback unit is what the technology is priced in, and
					// the prerequisite and the level cap stay because the tier
					// still chose this rung. NOTHING IS CARRIED INTO THE
					// FALLBACK'S OWN LADDER either: there are no names to carry
					// when the list was never decoded.
					res.logs = append(res.logs, unreadableSourceLine(t.name, source))
					rt.unit = resolveUnit(w, &res, tgt, t.name, &by.Fallback, nil)
					res.noteOn(tgt, unreadableSourceNote(source))
				case filtered.hasList && filtered.kept == 0 && len(filtered.dropped) > 0:
					// EVERY PACK GONE, AND THERE IS A DECLARED COST BEHIND
					// THIS ONE: the author's own fallback unit is what the
					// technology is priced in, resolved through the same
					// ladder so its own absent rungs drop the same way. The
					// prerequisite and the level cap stay, because the tier
					// still chose this rung; only the price moved.
					res.logs = append(res.logs, packlessSourceLine(t.name, source))
					rt.unit = resolveUnit(w, &res, tgt, t.name, &by.Fallback, filtered.dropped)
					res.noteOn(tgt, packlessSourceNote(source))
				default:
					// A PACK DROPPED OUT OF A PRICE THE PLAYER CANNOT SEE gets
					// the note, and the FIRST one does: the tooltip says one
					// thing, and the log has the rest. A unit that kept
					// everything says nothing at all.
					if len(filtered.dropped) > 0 {
						res.noteOn(tgt, packDroppedNote(filtered.dropped[0]))
					}
					rt.unit = filtered.unit
				}
			}
			rt.hasUnit = true
			// THE THREE SETTINGS OVER THE TIER, where the technology declares
			// them. Through the value, never by name: a CustomCost holds a
			// PacksSettingRef, so packsSetting installed this and
			// validateTextSettings refused the plan if it had not. See Lib.lang
			// for the measurement this seam exists for.
			if c := t.spec.CostFrom; c != nil {
				unit, custom := l.customCost(l, w, text, &res, prefix, t, c,
					costTier{has: true, unit: rt.unit, dropdown: setting.emittedName(prefix), chosen: chosen, source: source, note: noteBefore}, tgt)
				if custom {
					rt.unit = unit
				}
			}
			res.techs = append(res.techs, rt)
			continue
		}

		// The hand-rolled cost, with each pack's ladder walked here rather than
		// at emit: which rung the game has is a fact about the game, and the
		// drops belong in the log stream at the point they were decided, before
		// this technology's tree placement.
		if t.spec.Unit != nil {
			rt.unit = resolveUnit(w, &res, tgt, t.name, t.spec.Unit, nil)
			rt.hasUnit = true
		}
		// THE COPIED COST, PROBED HERE RATHER THAN COPIED AT EMIT. The unit is
		// taken verbatim, which is what carries a count_formula and everything
		// else this library has never heard of across; what is NOT taken
		// verbatim any more is its science packs, because a pack this game
		// demoted or removed stops the load with a sentence naming neither this
		// mod nor the setting. See filterCopiedPacks.
		//
		// THERE IS NOTHING TO FALL BACK ON HERE, WHICH IS THE WHOLE DIFFERENCE
		// FROM A TIER. A CostOf source is a bare string with no declared ladder
		// behind it, so a copy that keeps no pack is emitted with an empty
		// ingredient list rather than priced on a cost this library would have
		// to invent. What that costs the player is a free research, and both
		// arms below say so where they look: see packlessAt.
		if t.spec.CostOf != "" {
			if u, ok := w.TechUnit(t.spec.CostOf); ok {
				filtered := filterCopiedPacks(&res, w, t.name, t.spec.CostOf, u)
				switch {
				case filtered.unreadable:
					// THE EMPTYING IS THE CALLER'S AND NOT THE FILTER'S, and
					// the reason is that the two callers of the filter do
					// different things with the same answer: the tier arm above
					// throws this unit away and prices the technology on the
					// author's declared cost, so a unit emptied inside the
					// filter would be built there and dropped. The filter
					// answers what it found; what to do about it is the
					// caller's, which is also why out.unit means one thing
					// (the unit as it arrived) on every path that does not read
					// out.kept.
					//
					// NOTHING IS INVENTED BY THE EMPTYING. The count, the time,
					// a count_formula and every field this library has never
					// heard of cross untouched; only the list it could not
					// decode is replaced, and it is replaced by the empty list
					// rather than by a guess at what was in it.
					//
					// THE NOTE AND THE LINE ARE BOTH THIS ARM'S OWN, because
					// this arm knows something packlessAt's pair does not say
					// and does NOT know something it does. The outcome is the
					// same emptied unit, but the REASON is that the copied list
					// could not be read at all: no pack was ever put to the
					// game, so "this game has none of the science packs this
					// research names" would be a fact this walk never
					// established. See unreadableCopyNote, which draws the same
					// line unreadableSourceNote draws for the arm above.
					res.logs = append(res.logs, unreadableCopyLine(t.name, t.spec.CostOf))
					res.noteOn(tgt, unreadableCopyNote(t.spec.CostOf))
					rt.unit = setUnitField(filtered.unit, "ingredients", Arr())
				case filtered.hasList && filtered.kept == 0 && len(filtered.dropped) > 0:
					res.packlessAt(tgt, t.name, filtered.dropped)
					rt.unit = filtered.unit
				default:
					if len(filtered.dropped) > 0 {
						res.noteOn(tgt, packDroppedNote(filtered.dropped[0]))
					}
					rt.unit = filtered.unit
				}
				rt.hasUnit = true
			}
		}
		// CostFrom is Unit with the three numbers read from settings, and it is
		// placed by the ORDINARY placement fields below rather than by a ladder
		// of its own: nothing moves with this unit, because no source
		// technology was named.
		if t.spec.CostFrom != nil {
			// Through the value, for the reason the CostBy arm above is. With
			// no tier the answer is always the custom cost: the three settings
			// are the whole price, and the word default in each of them means
			// that setting's own declaration.
			rt.unit, _ = l.customCost(l, w, text, &res, prefix, t, t.spec.CostFrom, costTier{}, tgt)
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
			// WHAT THE SPLICE REPLACED, recorded beside what it produced: the
			// anchor where one was taken out of the list, and the empty string
			// where the name was appended and nothing was. See rewriteRec.
			anchor := after
			if !replaced {
				anchor = ""
				list = append(list, newName)
				res.logs = append(res.logs, "fkrecipes: "+t.name+": "+before+" does not require "+after+", so the new technology is appended to its prerequisites")
			}
			res.rewrites = append(res.rewrites, rewriteRec{before: before, list: list, anchor: anchor})
			rt.rewrite = len(res.rewrites)
		}

		res.techs = append(res.techs, rt)
	}
	return res
}

// currentPrereqs is the prerequisite list a splice should build on: the one an
// earlier splice in this same plan already planned, or the game's own.
// A SPLICE THE CYCLE WALK DROPPED IS NOT THERE ANY MORE, so the walk's next
// pass builds its overlay out of what the plan will actually emit. During
// resolve nothing is dropped yet and this term costs a comparison.
func (r *resolution) currentPrereqs(w World, tech string) []string {
	if list, planned := r.plannedPrereqs(tech); planned {
		return list
	}
	return w.TechPrereqs(tech)
}

// plannedPrereqs is currentPrereqs' PLAN half on its own: the list an earlier
// splice in this same plan planned, and whether there was one at all.
//
// IT IS SPLIT OUT FOR ONE CALLER, checkCycles, which asks it once per
// technology per pass and must not re-ask the World alongside: the World's
// answer is invariant across passes and a host call is not a map lookup. See
// the measurement there.
func (r *resolution) plannedPrereqs(tech string) ([]string, bool) {
	for i := len(r.rewrites) - 1; i >= 0; i-- {
		if r.rewrites[i].dropped {
			continue
		}
		if r.rewrites[i].before == tech {
			// Cloned because the Rust mirror clones: an aliased list here is
			// a caller that can rewrite a planned splice through the slice it
			// was handed.
			return copyStrings(r.rewrites[i].list), true
		}
	}
	return nil, false
}

func dropLine(tech, after string) string {
	return "fkrecipes: " + tech + ": " + after + " is absent, so the prerequisite is dropped"
}

// unlockedRecipes marks the recipes some technology unlocks. Those are emitted
// disabled, because the research is what turns them on.
// checkResolvedCraftTimes is the post-condition on every bound crafting time,
// and it is the AUTHOR-SIDE twin of refuseCostNumbers.
//
// A VALUE THE PLAYER'S SETTING ANSWERED HAS ALREADY FALLEN BACK by the time
// this runs: craftTimeNumber holds it to the same two rules where it is read
// and answers with the setting's declared default when it fails one, logging
// one line. So the only world left for this loop is a DECLARED default the
// engine would not take, which validateSettings refuses at the settings stage
// (a bound setting's minimum has to clear the floor and its default has to
// clear the minimum). The engine runs that stage before the data stage, so this
// answers only for a host test that calls PlanData on its own, and it stays
// because what it protects is the invariant that no recipe this library emits
// carries an energy_required the engine refuses.
//
// THE TWO RULES ARE THE FALLBACK'S, THE TWO SENTENCES ARE NOT. craftTimeFault
// answers the same two questions in the same order on both sides, so the sides
// cannot drift about what is wrong; the wording says DECLARED DEFAULT here,
// because that is the number this loop is looking at. The stored value it
// replaced is gone, and a sentence saying the setting "answers" this would be
// describing a value nothing is holding any more.
func (l *Lib) checkResolvedCraftTimes(res resolution) error {
	for i, ct := range res.craftTimes {
		if !ct.bound {
			continue
		}
		if f := craftTimeFault(ct.value); f != faultNone {
			return errors.New(declaredCraftTimeProblem(l.recipes[i].name, ct.setting, f))
		}
	}
	return nil
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

// unlockingTech names the first technology in declaration order that unlocks
// the recipe at index i, or the empty string when none does. It runs DURING
// validation, before the technology loop has proved the handles, so a handle
// from another plan is skipped rather than followed: reading past this plan's
// recipes here would panic in front of the sentence that names it.
func (l *Lib) unlockingTech(i int) string {
	for _, t := range l.techs {
		for _, u := range t.spec.Unlocks {
			if l.validRecipe(u) && u.index-1 == i {
				return t.name
			}
		}
	}
	return ""
}

func itemProto(prefix string, it itemDecl) Value {
	pairs := []KV{
		kv("type", Str("item")),
		kv("name", Str(it.emittedName(prefix))),
	}
	pairs = appendLocalised(pairs, "item", it.emittedName(prefix), it.spec.DisplayName, it.spec.Description, "")
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

// recipeProduct is the EMITTED name of what a recipe makes: an item this plan
// declares (prefixed or legacy by its own declaration) or one that already
// exists, named verbatim because it is somebody else's and validation has
// probed it. Empty only for a recipe that declares neither, which validate
// refuses before resolution runs.
//
// ONE HELPER RATHER THAN TWO COPIES. The prototype builder writes this name
// into results and resolve compares it against the resolved ingredients; two
// spellings of one rule would let the log line and the prototype disagree about
// what the recipe makes.
func recipeProduct(prefix string, l *Lib, r recipeDecl) string {
	if r.result.index != 0 {
		return l.items[r.result.index-1].emittedName(prefix)
	}
	return r.spec.ResultNamed
}

func recipeProto(prefix string, l *Lib, r recipeDecl, ings []resolvedIngredient, ct craftTime, unlocked bool, note string) Value {
	pairs := []KV{
		kv("type", Str("recipe")),
		kv("name", Str(r.emittedName(prefix))),
	}
	pairs = appendLocalised(pairs, "recipe", r.emittedName(prefix), r.spec.DisplayName, r.spec.Description, note)
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
	// enabled sits HERE whoever wrote it. A consumer migrating a recipe onto
	// the library while its technology stays hand-rolled writes the field
	// through Extra, and it keeps the position the library's own would have
	// taken, so a golden captured before that migration does not move.
	// Validation has already proved nothing in this plan unlocks the recipe,
	// so the two writers can never disagree about it.
	if e, ok := extraField(r.spec.Extra, "enabled"); ok {
		pairs = append(pairs, kv("enabled", e))
	} else {
		pairs = append(pairs, kv("enabled", Bool(!unlocked)))
	}

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
	result := recipeProduct(prefix, l, r)
	pairs = append(pairs, kv("results", Arr(Obj(
		kv("type", Str("item")),
		kv("name", Str(result)),
		kv("amount", Num(float64(count))),
	))))
	if r.spec.Order != "" {
		pairs = append(pairs, kv("order", Str(r.spec.Order)))
	}
	return Obj(append(pairs, extraWithout(r.spec.Extra, "enabled")...)...)
}

// extraField reads one key out of an Extra list. Extra is a slice in
// declaration order and never a map, so this is a walk; the lists are the
// handful of fields one prototype carries.
func extraField(extra []KV, key string) (Value, bool) {
	for _, e := range extra {
		if e.Key == key {
			return e.Val, true
		}
	}
	return Nil(), false
}

// extraWithout is the same list minus one key, which is how a field the
// consumer wrote reaches its position among the library's own instead of the
// tail. The list itself is returned when the key is absent, so the ordinary
// declaration allocates nothing and keeps its order exactly.
func extraWithout(extra []KV, key string) []KV {
	for i, e := range extra {
		if e.Key != key {
			continue
		}
		out := make([]KV, 0, len(extra)-1)
		out = append(out, extra[:i]...)
		return append(out, extra[i+1:]...)
	}
	return extra
}

func techProto(prefix string, l *Lib, w World, t techDecl, rt resolvedTech, note string) Value {
	pairs := []KV{
		kv("type", Str("technology")),
		kv("name", Str(t.emittedName(prefix))),
	}
	pairs = appendLocalised(pairs, "technology", t.emittedName(prefix), t.spec.DisplayName, t.spec.Description, note)
	if t.spec.Icon != "" {
		pairs = append(pairs, kv("icon", Str(t.spec.Icon)))
	}
	if t.spec.IconSize != 0 {
		pairs = append(pairs, kv("icon_size", Num(float64(t.spec.IconSize))))
	}
	if len(rt.prereqs) > 0 {
		pairs = append(pairs, kv("prerequisites", strArr(rt.prereqs)))
	}
	pairs = append(pairs, kv("unit", techUnit(rt)))
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

func techUnit(rt resolvedTech) Value {
	// EVERY ARM SETTLES ITS COST DURING RESOLUTION NOW, CostOf with the rest:
	// a copied unit's science packs are a question about the game, so they are
	// asked where every other question about the game is asked and where a log
	// line can still be written. The flag is still read rather than dropped,
	// because a source that answers with no unit at all leaves it false and the
	// Rust mirror maps that absent case to the same nil.
	if rt.hasUnit {
		return rt.unit
	}
	return Nil()
}

// copiedPacks is what a verbatim-copied research unit came to after every
// science pack this game does not have was taken out of it.
//
// A COPIED UNIT IS SOMEBODY ELSE'S DECLARATION AND THE ENGINE DOES NOT FORGIVE
// IT. MEASURED on 2.0.77 build 84539, headless: a unit priced in a plain item
// refuses the load with `Error while running setup for technology prototype
// "tprobe-t" (technology): Invalid research unit (iron-plate). Research unit(s)
// can only be tool type items at the moment.`, and a name the game does not
// have at all fails earlier and more coarsely, naming neither the technology
// nor the property: `Error in assignID: item with name 'water' does not exist.`
// So a pack that a modpack demoted from tool to item, or removed, stops the
// load on the DEFAULT setting with no fkrecipes: line anywhere. The filter asks
// BOTH questions at once, because ToolExists is the one probe that answers
// them: a name that is not a tool-type item this game has is not a rung.
type copiedPacks struct {
	// unit is the copied unit with its ingredients array filtered, the rest of
	// it in the order and the shape it arrived in.
	unit Value
	// hasList is whether the unit carried an ingredients array at all. A unit
	// that carries none is left exactly as it was: there is nothing to filter
	// and nothing to say.
	hasList bool
	// kept is how many packs survived, dropped the names that did not, in the
	// copied unit's own order.
	kept    int
	dropped []string
	// unreadable is an entry in NEITHER engine form, which is a unit this
	// library cannot copy faithfully. A form it cannot decode is not a licence
	// to pass it through: the name might be one the game does not have, and the
	// refusal it would earn names neither the technology nor the property.
	unreadable bool
}

// filterCopiedPacks drops the science packs a copied research unit names that
// this game does not have, one log line each, and answers what became of it.
//
// THE DROP LINE IS THE LADDER'S VOICE with the source named, because the author
// did not write this list: the technology it was copied out of did.
//
// BOTH ENGINE FORMS ARE DECODED. The engine takes the short tuple
// {"automation-science-pack", 1} and the long {name = ..., amount = ...} alike
// and base writes the short one; only the NAME is read out, and the entry
// itself is what is kept, so an amount this library does not model and a field
// no version of it has heard of survive the filter untouched.
func filterCopiedPacks(res *resolution, w World, tech, source string, unit Value) copiedPacks {
	out := copiedPacks{unit: unit}
	if unit.Kind != KindMap {
		return out
	}
	for _, e := range unit.Map {
		if e.Key != "ingredients" {
			continue
		}
		if e.Val.Kind != KindArr {
			// AN ingredients KEY THAT IS NOT AN ARRAY IS A FORM THIS LIBRARY
			// CANNOT DECODE, and a form it cannot decode is not a licence to
			// pass it through: the caller degrades on it, exactly as it does
			// for an ENTRY in neither form, to the declared cost behind a tier
			// or to an emptied unit where there is none. What would otherwise
			// happen is worse: the unit crosses unfiltered and the engine
			// answers about a science pack, naming neither this mod nor the
			// property.
			out.hasList = true
			out.unreadable = true
			return out
		}
		out.hasList = true
		kept := make([]Value, 0, len(e.Val.Arr))
		for _, item := range e.Val.Arr {
			name, ok := copiedPackName(item)
			if !ok {
				out.unreadable = true
				return out
			}
			// A NAME THIS LIBRARY CANNOT PUT TO THE WORLD IS KEPT UNASKED, and
			// the empty name is that answer. Another mod's science pack can be
			// named with bytes that are not UTF-8; fkdata hands those over
			// unchanged and a copied unit has always crossed byte-exact.
			// Rust's Value has a Bytes variant those arrive in and its
			// tool_exists takes a &str, so that half CANNOT ask; this half
			// could, and does not, because the two halves answer alike.
			if name == "" {
				kept = append(kept, item)
				continue
			}
			if w.ToolExists(name) {
				kept = append(kept, item)
				continue
			}
			out.dropped = append(out.dropped, name)
			res.logs = append(res.logs, "fkrecipes: "+tech+": "+name+
				" is not a science pack this game has, so it is left out of the "+source+" cost")
		}
		out.kept = len(kept)
		out.unit = setUnitField(unit, "ingredients", Arr(kept...))
		return out
	}
	return out
}

// copiedPackName is the name out of one entry of a copied unit's ingredients,
// in either of the two forms the engine takes.
//
// THREE ANSWERS IN TWO VALUES. `false` is an entry in NEITHER form, which the
// caller degrades on; the EMPTY name beside `true` is an entry in a form whose
// name is not text this library can put to the World, which the caller keeps
// unasked. No prototype is named by the empty string, so the sentinel names
// nothing real.
func copiedPackName(v Value) (string, bool) {
	if v.Kind == KindArr && len(v.Arr) >= 1 && v.Arr[0].Kind == KindStr {
		return askableName(v.Arr[0].Str), true
	}
	if v.Kind != KindMap {
		return "", false
	}
	for _, e := range v.Map {
		if e.Key != "name" {
			continue
		}
		// THE FIRST name KEY ANSWERS, whatever it holds. Walking past one whose
		// value is not a string would let a later duplicate key answer instead,
		// and the Rust mirror stops at the first: two halves that disagree
		// about which key wins is a divergence waiting for the map that has
		// two.
		if e.Val.Kind != KindStr {
			return "", false
		}
		return askableName(e.Val.Str), true
	}
	return "", false
}

// askableName is a prototype name this library can hand to a World, or the
// empty string where it cannot. See copiedPackName for why the Rust mirror
// decides the same question by the value's own variant.
func askableName(s string) string {
	if !utf8.ValidString(s) {
		return ""
	}
	return s
}

// unreadableUnitPhrase is the FACT both lines about an undecodable copied unit
// open with, and it is one composer because the two differ only in what the
// library did next. It keeps the CostOf family's own wording, because it is the
// same fact about the same value: a table that cannot be copied faithfully.
//
// IT NAMES THE TECHNOLOGY RATHER THAN WEARING THE CostOf( PREFIX, because the
// same filter runs over a CostBy tier's chosen source, where there is no CostOf
// to name.
func unreadableUnitPhrase(tech, source string) string {
	return tech + ": the unit of " + source + " holds a table this library cannot copy faithfully"
}

// unreadableSourceLine is what an undecodable copied unit logs where there IS a
// declared cost behind it, and unreadableCopyLine what it logs where there is
// not. Both are ERROR lines because the technology is not priced the way
// anybody declared it, and neither goes through playerFallback: nothing was
// stored and nothing was typed, so there is no field to send anybody to.
func unreadableSourceLine(tech, source string) string {
	return messagePrefix + "ERROR: " + unreadableUnitPhrase(tech, source) +
		", so this mod's own declared cost applies instead"
}

func unreadableCopyLine(tech, source string) string {
	return messagePrefix + "ERROR: " + unreadableUnitPhrase(tech, source) +
		", so the research is emitted with no science pack and completes for free"
}

// packlessLine is what a technology priced in no science pack at all logs, and
// it names every rung the walk asked the game about.
//
// THE NAMES ARE PART OF THE ANSWER. "this research names no science pack this
// game has" on its own tells an author that something is absent and not which
// thing, and the author reading it is one whose ladders all missed: the names
// are the rungs they wrote and the one thing that says which mod set this is.
func packlessLine(tech string, names []string) string {
	return messagePrefix + "ERROR: " + tech + ": none of " + strings.Join(names, ", ") +
		" is a science pack this game has, so the research is emitted with no science pack and completes for free"
}

// ingredientlessLine is packlessLine's recipe twin: every entry this recipe
// declared was put to the game and every ladder ran out.
//
// IT NAMES NO RUNG, and that is the one place it differs from packlessLine.
// Each entry that dropped already logged its own line naming every candidate it
// tried ("none of a, b is present, so the ingredient is dropped"), so the rungs
// are directly above this line in the same stream; repeating them here would
// print the same names twice. What this line adds is the thing no per-entry
// line can say, that NOTHING was left.
func ingredientlessLine(recipe string) string {
	return messagePrefix + "ERROR: " + recipe +
		": this game has none of the ingredients this recipe names," +
		" so it is emitted with no ingredients and costs nothing to craft"
}

// packlessSourceLine is what a copied cost that named no science pack this game
// has logs, and it is an ERROR because the technology is not priced the way
// anybody declared it.
//
// IT IS NOT A PLAYER'S FALLBACK AND HAS ITS OWN COMPOSER FOR THAT REASON.
// Nothing was stored and nothing was typed: there is no field on the settings
// screen to send anybody to, so playerFallback's tail would be advice about a
// value that does not exist. The two cannot drift, because neither reads the
// other.
func packlessSourceLine(tech, source string) string {
	return messagePrefix + "ERROR: " + tech + ": the " + source +
		" cost names no science pack this game has, so this mod's own declared cost applies instead"
}

// techMaxLevel is the level cap a technology carries, if any. A hand-rolled
// cost has none; a copied one carries the source's.
func techMaxLevel(w World, t techDecl, rt resolvedTech) (Value, bool) {
	if t.spec.CostBy != nil {
		return rt.maxLevel, rt.hasMaxLevel
	}
	// A hand-rolled cost has no source technology to read a cap from, and a
	// custom one has none either: both are prices this plan wrote, not copies
	// of somebody else's multi-level research.
	if t.spec.Unit != nil || t.spec.CostFrom != nil {
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
// so. A unit that loses ALL of them is emitted EMPTY, with a line and a tooltip
// of its own rather than a refusal, because the dialog a refusal raises cannot
// reach the settings screen the player would fix it from: see packlessAt.
func resolveUnit(w World, res *resolution, tgt noteTarget, tech string, u *UnitSpec, alreadyTried []string) Value {
	resolved, tried := resolvePackLadders(w, res, tgt, tech, u.Packs)
	packs := make([]Value, 0, len(resolved))
	for _, p := range resolved {
		packs = append(packs, Arr(Str(p.name), Num(float64(p.amount))))
	}
	// A cost that named packs and got none of them. A unit that DECLARED none
	// never reaches here: plan validation refused it before any of this was
	// asked, which is what leaves this sentence to the world's answer alone.
	if len(packs) == 0 {
		// EVERY RUNG THE WALK ASKED ABOUT, COPIED PACK FIRST. alreadyTried is
		// what a caller asked the game before this unit was built at all: the
		// science packs a chosen tier's copied unit named and lost. An author
		// reading the line is one whose ladders all missed, and the one thing
		// that says which mod set this is is the whole list of names, in the
		// order they were asked. ONCE EACH is packlessAt's rule and not this
		// caller's: see there.
		res.packlessAt(tgt, tech, append(append([]string{}, alreadyTried...), tried...))
	}
	return Obj(
		kv("count", Num(float64(u.Count))),
		kv("time", Num(u.Seconds)),
		kv("ingredients", Arr(packs...)),
	)
}

// resolvePackLadders walks each declared pack's ladder and drops what the game
// does not have, answering with the packs that survived.
//
// IT IS THE ONE PLACE THE DROP LINE IS WRITTEN, which is what the design record
// asks for: a pack that resolves to nothing is dropped with a log line
// WHEREVER a unit is built, so the hand-rolled Unit, the CostBy Fallback and a
// pack list left on the word default all say the same sentence.
//
// THE LADDER ASKS ToolExists AND NOTHING ELSE. MEASURED: a research unit priced
// in a plain item refuses the load with "Invalid research unit (iron-plate).
// Research unit(s) can only be tool type items at the moment", and a fluid name
// in a unit refuses with "Error in assignID: item with name 'water' does not
// exist". Asking ItemExists would let either of those through as a rung.
// IT ALSO ANSWERS WHAT IT TRIED, in declaration order, because the line a unit
// that kept nothing gets has to name the names: see packlessAt.
// REPEATS ARE LEFT IN, because this walk is one of four callers feeding that
// sentence and none of them can see the others: packlessAt is the one place
// the once-each rule is applied. Collected on every walk rather than only on
// the empty one, so the answer costs the same branch whatever the game holds.
func resolvePackLadders(w World, res *resolution, tgt noteTarget, tech string, packs []Pack) (ingredientList, []string) {
	out := make(ingredientList, 0, len(packs))
	tried := make([]string, 0, len(packs))
	for _, p := range packs {
		candidates := p.ladder()
		picked := ""
		for _, c := range candidates {
			if w.ToolExists(c) {
				picked = c
				break
			}
			tried = append(tried, c)
		}
		if picked == "" {
			res.logs = append(res.logs, "fkrecipes: "+tech+": none of "+strings.Join(candidates, ", ")+
				" is present, so the science pack is dropped")
			continue
		}
		// TWO LADDERS CAN LAND ON ONE PACK, exactly as two ingredient ladders
		// can, and the emitted form here is the SHORT TUPLE rather than the
		// recipe's dict. See mergePack.
		out = mergePack(res, tgt, out, tech, listEntry{name: picked, amount: p.Amount})
	}
	return out, tried
}

// appendOnce keeps a slice in first-seen order with no repeats. Its one caller
// is packlessAt, which is where the reason it is needed is written.
func appendOnce(list []string, name string) []string {
	for _, seen := range list {
		if seen == name {
			return list
		}
	}
	return append(list, name)
}

// packlessRec is one technology that went packless and the exact line it
// logged, so the one site that can take it back has the string the writer used.
//
// KEYED ON THE DECLARATION INDEX AND NOT ON THE NAME. A declared name is not
// unique: validate refuses two technologies whose EMITTED names collide, and a
// legacy declaration keeps its name unprefixed, so a legacy technology and an
// ordinary one can legally both be called "steel-axes". A retraction matching
// on the name would take back a line that is still true and belongs to the
// other one. The index is already threaded to every site through noteTarget.
type packlessRec struct {
	index int
	line  string
}

// packlessAt is what a technology priced in no science pack at all earns: the
// unit is emitted with an empty ingredient list, one ERROR line names every
// rung the walk asked the game about, and the technology's own tooltip says
// what it costs the player.
//
// A COST WITH NO PACKS IS NOT A CHEAP RESEARCH, IT IS A FREE ONE, and that is
// measured in play rather than only as far as the load. On 2.0.77 build 84539 a
// unit of {count = 10, time = 15, ingredients = {}} loads with exit 0 and no
// engine line, force.add_research returns true, the research queue takes it,
// progress advances in a lab holding nothing and the technology COMPLETES after
// count * time ticks. So emitting one is a balance change the player did not
// choose, which is exactly why it is disclosed where they look.
//
// IT USED TO BE A REFUSAL AND THAT WAS A LOCK-OUT. A mod set that demotes one
// science pack could stop the load on the DEFAULT setting, and the client
// cannot reach the Mod Settings screen from an "Error loading mods" dialog
// (re-measured on 2.0.77: see fallbackFact). A player whose pack was demoted
// had no way back into the game that did not disable the mod. A free research
// they are told about is worse than the research they asked for and better than
// no game, and the threat model grades it that way.
//
// IT IS PER TECHNOLOGY, not first-in-plan-order. The old carrier held ONE
// technology because a refusal is one sentence; a line and a tooltip are per
// prototype, so two packless technologies are two lines and two tooltips.
//
// AND THIS IS THE ONE PLACE THE ONCE-EACH RULE IS APPLIED, on the way into the
// sentence. It belongs to the single WRITER rather than to any caller because
// no caller can see another's list: two declared ladders ending on one absent
// rung would print that rung twice, a copied unit naming one absent pack twice
// would print it twice, and a fallback rung repeating a pack the copied unit
// already lost would print it twice across two producers. FIRST-SEEN ORDER,
// because the sentence is the walk's own order: the copied unit's lost packs
// first, then the declared ladders in declaration order with each ladder's
// rungs in ladder order.
func (r *resolution) packlessAt(tgt noteTarget, tech string, tried []string) {
	names := make([]string, 0, len(tried))
	for _, name := range tried {
		names = appendOnce(names, name)
	}
	line := packlessLine(tech, names)
	r.logs = append(r.logs, line)
	r.noteOn(tgt, packlessNote())
	r.packlessSaid = append(r.packlessSaid, packlessRec{index: tgt.index, line: line})
}

// retractPacklessLine takes back the LINE one technology earned from
// packlessAt, for the one caller that can: a player's typed pack list writing
// over the very unit that went packless. See resolveCustomCost.
//
// THE LINE ONLY, because the note that went with it is taken back by the
// snapshot the same caller restores, along with everything else the tier arm
// wrote. See noteAt.
//
// THE LINE IS READ BACK RATHER THAN RECOMPOSED, because the names in it were
// asked by a walk the retracting site never saw.
func (r *resolution) retractPacklessLine(tgt noteTarget) {
	for i, rec := range r.packlessSaid {
		if rec.index != tgt.index {
			continue
		}
		r.retractLog(rec.line)
		r.packlessSaid = append(r.packlessSaid[:i], r.packlessSaid[i+1:]...)
		return
	}
}

// appendLocalised is the ONE writer of localised_name and localised_description
// on every prototype this library emits, and the note is the trailing line a
// recipe or a technology carries when a stored value was set aside.
//
// FOUR SHAPES. An author's description with no note is {"", "<description>"}
// byte for byte as it always was, so a golden taken before this line existed
// does not move for a load nothing fell back on; a description with a note adds
// the note as a further parameter opening with a newline; a note with NO
// description opens with descriptionRef's wrapper, which resolves to the
// author's own [<kind>-description] entry plus a newline where they wrote one
// and to nothing where they did not; and nothing at all is emitted when there
// is neither, so the engine resolves that entry on its own exactly as it always
// did. Each is the shape of the SHORT case: a part over the chunk budget is
// more than one parameter, and the parameters concatenate to the same bytes.
//
// THE THIRD SHAPE IS WHY THE KIND AND THE EMITTED NAME ARE PARAMETERS. A
// prototype's own localised_description field WINS OVER the locale entry, so
// before that wrapper an author who wrote their description the ordinary
// Factorio way, in a .cfg rather than in the plan, had it DISPLACED by the note
// for the whole of that load. The declared Description arm does NOT compose the
// key, and that is the deliberate half of it: an author who put their
// description in the plan wrote the literal that takes the entry's place
// already, and composing both would print it twice.
//
// AN ITEM NEVER CARRIES A NOTE, so itemProto passes the empty string. The
// fallback is about what a recipe makes or what a technology costs, and an item
// prototype is neither, so an item never reaches the third shape and never
// composes a key. Keep it that way: the kind and the name an item passes are
// never read.
//
// EVERY PARAMETER IS CHUNKED AND THE WHOLE IS GROUPED, because the engine
// polices ONE STRING ELEMENT at 200 BYTES on a data-stage prototype and the
// three notes a RECIPE can carry reach 229, 229 and 246 bytes before any name
// goes into them: before the splitter, no consumer on any mod name could bind a
// text setting to a recipe's ingredient list and have the resulting fallback
// load at all. Each figure is a sentence of its own plus one space plus the 100
// bytes of recipeChangeSentence: 128 + 1 + 100 for fallbackNote, 128 + 1 + 100
// for clampedItemNote and 145 + 1 + 100 for clampedFluidNote.
//
// THE TAIL IS WHAT MOVED AND NOT WHAT CARRIES IT, which is withDestruction's
// whole rule and is why those three are MAXIMA rather than fixed widths. A
// recipe whose CRAFTING TIME fell back takes fallbackNote with destroysInputs
// false (craftTimeNumber in customize.go), so its note is the bare 128 bytes
// and no tail at all: an ingredient list that did not move empties no
// assembling machine. TestFallbackNoteShape pins both halves of that.
//
// A TECHNOLOGY's notes never take the tail, because a research costs no
// assembling machine anything, and the longest of them before a name goes in is
// unpricedSourceNote's 168, which takes no name at all and so is one element on
// every mod set; it is chunked all the same, because nothing here decides per
// sentence. See localisedChunkBudget and chunkLocalised for the measurement and
// for the properties the split has by construction.
//
// THE DESCRIPTION AND THE NOTE ARE CHUNKED SEPARATELY, so the newline stays at
// the head of the note's first chunk; a short description with a short note is
// two parameters and keeps the shape it always had.
//
// THE GROUPING HELPER RATHER THAN THE FLAT ONE, because the parameter count
// here is UNBOUNDED: a consumer's Description is unbounded, and a long one is
// as many chunks as it takes. localisedGroup answers the other measured ceiling
// (20 PARAMETERS PER TABLE and 20 LEVELS OF NESTING DEPTH, measured on 2.0.77:
// the 21st of either refuses the load by name, and a description holding 421
// tables at depth 3 loads, so there is no global table budget) by keeping the
// first nineteen parameters and handing the rest to a nested group in the
// twentieth slot. That is 19*(d-1)+20 parameters at depth d, so 381 chunks at
// the measured depth ceiling of 20, which is 68,580 bytes of one description at
// 180 bytes a chunk. Past that it is a consumer's own declared description that
// refuses, and it refuses on DEPTH rather than on the element rule.
func appendLocalised(pairs []KV, kind, name, displayName, description, note string) []KV {
	if displayName != "" {
		pairs = append(pairs, kv("localised_name", localised(displayName)))
	}
	switch {
	case description != "" && note != "":
		params := localisedChunks(description)
		params = append(params, localisedChunks("\n"+note)...)
		pairs = append(pairs, kv("localised_description", localisedGroup(params)))
	case description != "":
		pairs = append(pairs, kv("localised_description", localised(description)))
	case note != "":
		params := localisedChunks(note)
		// The key form is dropped rather than composed where it would not fit
		// the engine's element ceiling, which leaves the shape this arm had
		// before the wrapper existed. See descriptionRef.
		if ref, ok := descriptionRef(kind, name); ok {
			params = append([]Value{ref}, params...)
		}
		pairs = append(pairs, kv("localised_description", localisedGroup(params)))
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
			// FixedPointNumber.hpp with the crash handler; the wall itself is
			// 1.0715086071862672e301, see maxFluidAmount). A crash is not
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
		// THE ENGINE'S CEILING, and the one the language already holds a TYPED
		// list to. MEASURED: amount=65536 refuses the load with "Value (65536)
		// outside of range. The data type allows values from 0 to 65535". A
		// declared list above it used to reach the engine and be refused there,
		// which blames the consumer's mod for a number its author wrote.
		//
		// AFTER THE HANDLE CHECK, because the sentence names the ingredient and
		// this plan's own item has no candidate to name: the handle is proved
		// first, so the declared name is safe to read.
		if ing.amount > maxItemAmount {
			name := ""
			if len(ing.candidates) > 0 {
				name = ing.candidates[0]
			} else {
				name = l.items[ing.item.index-1].name
			}
			return errors.New(at + who + " takes " + strconv.FormatInt(ing.amount, 10) + " of " + name +
				", and an item amount goes up to " + strconv.FormatInt(maxItemAmount, 10))
		}
	}
	return nil
}

// validateNoDuplicates refuses a DECLARED list that names one thing twice.
//
// IT IS THE OTHER HALF OF THE MERGE, and the two are not the same problem. A
// ladder that COLLAPSES onto a name the list already carries is a mod set
// taking an ingredient away, and the amounts are added so the recipe still
// loads for a player who never opened the settings. A duplicate the AUTHOR
// WROTE OUT is a bug in the declaration, and adding it up silently would emit
// a recipe nobody designed and say nothing: `4 iron-plate, 2 iron-plate`
// composed a dropdown description the player's own custom field refuses, while
// the recipe quietly said 6. Refused here, because an author's bug is caught
// in development and a player is not locked out of a game.
//
// AFTER THIS, THE MERGE IS REACHABLE ONLY FROM A LADDER COLLAPSE, which is
// what makes the merge line's phrase "after the fallbacks" true wherever it
// appears.
//
// THE HEADS ARE COMPARED AS RESOLUTION WILL SEE THEM: a ladder by its first
// rung, this plan's own item by the name it is EMITTED under, which is the
// pair declaredHead answers with and the pair a description already shows. Any
// looser comparison would leave a collision that is plain in the declaration
// to be found later by the merge, and the sentence about fallbacks would be
// false about it.
//
// KIND IS PART OF THE IDENTITY. An item and a fluid of one name are two
// ingredients, and the engine takes both in one recipe (measured: base carries
// parameter-0 to parameter-9 as an item AND a fluid).
//
// LAST, AND ACROSS THE WHOLE LIST, exactly where the language checks it: it is
// the one problem no single entry can see. Every caller runs validateIngredients
// first, which is what proves a handle before declaredHead reads it.
func (l *Lib) validateNoDuplicates(at, who, prefix string, ings []Ingredient) error {
	names := make([]string, len(ings))
	for i, ing := range ings {
		names[i] = l.declaredHead(prefix, ing)
	}
	// The SECOND occurrence is what the walk reports, with the earliest
	// partner, which is the order the language's own duplicate rule uses.
	for j := 1; j < len(ings); j++ {
		for i := 0; i < j; i++ {
			if ings[i].kind == ings[j].kind && names[i] == names[j] {
				return errors.New(at + who + " names " + names[j] + " twice; each ingredient is taken once")
			}
		}
	}
	return nil
}

// validateNoDuplicatePacks is validateNoDuplicates for a declared pack list.
//
// NO KIND AND NO PREFIX. A science pack is asked for with ToolExists, a tool
// is an item, and this library declares no tools, so a pack list has one
// namespace and the declared names alone decide identity.
func validateNoDuplicatePacks(at, who string, packs []Pack) error {
	for j := 1; j < len(packs); j++ {
		for i := 0; i < j; i++ {
			if packs[i].Name == packs[j].Name {
				return errors.New(at + who + " names " + packs[j].Name + " twice; each science pack is taken once")
			}
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
		// THE SAME 16 BITS AN ITEM INGREDIENT IS HELD IN, and it is the
		// engine's own rule rather than an analogy. MEASURED (Factorio 2.0.77,
		// build 84539, mac-arm64, steam), on a technology whose unit
		// ingredients carry one pack: 65535 loads and dumps as written; 65536,
		// 2^31 and 2^53 each refuse the load with "Value (<n>) outside of
		// range. The data type allows values from 0 to 65535 in property tree
		// at ROOT.technology.<name>.unit.ingredients[0][1]", exit 1, no dump.
		// A declared pack above it used to reach the engine and fail the whole
		// load, blaming the consumer's mod for a number its author wrote,
		// which is the same defect the ingredient ceiling closed.
		//
		// AFTER THE NAME CHECKS, so the sentence has a name to quote.
		if p.Amount > maxItemAmount {
			return errors.New(at + "the technology " + name + " takes " + strconv.FormatInt(p.Amount, 10) +
				" of " + p.Name + ", and a science pack amount goes up to " + strconv.FormatInt(maxItemAmount, 10))
		}
	}
	return validateNoDuplicatePacks(at, "the technology "+name, u.Packs)
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
//
// A STORED VALUE THE DROPDOWN DOES NOT OFFER IS REFUSED. It is unreachable
// through the engine, which resets such a value to the default before any stage
// runs (measured), and reachable through a hand-edited mod-settings.dat. What
// it used to do was worse than a refusal: the choice lookup found no plan,
// the recipe came out made of nothing, and no line said so. This is the
// pilot's own finding, closed.
func (r *resolution) readDropdown(w World, s settingDecl, prefix string) string {
	full := s.emittedName(prefix)
	v, ok := w.StartupSetting(full)
	if !ok || v.Kind != KindStr {
		r.logs = append(r.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
		return s.defStr
	}
	for _, allowed := range s.values {
		if allowed == v.Str {
			return v.Str
		}
	}
	r.refuse("fkrecipes: " + full + ` holds "` + v.Str + `", which is not one of its values`)
	return v.Str
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
//
// WHAT IT ANSWERS WITH NEVER NAMES ONE THING TWICE. Two ladders can land on
// one rung, and the engine refuses the whole load for it (MEASURED, on the
// pilot's own recipe: Error while running setup for recipe prototype
// "bbb-balancer-part" (recipe): Duplicate item ingredients are not allowed
// (iron-plate exists 2 or more times), exit 1, no dump). See mergeIngredient.
func (l *Lib) resolveIngredients(w World, res *resolution, tgt noteTarget, prefix, recipe string, ings []Ingredient) []resolvedIngredient {
	list := make([]resolvedIngredient, 0, len(ings))
	for _, ing := range ings {
		if len(ing.candidates) == 0 {
			own := l.items[ing.item.index-1].emittedName(prefix)
			list = mergeIngredient(res, tgt, list, recipe, own,
				resolvedIngredient{name: own, amount: ing.amount})
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
			list = mergeIngredient(res, tgt, list, recipe, ing.candidates[0],
				resolvedIngredient{kind: kindFluid, name: picked, fluid: ing.fluidAmount})
			continue
		}
		list = mergeIngredient(res, tgt, list, recipe, ing.candidates[0],
			resolvedIngredient{name: picked, amount: ing.amount})
	}
	return list
}

// mergeIngredient adds one resolved ingredient to a list that may already
// carry its name, and is the reason nothing this library emits can name the
// same item, or the same fluid, twice.
//
// MEASURED, and it is what this exists for: a recipe whose ingredient list
// names one item twice refuses the WHOLE LOAD with "Error while running setup
// for recipe prototype "bbb-balancer-part" (recipe): Duplicate item
// ingredients are not allowed (iron-plate exists 2 or more times)", exit 1,
// no dump, no line naming a setting or a missing item. A ladder is exactly
// what produces one: the pilot's ladders all end on iron-plate, so a mod set
// without transport-belt resolves a third rung onto a name the list already
// carries and a player who never opened the settings cannot load the game.
//
// THE AMOUNTS ARE ADDED, IN THE POSITION OF THE FIRST OCCURRENCE, so
// declaration order survives the merge. A list is a slice walked in order and
// the merged entry is written back where it already sat; nothing is moved,
// which is what keeps the emitted order the declared one.
//
// AN ITEM AND A FLUID OF ONE NAME DO NOT MERGE, because the kind is part of
// the identity: the engine takes both in one recipe (measured: base carries
// parameter-0 to parameter-9 as an item AND a fluid), and adding a count to
// an amount would be adding two different things. The mismatched pair falls
// through to the next entry in either order, which is what lets a list hold
// one of each whichever the author declared first.
//
// ONLY A LADDER COLLAPSE REACHES HERE. A list that named one thing twice in
// the declaration is refused by validateNoDuplicates before any of this runs,
// which is what makes "after the fallbacks" true in every sentence below.
//
// FROM is the FIRST RUNG of the ladder being added, and it is what the fluid
// sentences carry in place of the numbers they cannot print: without it a
// three-way collapse writes the same line twice and names no declaration the
// author could go and change.
func mergeIngredient(res *resolution, tgt noteTarget, list []resolvedIngredient, subject, from string, add resolvedIngredient) []resolvedIngredient {
	for i := range list {
		if list[i].kind != add.kind || list[i].name != add.name {
			continue
		}
		if add.kind == kindFluid {
			res.logs = append(res.logs, mergedFluidLine(subject, add.name, from))
			sum := list[i].fluid + add.fluid
			// THE CEILING IS RE-ASKED HERE AND NOWHERE ELSE. Both amounts
			// crossed validateIngredients on their own and both were legal;
			// their sum is a number no author wrote, and above the engine's
			// wall it does not refuse, it ABORTS (see maxFluidAmount).
			//
			// AND IT CLAMPS RATHER THAN REFUSING. Which rungs the ladders
			// landed on is a fact about the mod set, so this is an
			// ENVIRONMENTAL check: a modpack that removed two first rungs must
			// not stop the load on a number arithmetic can pin at the ceiling
			// the engine itself takes. The amount is not the author's any more
			// either way, so the line says what it became and the recipe's own
			// tooltip carries a note.
			if sum > maxFluidAmount {
				res.logs = append(res.logs, mergedFluidClamp(subject, add.name, from))
				res.noteOn(tgt, withDestruction(clampedFluidNote(add.name), true))
				sum = maxFluidAmount
			}
			list[i].fluid = sum
			return list
		}
		res.logs = append(res.logs, mergedItemLine(subject, add.name, list[i].amount, add.amount))
		sum := addAmounts(list[i].amount, add.amount)
		if sum > maxItemAmount {
			res.logs = append(res.logs, mergedItemClamp(subject, add.name, list[i].amount, add.amount))
			res.noteOn(tgt, withDestruction(clampedItemNote(add.name), true))
			sum = maxItemAmount
		}
		list[i].amount = sum
		return list
	}
	return append(list, add)
}

// mergePack is mergeIngredient for a science pack, whose subject is the
// technology and whose kind is never in question: the ladder asks ToolExists,
// a tool is an item, so a pack list has one namespace and the names alone
// decide identity.
func mergePack(res *resolution, tgt noteTarget, list ingredientList, subject string, add listEntry) ingredientList {
	for i := range list {
		if list[i].name != add.name {
			continue
		}
		res.logs = append(res.logs, mergedItemLine(subject, add.name, list[i].amount, add.amount))
		sum := addAmounts(list[i].amount, add.amount)
		// THE ITEM CEILING, ON A NUMBER NO AUTHOR DECLARED, and it is the
		// engine's own rule for a unit ingredient too. MEASURED on 2.0.77:
		// a technology whose unit ingredients carry 65535 loads and dumps,
		// and 65536, 2^31 and 2^53 each refuse with "The data type allows
		// values from 0 to 65535" at
		// ROOT.technology.<name>.unit.ingredients[0][1]. validateUnit holds a
		// DECLARED pack to the same number, so this is the only pack amount
		// left that no author wrote, and it is CLAMPED rather than refused for
		// the reason mergeIngredient's twin is. A research costs no assembling
		// machine anything, so the note carries no destruction sentence.
		if sum > maxItemAmount {
			res.logs = append(res.logs, mergedItemClamp(subject, add.name, list[i].amount, add.amount))
			res.noteOn(tgt, clampedItemNote(add.name))
			sum = maxItemAmount
		}
		list[i].amount = sum
		return list
	}
	return append(list, add)
}

// addAmounts is the merge's only arithmetic, and it SATURATES rather than
// wrapping.
//
// THE REASON WAS NEVER THIS HALF. Nothing here can wrap into a wrong answer:
// every declared amount is validated at 1 or more, the running sum only ever
// grows, and res.refuse keeps the FIRST sentence it was handed, so the first
// sum over the ceiling is what the plan is refused with and no later
// arithmetic can take that back. MEASURED: replacing this body with `a + b`
// changes no output at all, in any test in this suite.
//
// WHAT IT WAS FOR IS THE RUST MIRROR, where resolution keeps merging AFTER a
// refusal is recorded and an unbounded sum is a DEBUG PANIC rather than a
// wrap. MEASURED there, with this round's declared-pack ceiling taken back
// out: 1025 rungs of 2^53 landing on one pack panic with "attempt to add with
// overflow".
//
// AND THAT CASE IS NOW UNREACHABLE, which is why this stays rather than
// growing a test. Every amount that reaches a merge is at most 65535, held
// there by validateIngredients, by validateUnit and, for a packs setting's
// declared default, by the language's own round trip; overflowing an int64 at
// that size takes about 1.4e14 rungs in one list. The saturation is the guard
// that keeps the two halves computing the same number, and the one that keeps
// a Rust debug build standing if a declared amount is ever admitted above the
// ceiling again.
//
// SYMMETRIC, because the Rust mirror's saturating_add is: no declared amount
// is negative today, so the lower arm is unreachable, and an arithmetic
// helper whose two halves disagree about a case neither can reach is a
// difference waiting for the day one of them can.
func addAmounts(a, b int64) int64 {
	const maxInt64 = int64(^uint64(0) >> 1)
	const minInt64 = -maxInt64 - 1
	switch {
	case b > 0 && a > maxInt64-b:
		return maxInt64
	case b < 0 && a < minInt64-b:
		return minInt64
	}
	return a + b
}

// The four merge sentences, in one place because they are two sentences with
// two shapes each and a reader who sees the merge line and the clamp line
// beside it has to recognise the pair: both open with the same clause.
//
// AN ITEM PRINTS ITS NUMBERS AND A FLUID NAMES THE RUNG INSTEAD. Rendering a
// fluid amount is the ingredient language's job (formatListAmount), and the
// language is reached only through the two text-setting constructors so that a
// plan declaring no text setting links none of it; a call from here would link
// it into every consumer, which the source-property test refuses by name. So
// the fluid sentences carry the first rung of the ladder that landed on the
// name: it costs no formatter, it is what an author goes and edits, and it is
// what makes two lines of a three-way collapse different lines.
//
// AND DO NOT REACH FOR THE STANDARD LIBRARY HERE. MEASURED on this machine:
// Go's strconv.FormatFloat(5e300, 'g', -1, 64) is "5e+300", while Rust's
// format!("{}", 5e300f64) is a 5 with 300 zeros after it and format!("{:e}",
// ...) is "5e300". No two of those are the same string, so a fluid amount
// printed the convenient way in each half splits the mirror on a line that is
// compared byte for byte. formatListAmount exists because of exactly this, and
// it is the one thing this file may not call.
func mergedOpening(subject, name string) string {
	return "fkrecipes: " + subject + ": " + name + " is in the list twice after the fallbacks"
}

func mergedItemLine(subject, name string, a, b int64) string {
	return mergedOpening(subject, name) + ", so the amounts are added: " +
		strconv.FormatInt(a, 10) + " plus " + strconv.FormatInt(b, 10) + " is " +
		strconv.FormatInt(addAmounts(a, b), 10)
}

// See the note above: the rung is here in place of the two amounts, and
// printing them instead is what may not be done.
func mergedFluidLine(subject, name, from string) string {
	return mergedOpening(subject, name) + ", so the amounts are added" + mergedFrom(from)
}

func mergedItemClamp(subject, name string, a, b int64) string {
	return mergedOpening(subject, name) + ", and " + strconv.FormatInt(a, 10) + " plus " +
		strconv.FormatInt(b, 10) + " is above the item ceiling of " + strconv.FormatInt(maxItemAmount, 10) +
		", so it is capped there"
}

// The same, for the same reason: the ceiling is a constant this file may
// spell, and the amount that crossed it is not.
func mergedFluidClamp(subject, name, from string) string {
	return mergedOpening(subject, name) +
		", and the added amount is above the fluid ceiling of 1e301, so it is capped there" + mergedFrom(from)
}

// mergedFrom is the clause both fluid sentences end with, naming the ladder
// whose collapse produced the second occurrence.
func mergedFrom(from string) string {
	return "; the ladder from " + from + " resolved onto it"
}

// selfProductLine is what a recipe whose resolved list names its own product
// says, and it is a LOG LINE rather than a refusal.
//
// THE SHAPE IS LEGAL AND THE BASE GAME SHIPS IT. MEASURED on 2.0.77 (build
// 84539, mac-arm64, steam), base alone: kovarex-enrichment-process takes 40
// uranium-235 and 5 uranium-238 and gives back 41 uranium-235 and 2
// uranium-238, and coal-liquefaction takes 25 heavy-oil and gives back 90. A
// sweep of the same dump found exactly those two among base's 217 recipes, so a
// library that refused this would be refusing something the game itself does.
//
// IT IS NOT AN `fkrecipes: ERROR: ` LINE EITHER. That prefix belongs to a
// stored value the library set aside; nothing is set aside here, and the plan
// emits exactly what it resolved.
//
// WHAT IT IS FOR IS THE SIGNAL. A player's typed text and an author's declared
// ladder can both land on the product, because the text world overlays the
// plan's own items and a ladder's last rung is whatever the author wrote. The
// result loads and then does nothing anybody expects: an assembler fed the
// recipe consumes the product to make the product, and the first one has to
// come from somewhere else entirely.
func selfProductLine(subject, name string) string {
	return "fkrecipes: " + subject + ": " + name +
		" is in the list and is also what this recipe makes," +
		" so nothing can craft the first one unless something else produces it"
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
	// enabled is NOT in the recipe list, and is the single exception in this
	// file: validate takes it on its own, because whether the library owns it
	// depends on the plan rather than on the builder's arms, and the sentence
	// it is refused with names the technology that decided.
	// BOTH SPELLINGS OF THE CATEGORY, because the library owns the CONCEPT on
	// either engine and only one of the two names reaches a given one. On 2.1
	// what it writes is `categories` (respellRecipeCategory, world.go), so an
	// author's Extra carrying that name would be silently overwritten there
	// and would mean nothing here; naming both makes it the same refusal on
	// both engines instead of a refusal on one and a surprise on the other.
	recipeOwnFields = []string{
		"type", "name", "localised_name", "localised_description",
		"category", "categories", "energy_required", "ingredients", "results",
		"order",
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
