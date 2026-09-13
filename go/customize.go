package fkrecipes

import (
	"errors"
	"strconv"
	"strings"
)

// THE CUSTOMIZER: what binds the ingredient-list language to settings, recipes
// and research.
//
// ingredientlist.go is the language itself and knows nothing about a plan.
// This file is the other side of that seam: it renders a DECLARED list into
// the text a setting's description shows, decides which recipe or technology a
// text setting serves, reads the player's text back at the data stage, and
// composes the localised strings the settings screen renders.
//
// THE RESERVED WORD IS THE WHOLE DESIGN. A text setting's default_value is the
// word default, never the rendered list, so a player who never opens the
// settings screen keeps the author's list WITH ITS LADDERS across releases.
// The rendering is written into the setting's DESCRIPTION instead, which is
// where a player who wants to edit copies it from. See IngredientsSetting.

// maxLocalisedParams is the engine's ceiling on one localised string's
// parameters.
//
// MEASURED (Factorio 2.0.77, build 84539): a localised string with 21
// parameters refuses the load, and so does one nested 20 tables deep; 20
// parameters and 19 nested tables load, and two nested groups of 20 load. (The
// engine's refusal counts one higher than the tables, "21 > 20 (limit)" for
// 20 of them; FkLua's data-stage probe pinned both limits.) A dropdown with
// more presets than fit therefore NESTS rather than overflowing: each level carries at most
// this many parameters, and when there are more the last parameter is a nested
// localised string holding the rest by the same rule. That is what keeps the
// composed description a load the engine takes rather than a hard failure
// naming nothing useful.
const maxLocalisedParams = 20

// allExistsWorld answers yes to every presence question and nothing else.
//
// IT IS THE WORLD THE ROUND-TRIP CHECK RUNS AGAINST, and it has to be: the
// rendered default of a text setting is checked at PLAN time, where the
// settings stage has no data.raw at all (measured: data.raw is empty there),
// and a name in a declared ladder is a name the game may or may not have
// anyway. The question this check asks is whether the LANGUAGE reads back what
// the renderer wrote, not whether the game has the names, so every name
// exists and the parse is about syntax, kinds and duplicates.
//
// It embeds UnimplementedWorld, so a question the language grows later panics
// naming itself here rather than answering a silent yes.
type allExistsWorld struct{ UnimplementedWorld }

func (allExistsWorld) ItemExists(name string) bool  { return true }
func (allExistsWorld) FluidExists(name string) bool { return true }
func (allExistsWorld) ToolExists(name string) bool  { return true }

// ---------------------------------------------------------------------------
// Rendering a DECLARED list.
// ---------------------------------------------------------------------------

// declaredIngredientEntries turns a declared list into the entries the renderer
// takes: this plan's own item under its EMITTED name, and the FIRST RUNG of
// every presence ladder.
//
// THE FIRST RUNG, not the one the game answers with, and that is a decision
// rather than a shortcut: the description is written at the SETTINGS stage,
// where data.raw does not exist and no ladder can be walked, and the text is
// what the author declared rather than what this particular modpack resolved
// it to. The word default is what carries the ladder to the player, so the
// description showing a rung the modpack lacks costs nothing: the field itself
// never shows a name.
//
// THE PRECONDITION IS A VALIDATED LIST. validateIngredients has already refused
// an empty ladder and an item handle this plan never issued, so the two
// dereferences here cannot be reached with anything else.
func (l *Lib) declaredIngredientEntries(prefix string, ings []Ingredient) ingredientList {
	out := make(ingredientList, 0, len(ings))
	for _, ing := range ings {
		if ing.kind == kindFluid {
			out = append(out, listEntry{kind: kindFluid, name: l.declaredHead(prefix, ing), fluid: ing.fluidAmount})
			continue
		}
		out = append(out, listEntry{name: l.declaredHead(prefix, ing), amount: ing.amount})
	}
	return out
}

// declaredHead is the name a declared ingredient WILL RESOLVE TO if nothing is
// missing: this plan's own item under its emitted name, or a ladder's first
// rung.
//
// ONE ANSWER FOR TWO CALLERS, and that is the point of writing it out. The
// description shows these names, and validateNoDuplicates compares them; the
// two spelling the same rule apart from each other is how a list could be
// described as one thing and refused as another. Its precondition is the one
// above: a validated list, so both dereferences are safe.
func (l *Lib) declaredHead(prefix string, ing Ingredient) string {
	if len(ing.candidates) == 0 {
		return l.items[ing.item.index-1].emittedName(prefix)
	}
	return ing.candidates[0]
}

// declaredPackEntries is the same for a pack list. A science pack is always an
// item and never this plan's own: this library declares no tools.
func declaredPackEntries(packs []Pack) ingredientList {
	out := make(ingredientList, 0, len(packs))
	for _, p := range packs {
		out = append(out, listEntry{name: p.Name, amount: p.Amount})
	}
	return out
}

// ---------------------------------------------------------------------------
// Which recipe or technology reads which setting.
// ---------------------------------------------------------------------------

// textBinding is what a text setting is bound to: how many declarations read
// it, and the category of the recipe when one does.
//
// THE CATEGORY TRAVELS WITH THE BINDING because the fluid rule is about the
// RECIPE and not about the ingredient: the same declared fluid is legal in a
// chemistry recipe and a load failure in a crafting one, so the declared
// default of a text setting can only be checked against the recipe that reads
// it.
type textBinding struct {
	count    int
	category string
}

// textSettingBindings walks the plan once and records who reads each text
// setting. A handle this plan never issued is SKIPPED rather than followed,
// exactly as craftTimeBoundSettings skips one: validateBindings is what
// refuses it by name, and following it here would mark the wrong setting.
func (l *Lib) textSettingBindings() []textBinding {
	out := make([]textBinding, len(l.settings))
	mark := func(i int, category string) {
		out[i].count++
		out[i].category = category
	}
	for _, r := range l.recipes {
		if l.validIngredientsSetting(r.spec.IngredientsFrom) {
			mark(r.spec.IngredientsFrom.index-1, r.spec.Category)
		}
		if by := r.spec.IngredientsBy; by != nil && l.validIngredientsSetting(by.Custom) {
			mark(by.Custom.index-1, r.spec.Category)
		}
	}
	for _, t := range l.techs {
		if c := t.spec.CostFrom; c != nil && l.validPacksSetting(c.Packs) {
			mark(c.Packs.index-1, "")
		}
		if by := t.spec.CostBy; by != nil && by.Custom != nil && l.validPacksSetting(by.Custom.Packs) {
			mark(by.Custom.Packs.index-1, "")
		}
	}
	return out
}

// numberBinding is what a NUMERIC setting is bound to: how many declarations
// read it at all, and whether any of those reads is a custom cost's count or
// seconds.
//
// THE TWO HALVES ARE SEPARATE BECAUSE THE TWO READS ARE. A crafting time two
// recipes share is ordinary and always was; a cost arm's number is the one
// this library says out loud is ignored, and that sentence is only true of a
// setting nothing else reads.
type numberBinding struct {
	count  int
	asCost bool
}

// numberSettingBindings walks the plan once and records who reads each numeric
// setting: a recipe's crafting time, and a custom cost's count and seconds
// under CostFrom and under a Custom arm alike.
//
// A handle this plan never issued is SKIPPED rather than followed, exactly as
// textSettingBindings and craftTimeBoundSettings skip one: the validators and
// the data planner are what refuse it by name, and following it here would
// mark the wrong setting.
//
// AND SO IS A DECLARATION THAT HAS NOT SAID WHAT IT COSTS, which is the
// validator's own step past carried into this walk: a recipe naming CraftTime
// beside CraftTimeFrom, and a technology whose cost sources are not exactly
// one, are answered by "pick one" and "exactly one", and a reader counted out
// of such a declaration would put this rule's sentence in front of the one its
// author reads best.
func (l *Lib) numberSettingBindings() []numberBinding {
	out := make([]numberBinding, len(l.settings))
	markCost := func(c *CustomCost) {
		if l.validIntSetting(c.Count) {
			out[c.Count.index-1].count++
			out[c.Count.index-1].asCost = true
		}
		if l.validDoubleSetting(c.Seconds) {
			out[c.Seconds.index-1].count++
			out[c.Seconds.index-1].asCost = true
		}
	}
	for _, r := range l.recipes {
		// A recipe holding a crafting time AND a handle to one is stepped past
		// whole: it has not said what it costs to make, the data planner says
		// so, and this walk has nothing to add in front of that.
		if r.spec.CraftTime != 0 && r.spec.CraftTimeFrom.index != 0 {
			continue
		}
		if l.validDoubleSetting(r.spec.CraftTimeFrom) {
			out[r.spec.CraftTimeFrom.index-1].count++
		}
	}
	for _, t := range l.techs {
		// The same step past against the same predicate the validator's own
		// technology walk uses, so the two agree by construction.
		if namedCostSources(&t.spec) != 1 {
			continue
		}
		if c := t.spec.CostFrom; c != nil {
			markCost(c)
		}
		if by := t.spec.CostBy; by != nil && by.Custom != nil {
			markCost(by.Custom)
		}
	}
	return out
}

// namedCostSources counts the cost sources a technology declares. Exactly one
// is the rule and the data planner is where it is refused, so every walk that
// steps past a technology naming some other number asks this one question.
func namedCostSources(spec *TechSpec) int {
	named := 0
	for _, set := range []bool{spec.CostOf != "", spec.Unit != nil, spec.CostBy != nil, spec.CostFrom != nil} {
		if set {
			named++
		}
	}
	return named
}

// ---------------------------------------------------------------------------
// Plan-time validation.
// ---------------------------------------------------------------------------

// validateBindings is every rule about how a text setting is BOUND, and both
// planners run it: the settings stage composes a dropdown's description out of
// a Custom arm, so it needs the arm to be well formed just as much as the data
// stage does.
//
// IT REFUSES NOTHING THE OTHER TWO VALIDATORS ALREADY OWN. Where a recipe or a
// technology names two costs, or two ingredient sources this file did not add,
// the walk SKIPS that declaration and leaves the sentence to the data
// planner's own loop: a plan with two problems should be answered by the one
// its author is likelier to recognise, and "pick one" is that sentence.
func (l *Lib) validateBindings(prefix string) error {
	at := "fkrecipes: "

	// ONE DROPDOWN COMPOSES ONE DESCRIPTION, so it takes a Custom arm from one
	// declaration. Counted in the two walks below, where an arm is proved, and
	// refused after both of them so the sentence names the first such setting
	// in declaration order rather than whichever walk noticed first.
	armedByRecipe := make([]int, len(l.settings))
	armedByTech := make([]int, len(l.settings))

	for _, r := range l.recipes {
		who := "the recipe " + r.name
		if r.spec.IngredientsFrom.index != 0 {
			if len(r.spec.Ingredients) > 0 {
				return errors.New(at + who + " names both Ingredients and IngredientsFrom; pick one")
			}
			if r.spec.IngredientsBy != nil {
				return errors.New(at + who + " names both IngredientsBy and IngredientsFrom; pick one")
			}
			if !l.validIngredientsSetting(r.spec.IngredientsFrom) {
				return errors.New(at + who + " reads its ingredients from a setting that this plan never declared")
			}
		}
		by := r.spec.IngredientsBy
		// The dropdown's own exclusivity with Ingredients is the data planner's
		// sentence; nothing here may fire in front of it.
		if by == nil || len(r.spec.Ingredients) > 0 {
			continue
		}
		if !l.validDropdownSetting(by.Setting) {
			return errors.New(at + who + " names an ingredients setting that this plan never declared")
		}
		setting := l.settings[by.Setting.index-1]
		full := setting.emittedName(prefix)
		if by.Custom.index == 0 {
			// A value named custom with NOTHING BEHIND IT is the pilot's own
			// defect: the player picks it and gets a recipe made of nothing,
			// with no line in the log saying why.
			//
			// A CHOICE THAT COVERS IT IS WHAT MAKES IT ORDINARY. A mod whose
			// dropdown already ships a preset named custom keeps it as a
			// preset: this refusal is about a value with no plan behind it,
			// not about the word, and a covered value has a plan like every
			// other one.
			if v := by.customValue(); countValue(setting.values, v) > 0 && !coversIngredientValue(by.Choices, v) {
				return errors.New(at + "the setting " + full + " offers " + v +
					", and " + who + " names no Custom arm for it")
			}
			continue
		}
		armedByRecipe[by.Setting.index-1]++
		if !l.validIngredientsSetting(by.Custom) {
			return errors.New(at + who + " names a Custom ingredients setting that this plan never declared")
		}
		if err := customArmValues(at, who, full, by.customValue(),
			coversIngredientValue(by.Choices, by.customValue()), setting.values); err != nil {
			return err
		}
		// The presets are RENDERED into the dropdown's description at the
		// settings stage, so they have to be renderable there. The data
		// planner checks the same thing in its own loop with the same
		// sentence; this is what brings the check forward to the stage that
		// needs it.
		for _, c := range by.Choices {
			if err := l.validateIngredients(at, who, r.spec.Category, c.Ingredients); err != nil {
				return err
			}
			if err := l.validateNoDuplicates(at, who, prefix, c.Ingredients); err != nil {
				return err
			}
		}
	}

	for _, t := range l.techs {
		who := "the technology " + t.name
		// Exactly one cost source is the data planner's sentence, and it is the
		// one an author reads best; everything below assumes it held.
		if namedCostSources(&t.spec) != 1 {
			continue
		}
		if c := t.spec.CostFrom; c != nil {
			if len(c.Position) > 0 {
				return errors.New(at + who + " names CostFrom with a Position; Position belongs to a Custom arm, and CostFrom is placed by After, Before and AfterTech")
			}
			if err := l.validateCustomCost(at, who, c); err != nil {
				return err
			}
			continue
		}
		by := t.spec.CostBy
		if by == nil {
			continue
		}
		if !l.validDropdownSetting(by.Setting) {
			return errors.New(at + who + " names a cost setting that this plan never declared")
		}
		setting := l.settings[by.Setting.index-1]
		full := setting.emittedName(prefix)
		if by.Custom == nil {
			// The recipe twin's rule, word for word: a value with no plan
			// behind it is the defect, and one a Choice covers is a preset.
			if v := by.customValue(); countValue(setting.values, v) > 0 && !coversCostValue(by.Choices, v) {
				return errors.New(at + "the setting " + full + " offers " + v +
					", and " + who + " names no Custom arm for it")
			}
			continue
		}
		armedByTech[by.Setting.index-1]++
		if err := customArmValues(at, who, full, by.customValue(),
			coversCostValue(by.Choices, by.customValue()), setting.values); err != nil {
			return err
		}
		// THE PREREQUISITE MOVES WITH THE UNIT everywhere else in CostBy: the
		// chosen tier's source technology becomes the sole prerequisite. A
		// custom arm has no source, so it carries its own ladder, and an arm
		// with none would place the technology nowhere at all.
		if len(by.Custom.Position) == 0 {
			return errors.New(at + who + " names a Custom cost arm with no Position; the arm places the technology, so it needs a prerequisite ladder")
		}
		if err := l.validateCustomCost(at, who, by.Custom); err != nil {
			return err
		}
	}

	// The composed description is ONE declaration's presets, so two of them
	// reaching one dropdown is a settings screen showing a list that belongs to
	// the other declaration. The two counts are separate because the sentence
	// names what the author wrote; a dropdown armed by one recipe AND one
	// technology is not refused here, and the technology's description is the
	// one that lands, because settingDescriptions walks recipes first.
	for i, s := range l.settings {
		full := s.emittedName(prefix)
		if armedByRecipe[i] > 1 {
			return errors.New(at + "the setting " + full +
				" takes a Custom arm from more than one recipe; one dropdown composes one description")
		}
		if armedByTech[i] > 1 {
			return errors.New(at + "the setting " + full +
				" takes a Custom arm from more than one technology; one dropdown composes one description")
		}
	}

	bindings := l.textSettingBindings()
	for i, s := range l.settings {
		if !s.kind.isText() {
			continue
		}
		switch {
		case bindings[i].count == 0:
			// A FIELD THE PLAYER CAN EDIT THAT CHANGES NOTHING is worse than a
			// missing feature: it is a promise in the settings screen that the
			// mod does not keep.
			return errors.New(at + "the setting " + s.name + " is declared and nothing reads it; a text setting must be bound to one recipe or technology")
		case bindings[i].count > 1:
			return errors.New(at + "the setting " + s.name + " is read by more than one recipe or technology; a text setting serves exactly one")
		}
	}

	// THE SAME RULE FOR A COST ARM'S TWO NUMBERS, and it exists because the
	// line beside them would otherwise lie. A dropdown on a preset logs that
	// the count or the seconds is ignored, and that is only true if nothing
	// else in the plan reads that setting: one double can back a recipe's
	// CraftTimeFrom and a technology's Seconds at once, and the player reads
	// "so the number is ignored" two lines above a recipe using the very
	// number they moved.
	//
	// A CRAFTING TIME TWO RECIPES SHARE IS NOT THIS DEFECT and is not refused
	// here: nothing ignores a crafting time, so no line about one can lie, and
	// the sharing predates this rule.
	//
	// NO KIND CHECK STANDS IN FRONT OF IT: only a Count or a Seconds handle
	// sets asCost, every issuer of those two handle types declares an int or a
	// double setting beside it, and a handle from another plan is refused by
	// lib id, so a setting this refusal can reach is numeric by construction.
	numbers := l.numberSettingBindings()
	for i, s := range l.settings {
		if numbers[i].asCost && numbers[i].count > 1 {
			return errors.New(at + "the setting " + s.name +
				" is read as a research count or time by more than one declaration; a custom cost's number serves exactly one")
		}
	}
	return nil
}

// customArmValues is the shape rule a Custom arm's dropdown has to satisfy,
// written once because the recipe arm and the cost arm have the same one.
func customArmValues(at, who, setting, value string, covered bool, values []string) error {
	if covered {
		return errors.New(at + who + " gives " + value + " a preset as well as a Custom arm; name the arm's value with CustomValue")
	}
	switch countValue(values, value) {
	case 1:
		return nil
	case 0:
		return errors.New(at + who + " names a Custom arm for " + value + ", which the setting " + setting + " does not offer")
	default:
		return errors.New(at + "the setting " + setting + " offers " + value + " more than once, and a Custom arm needs it exactly once")
	}
}

// validateCustomCost checks the three handles and the two bounds the engine's
// own reset rule turns into a guarantee.
//
// THE BOUNDS ARE ON THE DECLARED SPEC, not on the value read. MEASURED: a
// numeric setting whose stored value falls outside its own bounds is RESET to
// the default rather than clamped, and a setting whose DEFAULT lies outside its
// own bounds refuses the load. So a minimum of at least 1 on the count and
// above 0 on the seconds makes every value the data stage can ever read a legal
// one, and the data stage needs no arm for an illegal one at all.
func (l *Lib) validateCustomCost(at, who string, c *CustomCost) error {
	if !l.validPacksSetting(c.Packs) {
		return errors.New(at + who + " reads its science packs from a setting that this plan never declared")
	}
	if !l.validIntSetting(c.Count) {
		return errors.New(at + who + " reads its research count from a setting that this plan never declared")
	}
	if !l.validDoubleSetting(c.Seconds) {
		return errors.New(at + who + " reads its research time from a setting that this plan never declared")
	}
	count := l.settings[c.Count.index-1]
	if !count.spec.HasMin || count.spec.Min < 1 {
		return errors.New(at + "the setting " + count.name +
			" backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)")
	}
	seconds := l.settings[c.Seconds.index-1]
	if !seconds.spec.HasMin || !(seconds.spec.Min > 0) {
		return errors.New(at + "the setting " + seconds.name +
			" backs a research time but declares no minimum above 0 (the engine refuses a unit time of 0)")
	}
	return nil
}

// validateTextSettings is every rule about what a text setting DECLARES, and
// both planners run it: the settings stage renders the declared list into the
// description, and the data stage emits it whenever the text says default.
//
// THE ROUND TRIP IS THE POINT. The description tells the player "this is what
// default means", and a player who copies it and changes one number must get a
// text this library reads. A declared list that renders into something the
// language refuses is a field nobody can edit, so it is refused here rather
// than shipped.
//
// IT IS ALSO WHERE THE LANGUAGE SEAM IS GUARDED, first thing and before every
// rule below, because both planners run this in front of their loops and
// nothing past it may dereference a language that is not there. See Lib.lang:
// the parser, the renderer, the amount formatter and the custom-cost resolver
// are reached as function values so a plan that declares no text setting never
// names them, and a declaration that arrived without them is a plan whose
// every text path would call nothing.
func (l *Lib) validateTextSettings(prefix string) error {
	at := "fkrecipes: "
	bindings := l.textSettingBindings()
	for i, s := range l.settings {
		if !s.kind.isText() {
			continue
		}
		// UNREACHABLE THROUGH THE PUBLIC SURFACE, and it has a witness anyway:
		// IngredientsSetting and PacksSetting are the only way a consumer
		// declares a text setting and both install what they need, so this
		// fires only for a declaration appended inside the package. A guard
		// with no witness is a guard nobody has seen work.
		if l.lang == nil || (s.kind == settingPacks && l.customCost == nil) {
			return errors.New(at + "the text setting " + s.name +
				" was declared without the ingredient language; declare it through IngredientsSetting or PacksSetting")
		}
		who := "the ingredients setting " + s.name
		if s.kind == settingPacks {
			who = "the packs setting " + s.name
		}
		var entries ingredientList
		if s.kind == settingIngredients {
			// The ordinary declared-ingredient rules, naming the SETTING: the
			// finiteness, zero and ceiling checks have to clear before the
			// renderer sees an amount, because a non-finite one renders as text
			// that does not read back.
			if err := l.validateIngredients(at, who, bindings[i].category, s.defIngredients); err != nil {
				return err
			}
			entries = l.declaredIngredientEntries(prefix, s.defIngredients)
		} else {
			// A DECLARED PACK LIST THAT IS EMPTY IS REFUSED, exactly as a
			// hand-rolled unit's is and for the same reason: the engine LOADS
			// such a cost (measured) and the player gets a research that
			// completes instantly.
			if len(s.defPacks) == 0 {
				return errors.New(at + who + " declares no science pack; research takes at least one")
			}
			if err := validateDeclaredPacks(at, who, s.defPacks); err != nil {
				return err
			}
			entries = declaredPackEntries(s.defPacks)
		}
		// The setting name in these messages is the DECLARED one wrapped in
		// what kind of setting it is, because this refusal is the author's to
		// fix and not the player's: they read "the ingredients setting
		// rivet-ingredients, entry 2 (...)" rather than a bare prefixed name.
		//
		// THE PARSE IS THE WHOLE CHECK. A rendering the language reads back as
		// something else was once refused here by comparing the two texts, and
		// that arm was unreachable: the renderer writes the canonical form of
		// every entry, so a text this parse accepts renders back to itself, and
		// a text it does not accept is answered by the sentence above.
		if _, problem := l.lang.parse(l.lang.render(entries), listKindOf(s.kind),
			bindings[i].category, who, allExistsWorld{}); problem != "" {
			return errors.New(problem)
		}
	}
	return nil
}

// listKindOf is which of the language's two lists a text setting holds,
// written once because the declaration check, the data path and the
// ignored-text note all have to answer it the same way.
func listKindOf(k settingKind) listKind {
	if k == settingPacks {
		return listPacks
	}
	return listRecipe
}

// validateDeclaredPacks is the pack-list check for a SETTING's declared
// default. UnitSpec's own loop stays where it is: its sentences name the
// technology that priced itself, and these name the setting that declared the
// list.
func validateDeclaredPacks(at, who string, packs []Pack) error {
	for _, p := range packs {
		if p.Amount < 1 {
			return errors.New(at + who + " has a science pack amount below 1, which the engine refuses")
		}
		if p.Amount > maxExactInt {
			return errors.New(at + who + " declares a science pack amount a Lua double cannot hold exactly: " + strconv.FormatInt(p.Amount, 10))
		}
		if p.Name == "" {
			return errors.New(at + who + " names a science pack with an empty name")
		}
		// A rung with no name is a ladder that can never answer, and it would
		// otherwise reach a log line reading "none of a, , b is present".
		for _, c := range p.Fallbacks {
			if c == "" {
				return errors.New(at + who + " names a science pack with an empty name")
			}
		}
	}
	return nil
}

// ---------------------------------------------------------------------------
// The composed localised strings.
// ---------------------------------------------------------------------------

// settingDescriptions is the localised_description each setting is emitted
// with, or Nil for the ones that carry none. Two settings get one:
//
//   - a TEXT setting, whose description is the consumer's own key followed by
//     the declared list written out, so the player can see what the word
//     default stands for and copy it;
//   - a DROPDOWN WITH A CUSTOM ARM, whose description is the consumer's own key
//     followed by one line per preset, so a player switching to custom can
//     start from the preset they were on.
//
// THE SECOND ONE EXISTS BECAUSE THE ENGINE FORBIDS THE ALTERNATIVE. Filling the
// text from the player's old dropdown choice is impossible: the settings stage
// sees no stored value (measured: data.raw is empty there), nothing at a data
// stage can write a setting, and settings.startup is read only at the control
// stage. Composing the description is what the library can do instead.
func (l *Lib) settingDescriptions(prefix string) []Value {
	out := make([]Value, len(l.settings))
	for i := range out {
		out[i] = Nil()
	}
	for i, s := range l.settings {
		if !s.kind.isText() {
			continue
		}
		var entries ingredientList
		if s.kind == settingIngredients {
			entries = l.declaredIngredientEntries(prefix, s.defIngredients)
		} else {
			entries = declaredPackEntries(s.defPacks)
		}
		// THE LANGUAGE IS THERE BECAUSE THIS SETTING IS. Both planners run
		// validateTextSettings in front of this walk, and it refuses a text
		// setting whose language is missing before anything renders.
		out[i] = textDescription(s.emittedName(prefix), l.lang.render(entries))
	}
	// Recipes then technologies, in declaration order, which is the order a
	// dropdown's own description is built in when two declarations arm one
	// dropdown. validateBindings refuses two recipes and two technologies over
	// one dropdown, each with its own sentence; what it does NOT refuse is one
	// recipe and one technology arming the same one, and there the technology's
	// preset list is the one that lands, because this walk runs second. That is
	// a plan nobody has written and a sentence nobody has agreed on, so it is
	// written down here rather than answered on a guess.
	for _, r := range l.recipes {
		by := r.spec.IngredientsBy
		// EXACTLY THE CONDITION validateBindings CHECKED UNDER, len(Ingredients)
		// and all: a recipe it stepped past is one whose choices it never
		// validated, and rendering an unvalidated choice would dereference an
		// item handle nothing proved. The data planner refuses that plan by
		// name; this one just says nothing about it.
		if by == nil || by.Custom.index == 0 || len(r.spec.Ingredients) > 0 || !l.validDropdownSetting(by.Setting) {
			continue
		}
		i := by.Setting.index - 1
		full := l.settings[i].emittedName(prefix)
		params := make([]Value, 0, len(by.Choices)+1)
		params = append(params, localeRef("mod-setting-description", full))
		// THE ARM IS WHAT GUARANTEES THE LANGUAGE. A Custom arm holds an
		// IngredientsSettingRef, which only IngredientsSetting issues, and
		// that constructor installs the renderer. A dropdown with no arm was
		// stepped past above and renders nothing.
		for _, c := range by.Choices {
			params = append(params, presetLine(full, c.Value,
				Str(ingredientPresetHead+l.lang.render(l.declaredIngredientEntries(prefix, c.Ingredients)))))
		}
		out[i] = localisedGroup(params)
	}
	for _, t := range l.techs {
		by := t.spec.CostBy
		if by == nil || by.Custom == nil || !l.validDropdownSetting(by.Setting) {
			continue
		}
		i := by.Setting.index - 1
		full := l.settings[i].emittedName(prefix)
		params := make([]Value, 0, len(by.Choices)+1)
		params = append(params, localeRef("mod-setting-description", full))
		for _, c := range by.Choices {
			params = append(params, presetLine(full, c.Value, costPresetTail(c)...))
		}
		out[i] = localisedGroup(params)
	}
	return out
}

// localeRef is a localised string that is nothing but a key: {"section.key"}.
func localeRef(section, key string) Value { return Arr(Str(section + "." + key)) }

// textDescription is the whole localised_description a TEXT setting is emitted
// with: the consumer's own entry, then the three things this library owes the
// player about the field beside it.
//
// FOUR PARAMETERS, and none of them a table beyond the consumer's key, so the
// twenty-parameter ceiling maxLocalisedParams records is nowhere near reached
// and the shape needs no nesting rule of its own.
//
// THE THREE LINES ARE THE ANSWER TO WHAT A CLIENT MEASUREMENT FOUND. A player
// standing in the Mod Settings screen reads the tooltip whole (measured on
// 2.0.77 on a DROPDOWN's composed description, one line per preset: seven
// lines rendered readable and unclipped; the ceilings on a composed
// description are the parameter count maxLocalisedParams holds and the nesting
// depth its comment records, and neither of them is a line count), so the
// description is where the library can say what the field takes; the closed
// dropdown's LABEL beside it is truncated at about 37 characters, which is why
// nothing a player needs may live in a label. The default line shows the list
// the word default stands for, in the internal names the field actually takes;
// the format line says so in words and states the ceiling; and the fallback
// line says what a text this library cannot use costs, which before it was
// stated nowhere a player looks.
//
// ONE COMPOSITION, TWO READERS. The settings planner emits this; CheckLocale
// asks the same function for the same shape with the list left out, so a line
// deleted here is a finding rather than a silent loss. See checkTextDescription.
func textDescription(full, rendered string) Value {
	return Arr(
		Str(""),
		localeRef("mod-setting-description", full),
		Str("\ndefault: "+rendered),
		Str(textFormatLine()),
		Str(textFallbackLine),
	)
}

// textFormatLine is the sentence that says what to type and how much of it.
//
// THE NUMBER IS maxListChars AND NOT A DIGIT TYPED HERE. The limit the player
// is told and the limit the parser enforces are one number by construction, so
// a change to the ceiling cannot ship a description that lies about it. That
// is held by a source property rather than by a value test, because a typed
// 2000 and this expression render the same bytes while the ceiling happens to
// be 2000: TestTheCeilingIsNeverTypedIntoAMessage forbids the ceiling's own
// decimal rendering in every string literal this package builds a message out
// of. It is a constant, so naming it here links no part of the language: a
// plan with no text setting still links no parser and no renderer.
//
// IT NAMES THE DEFAULT LINE ABOVE IT rather than describing internal names in
// the abstract, because that line is the copyable example, and copying it is
// exactly what the composition is for.
func textFormatLine() string {
	return "\nWrite internal names, as the default line above does, in at most " +
		strconv.Itoa(maxListChars) + " characters."
}

// textFallbackLine is what happens to a text this library cannot use.
//
// IT IS THE ONE THING THE SCREEN CANNOT SHOW. The text is set aside and the
// declared list applies (decision 2), so a player whose text went unused sees a
// settings screen that still holds it and a game that ignores it. Saying so in
// the description is the only warning available before the fact.
//
// IT PROMISES THE NARROW CLAIM AND NOT A LOAD, which is what the wording is
// for. Setting a text aside is not the same as loading: the declared list is
// held to every rule it always was, so a modpack in which that declaration
// cannot produce a legal result still stops the load, and on a refused load the
// log ops never reach the host at all (that is why withFallbackNote in data.go
// exists). Naming both places the reason can be, the log or the load error, is
// therefore the whole claim this line is allowed to make.
const textFallbackLine = "\nA text this mod cannot use is set aside and that default applies instead; the reason is in the log, or in the load error if the load stops anyway."

// presetLine is one preset's line in a composed dropdown description.
//
// THE VALUE IS LABELLED BY ITS OWN LOCALE ENTRY, not by its raw key, because
// the settings screen shows the player the localised label and a description
// naming the key would not match anything they can see. That entry is the one
// the dropdown already needs for the value to be readable at all, so the
// composition adds no locale obligation of its own.
//
// THE TAIL IS VALUES RATHER THAN A STRING because a cost line ends in a
// localised name and an ingredient line ends in a rendering this library wrote.
// Whatever the tail holds, the whole line is ONE parameter of the group above
// it, so localisedGroup's nesting rule counts it as one; and the tables the
// tail brings are the line's own elements, so they sit BESIDE the label, at
// the same level, not below it. A one-preset cost description is three table
// levels deep whichever tail it carries (measured on both shapes), which is
// what TestPlanSettingsComposesACostDropdownDescription's golden shows.
func presetLine(setting, value string, tail ...Value) Value {
	items := make([]Value, 0, 3+len(tail))
	items = append(items, Str(""), Str("\n"), localeRef("string-mod-setting", setting+"-"+value))
	return Arr(append(items, tail...)...)
}

// ingredientPresetHead opens the second line of an INGREDIENT preset, and it is
// a line of its own rather than a separator.
//
// TWO VOCABULARIES ARE NOT ONE RUN OF TEXT. A preset's label is the consumer's
// display prose ("Default: 4 iron plates, 2 gears") and the rendering after it
// is the internal names the field beside it takes ("4 iron-plate, ..."). Joined
// by ": " they read as one sentence in two languages, and only the second half
// can be copied into the field: measured on 2.0.77, the first half pasted into
// the text setting is refused and nothing in the tooltip said which half was
// which. The break and the word type put the copyable half on its own line,
// under the word a player acts on, so the two vocabularies are two lines.
//
// A COST PRESET KEEPS ITS ": cost of " (see costPresetTail) because it has only
// one vocabulary: a localised label followed by a localised technology name is
// prose throughout, and there is nothing in it to copy.
const ingredientPresetHead = "\n  type: "

// costPresetTail is what a research preset says it costs: the ladder's FIRST
// source, which is the technology whose unit would be copied. A ladder with no
// rungs at all falls back, and says so.
//
// THE TECHNOLOGY IS NAMED BY ITS LOCALISED NAME, not by its internal one. The
// line the player reads sits beside a tech tree that shows them "Military 4",
// and a description saying military-4 names something they cannot see, exactly
// as the raw value key would.
//
// AND IT ADDS NO LOCALE OBLIGATION OF THIS MOD'S: technology-name.<name> is the
// GAME's entry, for a technology some other mod or the base game declared, so
// the locale checker has nothing new to require and requires nothing new.
//
// WHERE THE TECHNOLOGY OR ITS ENTRY IS MISSING, THIS LINE READS WORSE THAN THE
// ONE IT REPLACED, and this file is not the place to pretend otherwise. An
// absent key is not rendered as nothing: locale.go's note records
// `Unknown key: "entity-name.bbb-linked-belt"` out of a live 2026 session, so
// a source no installed mod declares puts `Unknown key:
// "technology-name.<source>"` in the tooltip where the bare internal name used
// to stand. That is the trade, and both halves of it are the consumer's: the
// localised name wherever the technology exists, the Unknown key marker where
// it does not, which is their ladder to order (a first rung the game may lack
// is what shows the marker) and their locale to supply.
func costPresetTail(c CostChoice) []Value {
	if len(c.Sources) == 0 {
		return []Value{Str(": the fallback cost")}
	}
	return []Value{Str(": cost of "), localeRef("technology-name", c.Sources[0])}
}

// localisedGroup wraps parameters in a concatenating localised string, nesting
// when there are more than the engine takes. See maxLocalisedParams: each level
// holds at most twenty parameters, and a level that would need more keeps the
// first nineteen and hands the rest to a nested group in the twentieth slot.
func localisedGroup(params []Value) Value {
	items := make([]Value, 0, maxLocalisedParams+1)
	items = append(items, Str(""))
	if len(params) <= maxLocalisedParams {
		return Arr(append(items, params...)...)
	}
	items = append(items, params[:maxLocalisedParams-1]...)
	return Arr(append(items, localisedGroup(params[maxLocalisedParams-1:]))...)
}

// ---------------------------------------------------------------------------
// The data stage.
// ---------------------------------------------------------------------------

// messagePrefix is the constant every sentence this library composes begins
// with, in the language and in this layer alike.
//
// IT IS A CONSTANT SO IT CAN BE TAKEN BACK OFF. A fallback line carries a
// refusal's own sentence inside a line that already opens with the prefix, and
// it is removed with an explicit trim of this constant rather than by counting
// characters or by cutting at a colon. The corpus asserts the property over
// every message the language builds and TestFallbackSentencesCarryThePrefix
// over every one this layer builds, so the trim is total rather than hopeful.
const messagePrefix = "fkrecipes: "

// playerFallback is the ONE line a value the PLAYER controls logs when the
// library cannot use it, and it exists instead of a refusal.
//
// A VALUE THE PLAYER TYPES NEVER INTRODUCES A REFUSAL A PLAYER WHO TYPED
// NOTHING WOULD NOT ALSO HAVE HIT; AN INPUT THE AUTHOR DECLARES STILL REFUSES.
// The claim is that narrow one on purpose: what a fallback lands on is the
// author's declaration, and a modpack where that declaration cannot produce a
// legal result stops the load either way. See resolution.withFallbackNote for
// the sentence such a refusal then carries.
//
// MEASURED (Factorio 2.0.77, build 84539): the engine
// rewrites mod-settings.dat on every successful load and on NO failed one
// (three consecutive failed runs left the file at one sha256), so nothing in a
// failed run can edit the value that caused it. In the client the refusal is
// an "Error loading mods" dialog offering Disable listed mods, Disable all
// mods, Manage mods, Restart, Exit and a Reset mod settings checkbox; Manage
// mods shows the Mods screen, which offers enable and disable, has no Mod
// settings button, and whose Back returns to the same dialog rather than to
// the main menu, so the Mod Settings screen is not reachable. Disabling and
// re-enabling the mod does not help either: the engine keeps a disabled mod's
// settings, and a disabled mod's settings are not shown on the Mod Settings
// screen, so the identical refusal returns. The one escape measured is Reset
// mod settings plus Disable listed mods: six steps, every startup preference
// in the file lost, the mod disabled and a restart needed. A refusal on a
// field the player types into was therefore a lock-out, and the library owns
// it.
//
// ERROR IS UPPERCASE DELIBERATELY. Factorio's log() has one channel and no
// severity (verified: fk_log maps to the global log, and there is no second
// channel and no level parameter), so the severity has to be in the text.
// Uppercase also keeps the line out of a case-sensitive grep for the engine's
// own Error lines while a case-insensitive one still finds it.
//
// FIELD is the word the player looks for on the settings screen: the text of a
// list, or the number of a slider.
func playerFallback(reason, field string) string {
	return messagePrefix + "ERROR: " + strings.TrimPrefix(reason, messagePrefix) +
		". The mod loaded with its own default instead; fix the " + field +
		" under Settings > Mod settings > Startup, then restart."
}

// textFallback and numberFallback are the two fields that exist, named once so
// no caller spells the word.
func textFallback(reason string) string   { return playerFallback(reason, "text") }
func numberFallback(reason string) string { return playerFallback(reason, "number") }

// notTextSentence is the one sentence a stored value that is not text is
// answered with, built in ONE place: two spellings of one sentence is exactly
// the drift the corpus exists to prevent for the language, and this layer gets
// the same treatment.
func notTextSentence(setting string) string {
	return messagePrefix + setting + " is not text"
}

// numberFault is WHICH rule a number failed, and it exists so that the ORDER
// the questions are asked in lives in one place while the SENTENCE they are
// answered with lives in two.
//
// TWO WORDINGS, BECAUSE THE TWO SIDES ARE ABOUT DIFFERENT VALUES. A stored
// value is what the SETTING ANSWERED, and the value a fallback lands on is what
// the PLAN DECLARED; one wording over both would describe the setting's answer
// while talking about the declaration that replaced it. See storedNumberProblem
// and declaredNumberProblem.
type numberFault int

const (
	faultNone numberFault = iota
	faultNotFinite
	faultCountBelowOne
	faultTimeAtOrBelowZero
	faultBelowCraftTimeFloor
)

// countFault, secondsFault and craftTimeFault answer which rule a number
// failed, or faultNone.
//
// FINITENESS FIRST WITHIN EACH NUMBER, and not only for the message: a floor is
// a question only a finite number can be asked. A NaN is neither below 1 nor at
// or below zero, so a floor arm reached first would wave it through, and an
// infinity would be sorted by whichever side of the floor it fell on rather
// than told the one thing that is actually wrong with it.
//
// ONE FUNCTION PER NUMBER RATHER THAN ONE ACROSS ALL OF THEM, which is a change
// from the interleaved order this layer used to have: each number now falls
// back on its own and logs its own line, so a world where two of them are wrong
// answers about both instead of picking one. The residual refusal below walks
// them in the same per-number order, so there is one ordering in this file
// rather than two.
func countFault(v float64) numberFault {
	switch {
	case !finite(v):
		return faultNotFinite
	case v < 1:
		return faultCountBelowOne
	}
	return faultNone
}

func secondsFault(v float64) numberFault {
	switch {
	case !finite(v):
		return faultNotFinite
	case v <= 0:
		return faultTimeAtOrBelowZero
	}
	return faultNone
}

func craftTimeFault(v float64) numberFault {
	switch {
	case !finite(v):
		return faultNotFinite
	case v <= craftTimeFloor:
		return faultBelowCraftTimeFloor
	}
	return faultNone
}

// storedNumberProblem is what a value the SETTING ANSWERED WITH is told, and it
// is the reason a fallback line quotes. It names the setting rather than the
// technology, because the technology's own declaration is fine and the value
// came from outside it.
func storedNumberProblem(setting string, f numberFault) string {
	switch f {
	case faultNotFinite:
		return messagePrefix + setting + " holds a value that is not a finite number"
	case faultCountBelowOne:
		return messagePrefix + setting + " holds a research count below 1"
	case faultTimeAtOrBelowZero:
		return messagePrefix + setting + " holds a research time at or below zero"
	}
	return ""
}

// declaredNumberProblem is what the value a fallback LANDED ON is refused with,
// and it says declared default out loud: by the time this is reached the stored
// value is gone and the number being complained about is the one the plan
// wrote, which is an author's bug rather than a player's typing.
func declaredNumberProblem(setting string, f numberFault) string {
	switch f {
	case faultNotFinite:
		return messagePrefix + setting + " declares a default that is not a finite number"
	case faultCountBelowOne:
		return messagePrefix + setting + " declares a default research count below 1"
	case faultTimeAtOrBelowZero:
		return messagePrefix + setting + " declares a default research time at or below zero"
	}
	return ""
}

// storedCraftTimeProblem and declaredCraftTimeProblem are the same split for a
// recipe's bound crafting time, which names the recipe as well as the setting
// because that is the sentence this value has always been answered with.
func storedCraftTimeProblem(recipe, setting string, f numberFault) string {
	switch f {
	case faultNotFinite:
		return messagePrefix + "the recipe " + recipe + " reads its crafting time from " + setting +
			", which answers a value that is not a finite number"
	case faultBelowCraftTimeFloor:
		return messagePrefix + "the recipe " + recipe + " reads its crafting time from " + setting +
			", which answers at or below the engine floor (energy_required can't be <= 0.001)"
	}
	return ""
}

func declaredCraftTimeProblem(recipe, setting string, f numberFault) string {
	switch f {
	case faultNotFinite:
		return messagePrefix + "the recipe " + recipe + " reads its crafting time from " + setting +
			", whose declared default is not a finite number"
	case faultBelowCraftTimeFloor:
		return messagePrefix + "the recipe " + recipe + " reads its crafting time from " + setting +
			", whose declared default is at or below the engine floor (energy_required can't be <= 0.001)"
	}
	return ""
}

// planItemWorld is the World a text the player TYPED is resolved against: this
// plan's own item names answer ItemExists, and every other question is the real
// World's.
//
// MEASURED (Factorio 2.0.77): a text copied straight out of the setting's own
// composed description, "1 fkrecipes-example-steel-rivet, 10 water", refused
// with "no item or fluid is named fkrecipes-example-steel-rivet". The data
// planner resolves every text BEFORE it extends data.raw with its own items, so
// ItemExists cannot see one that is not there yet, and the description shows
// the player exactly those names. The overlay is the smaller of the two fixes:
// emitting the items first would also work and would reorder the whole stream.
//
// FluidExists AND ToolExists ARE UNTOUCHED. This library declares neither a
// fluid nor a tool, so a plan's own name is never either of them, and a pack
// text naming one still gets "is an item, not a science pack".
type planItemWorld struct {
	World
	items []string
}

// ItemExists answers for this plan's own items first, in declaration order. A
// slice and a scan rather than a map, for the reason every other lookup in this
// library is one: nothing here may depend on an iteration order.
func (p planItemWorld) ItemExists(name string) bool {
	for _, n := range p.items {
		if n == name {
			return true
		}
	}
	return p.World.ItemExists(name)
}

// ownItemWorld builds that overlay once per data plan, before any text is
// parsed. The names are the EMITTED ones, prefixed or legacy: they are what the
// description shows the player and what the recipe will carry.
func (l *Lib) ownItemWorld(w World, prefix string) planItemWorld {
	names := make([]string, 0, len(l.items))
	for _, it := range l.items {
		names = append(names, it.emittedName(prefix))
	}
	return planItemWorld{World: w, items: names}
}

// resolveTextList reads one text setting and answers with what it says.
//
// FOUR WAYS IN AND THREE OF THEM LAND ON THE AUTHOR'S OWN LIST:
//
//   - unreadable: the declared default applies, with the ordinary log line the
//     rest of the library uses for a setting it could not read;
//   - the word default: the AUTHOR's declared list with its ladders, which is
//     the pre-existing resolution path and gets no line of its own;
//   - not text at all, or a text the language refuses: the declared default
//     again, with ONE fallback line naming the setting and quoting the reason;
//   - anything else: parsed and resolved by the language, in the typed order.
//
// IT NEVER REFUSES, and that is the decision this round turned on. The two
// arms that used to stop the load are the two a PLAYER reaches by typing, and
// a refusal there is a lock-out rather than a diagnosis: see playerFallback for
// what the client actually does with one. The parser is untouched, and its rule
// is untouched with it: it still names the problem rather than guessing a
// substitute. What changed is what this caller does with the answer.
//
// A READABLE NON-STRING TAKES THE SAME PATH. The engine resets a wrong-typed
// stored value before any stage runs (measured), so it is reachable only
// through a hand-edited file; the line says so out loud, which is all a refusal
// ever bought and it buys it without stopping the game.
//
// THE WORLD HERE IS THE OVERLAY, planItemWorld, and every caller passes it: the
// language's resolver, its tag hint and its suggestion fold all have to see the
// items this plan is about to emit. Reading the setting itself goes through the
// overlay too, which delegates it.
func (l *Lib) resolveTextList(w World, res *resolution, s settingDecl, prefix, category string, kind listKind) parsedList {
	full := s.emittedName(prefix)
	v, ok := w.StartupSetting(full)
	if !ok {
		res.logs = append(res.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
		return parsedList{isDefault: true}
	}
	if v.Kind != KindStr {
		res.noteFallback(full, textFallback(notTextSentence(full)))
		return parsedList{isDefault: true}
	}
	parsed, problem := l.lang.parse(v.Str, kind, category, full, w)
	if problem != "" {
		// VERBATIM INSIDE THE LINE. The language already composed the whole
		// sentence, naming the setting, the entry and the problem, and it is
		// the same sentence the corpus pins in both languages; the fallback
		// line carries it with the shared prefix trimmed off, because the line
		// it sits in already opens with one.
		res.noteFallback(full, textFallback(problem))
		return parsedList{isDefault: true}
	}
	return parsed
}

// resolveIngredientsFrom is a recipe whose ingredients the player writes.
//
// TWO WORLDS, AND THE DIFFERENCE IS DELIBERATE. A text the player typed is
// resolved against the overlay, because it may name this plan's own items; the
// DECLARED ladders are walked against the real World, because a ladder is the
// author's list of things other mods might provide and its rungs are answered
// exactly as they were before the overlay existed.
func (l *Lib) resolveIngredientsFrom(w, text World, res *resolution, prefix string, r recipeDecl, s settingDecl) []resolvedIngredient {
	parsed := l.resolveTextList(text, res, s, prefix, r.spec.Category, listRecipe)
	if parsed.isDefault {
		// The pre-existing path, ladders and drop lines and all.
		return l.resolveIngredients(w, res, prefix, r.name, s.defIngredients)
	}
	list := make([]resolvedIngredient, 0, len(parsed.entries))
	for _, e := range parsed.entries {
		if e.kind == kindFluid {
			list = append(list, resolvedIngredient{kind: kindFluid, name: e.name, fluid: e.fluid})
			continue
		}
		list = append(list, resolvedIngredient{name: e.name, amount: e.amount})
	}
	// ONE LINE, AND IT IS THE CANONICAL RENDERING rather than the text as
	// typed: a player who wrote "iron-plate x2" reads back "2 iron-plate" and
	// learns the form the library would have written.
	res.logs = append(res.logs, "fkrecipes: "+r.emittedName(prefix)+" takes its ingredients from "+
		s.emittedName(prefix)+": "+l.lang.render(parsed.entries))
	return list
}

// noteIgnoredText says out loud that a text the player edited is not being
// used, because the dropdown beside it is on a preset.
//
// WITHOUT IT THE PLAYER EDITS A FIELD AND NOTHING HAPPENS, which is the worst
// shape a setting can have. It is a log line rather than a refusal because the
// player has not done anything wrong: the two fields are simply not both live.
//
// EDITED IS DECIDED BY THE LANGUAGE, not by comparing the text with the word.
// "default," is a tolerated trailing comma the language reads as the marker, so
// a comparison would call it an edit and tell the player their untouched field
// was ignored. This asks the same function the data path asks and takes the
// same answer; a text the language refuses is an edit, because it is certainly
// not the marker.
func (l *Lib) noteIgnoredText(text World, res *resolution, s settingDecl, prefix, category, dropdown, customValue string) {
	full := s.emittedName(prefix)
	v, ok := text.StartupSetting(full)
	// An unreadable or wrong-typed value is not an edit. Neither is refused
	// here: the text is not being read for real, and refusing a load over a
	// value nothing uses would be worse than saying nothing.
	if !ok || v.Kind != KindStr {
		return
	}
	parsed, problem := l.lang.parse(v.Str, listKindOf(s.kind), category, full, text)
	if problem == "" && parsed.isDefault {
		return
	}
	res.logs = append(res.logs, "fkrecipes: "+full+" is edited, but "+dropdown+
		" is not on "+customValue+", so the text is ignored")
}

// noteIgnoredNumber is the same sentence for the two NUMBERS a cost arm holds,
// the count and the seconds, and it exists because the pilot moved one under a
// tier and the log said nothing at all. A player who drags a slider and reads
// no line has been told their edit landed when it did not.
//
// EDITED IS A COMPARISON HERE, not a question for the language. A numeric
// setting has no reserved word standing for the author's answer: the DECLARED
// DEFAULT is the untouched value, and the engine stores it for a silent player
// exactly as it stores a moved one, so the comparison is the only thing that
// separates them.
//
// AN UNREADABLE OR WRONG-TYPED VALUE IS NOT AN EDIT, the same tolerance
// noteIgnoredText has and for the same reason: the field is not being read for
// real here, and refusing a load over a value nothing uses would be worse than
// saying nothing. It reads through the World readNumber reads through, which is
// the real one: a number is never resolved against the plan's own item overlay.
func noteIgnoredNumber(w World, res *resolution, s settingDecl, prefix, dropdown, customValue string) {
	full := s.emittedName(prefix)
	v, ok := w.StartupSetting(full)
	if !ok || v.Kind != KindNum || v.Num == s.defNum {
		return
	}
	res.logs = append(res.logs, "fkrecipes: "+full+" is edited, but "+dropdown+
		" is not on "+customValue+", so the number is ignored")
}

// resolveCustomCost is a research cost the player writes: the count and the
// seconds from their own settings, the packs from the text.
//
// THE COUNT AND THE SECONDS ARE ALWAYS READ, on both text paths. They are
// separate settings and the player may have moved them whether or not they
// touched the pack list, so there is no "untouched" arm for either.
//
// The pack ladders are walked against the real World and the pack TEXT against
// the overlay, for the reason resolveIngredientsFrom carries two of them.
//
// THE DATA PLANNER REACHES IT THROUGH Lib.customCost AND NEVER BY NAME, which
// is what keeps a plan that declares no pack setting from shipping it. See
// language: packsSetting is the only place this function's name appears
// outside this line.
func (l *Lib) resolveCustomCost(w, text World, res *resolution, prefix string, t techDecl, c *CustomCost) Value {
	countSetting := l.settings[c.Count.index-1]
	secondsSetting := l.settings[c.Seconds.index-1]
	packsSetting := l.settings[c.Packs.index-1]

	// EACH NUMBER IS HELD TO WHAT THE ENGINE TAKES WHERE IT IS READ, and one
	// that is not takes the setting's DECLARED DEFAULT with a line naming it.
	// All three fields here are the player's, so all three follow the same
	// rule; the lines come out in the order the values are read, which is the
	// order the cost line below names them.
	count := res.costNumber(w, countSetting, prefix, countFault)
	seconds := res.costNumber(w, secondsSetting, prefix, secondsFault)

	parsed := l.resolveTextList(text, res, packsSetting, prefix, "", listPacks)
	// AND THE POST-CONDITION, on whatever the two reads settled on. After a
	// fallback the value IS the declared default, so the only world this can
	// still refuse is a plan whose declared default is itself outside what the
	// engine takes. That is an AUTHOR bug: validateSettings refuses it at the
	// settings stage, which the engine runs before the data stage, so it
	// reaches here only through a host test that calls PlanData on its own.
	// It refuses, because an author's declaration is not a player's typing.
	if !res.refuseCostNumbers(countSetting.emittedName(prefix), secondsSetting.emittedName(prefix), count, seconds) {
		return refusedCost(count, seconds)
	}
	entries := parsed.entries
	if parsed.isDefault {
		// The declared packs with their ladders, dropped and logged one by one
		// exactly as a hand-rolled unit's are.
		entries = resolvePackLadders(w, res, t.name, packsSetting.defPacks)
	}
	packs := make([]Value, 0, len(entries))
	for _, e := range entries {
		packs = append(packs, Arr(Str(e.name), Num(float64(e.amount))))
	}
	// A parsed list can never be empty here: the language refuses none in a
	// pack list. So this is the ladder path having lost every pack, which is
	// the same refusal a hand-rolled unit gets.
	if len(packs) == 0 && res.packless == "" {
		res.packless = t.name
	}
	res.logs = append(res.logs, "fkrecipes: "+t.emittedName(prefix)+" takes its research cost from "+
		packsSetting.emittedName(prefix)+": count "+l.lang.amount(count)+
		", time "+l.lang.amount(seconds)+", packs "+l.lang.render(entries))
	return Obj(
		kv("count", Num(count)),
		kv("time", Num(seconds)),
		kv("ingredients", Arr(packs...)),
	)
}

// customPrereqs walks a Custom arm's Position ladder. The first technology the
// game has becomes the sole prerequisite, exactly as a chosen tier's source
// would; a ladder with no rung present leaves the technology unattached and
// says so, in the shape every other dropped ladder uses.
func customPrereqs(w World, res *resolution, tech string, position []string) []string {
	for _, name := range position {
		if w.TechExists(name) {
			return []string{name}
		}
	}
	res.logs = append(res.logs, "fkrecipes: "+tech+": none of "+strings.Join(position, ", ")+
		" is present, so the technology has no prerequisite")
	return nil
}

// readNumber is every numeric read a binding makes: the crafting time, the
// research count and the research time all come through here, so the line an
// unreadable one logs is one sentence in one place.
//
// IT SAYS WHETHER THE SETTING ANSWERED, and that second value is what keeps a
// fallback line honest: a setting the World has no number for was never HOLDING
// anything, so a declared default the engine would not take is an author bug
// found here rather than a player's stored value set aside. Only a value the
// setting actually answered with can fall back.
func (r *resolution) readNumber(w World, s settingDecl, prefix string) (float64, bool) {
	full := s.emittedName(prefix)
	if v, ok := w.StartupSetting(full); ok && v.Kind == KindNum {
		return v.Num, true
	}
	r.logs = append(r.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
	return s.defNum, false
}

// costNumber reads one of a custom cost's two numbers and holds it to what the
// engine takes, FALLING BACK to the setting's declared default rather than
// refusing.
//
// A NUMBER IS A FIELD THE PLAYER OWNS, exactly as the pack text beside it is,
// so it takes the same rule: see playerFallback for the client measurement that
// decided it. The declared minima and the engine's own reset rule keep a player
// from producing one of these through the settings screen, and a World is still
// an interface: a fixture that answers a NaN used to reach the amount formatter,
// and one that answers an infinity used to be rendered into the unit. The line
// names the SETTING, because that is the field somebody would go and fix.
//
// THE DEFAULT IS THE SAME ONE AN UNREADABLE SETTING TAKES, which is what makes
// this one shape rather than two: readNumber already answers with s.defNum for
// a value it cannot read, and this answers with it for a value it cannot use.
//
// ONLY A VALUE THE SETTING ANSWERED CAN FALL BACK, which is what readNumber's
// second answer is for. An unreadable setting was never HOLDING anything, so a
// declared default the engine would not take is an author bug refuseCostNumbers
// names as one; a fallback line there would send a player to a field whose
// stored value was never the problem.
func (r *resolution) costNumber(w World, s settingDecl, prefix string, fault func(float64) numberFault) float64 {
	full := s.emittedName(prefix)
	v, held := r.readNumber(w, s, prefix)
	if !held {
		return v
	}
	if f := fault(v); f != faultNone {
		r.noteFallback(full, numberFallback(storedNumberProblem(full, f)))
		return s.defNum
	}
	return v
}

// craftTimeNumber is costNumber for a recipe's bound crafting time: the same
// rule, with the sentence that names the recipe as well as the setting because
// that is the sentence this value has always been answered with.
//
// TWO RECIPES MAY READ ONE SETTING, which nothing else here can do, and that is
// why the line goes through noteFallback: one bad field on the settings screen
// is one problem, so it says so once however many recipes bind it. The recipe
// the line names is the FIRST in declaration order, the same one every run.
func (r *resolution) craftTimeNumber(w World, s settingDecl, prefix, recipe string) float64 {
	full := s.emittedName(prefix)
	v, held := r.readNumber(w, s, prefix)
	if !held {
		return v
	}
	if f := craftTimeFault(v); f != faultNone {
		r.noteFallback(full, numberFallback(storedCraftTimeProblem(recipe, full, f)))
		return s.defNum
	}
	return v
}

// refuseCostNumbers is the post-condition on the two numbers a unit is built
// from, and it answers whether the cost may be built.
//
// IT IS AN AUTHOR-SIDE GUARD NOW. Every value a player can hold has already
// fallen back to the setting's declared default by the time this runs, so the
// only world left for it is a declared default outside what the engine takes,
// which validateSettings refuses at the settings stage. The engine runs that
// stage first, so this answers only for a host test that calls PlanData on its
// own, and it stays because what it protects is the invariant that nothing this
// library emits carries a unit the engine would refuse.
//
// AND IT SAYS DECLARED DEFAULT. The stored value is gone by here, so the
// sentence a fallback line quoted would be describing a number nothing is
// holding any more; declaredNumberProblem is the same two rules in the same
// order with the wording that is true on this side.
//
// THE ORDER IS PER NUMBER, count then seconds, and inside each of them
// finiteness before the floor. It used to interleave the four checks so that a
// world with two bad numbers answered with the finiteness one; nothing observes
// that any more, because each number now falls back on its own and logs its own
// line, and one ordering in this file beats two.
func (r *resolution) refuseCostNumbers(countName, secondsName string, count, seconds float64) bool {
	if f := countFault(count); f != faultNone {
		r.refuse(declaredNumberProblem(countName, f))
		return false
	}
	if f := secondsFault(seconds); f != faultNone {
		r.refuse(declaredNumberProblem(secondsName, f))
		return false
	}
	return true
}

// refusedCost is the unit a refused custom cost hands back. It is never
// emitted: the refusal is recorded and PlanData stops the load before any op
// reaches the host. ONE ARM REACHES IT NOW, refuseCostNumbers over a declared
// default the settings stage would have refused; the text arm that used to
// share it falls back instead, and the shape is kept because a value has to
// come back from a function that returns one.
func refusedCost(count, seconds float64) Value {
	return Obj(kv("count", Num(count)), kv("time", Num(seconds)), kv("ingredients", Arr()))
}

// refuse records the FIRST refusal the resolution walk found.
//
// resolve answers with facts and PlanData decides which of them stops the load,
// which is the shape the crafting-time floor and the packless unit already use.
// A refusal that came out of the language is carried the same way: the walk
// runs to the end so the op stream is built from a complete resolution, and the
// planner refuses before any of it is handed to the host.
func (r *resolution) refuse(message string) {
	if r.refusal == "" {
		r.refusal = message
	}
}
