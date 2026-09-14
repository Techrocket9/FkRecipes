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
// parameters, and it is PER TABLE rather than per string.
//
// MEASURED (Factorio 2.0.77, build 84539), twice independently: ONE TABLE
// TAKES 20 PARAMETERS and the 21st refuses the load, `Too many parameters for
// localised string: 21 > 20 (limit).`, with a literal and a table parameter
// counting alike; nesting is capped at 20 LEVELS OF DEPTH and the 21st refuses,
// `Too deep recursion for localised string: 21 > 20 (limit).`, where the root
// table is level 1, every parameter sits one level below the table holding it,
// a plain-string parameter occupies a level of its own and the key at element 0
// does not; and there is NO GLOBAL TABLE BUDGET at all, a description holding
// 421 tables at depth 3 loading with exit 0. A recipe prototype carries the
// same two ceilings with its own prototype kind in the refusal text.
//
// THE RULE THE VALUE DRIVES IS UNCHANGED. A dropdown with more presets than fit
// NESTS rather than overflowing: each level carries at most this many
// parameters, and when there are more the last parameter is a nested localised
// string holding the rest by the same rule. Nesting spends depth, which is the
// budget with 20 levels in it, so the overflow is a fill rather than a wall.
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
	}
	for _, t := range l.techs {
		if c := t.spec.CostFrom; c != nil && l.validPacksSetting(c.Packs) {
			mark(c.Packs.index-1, "")
		}
	}
	return out
}

// numberBinding is what a NUMERIC setting is bound to: how many declarations
// read it at all, and whether any of those reads is a custom cost's count or
// seconds.
//
// THE TWO HALVES ARE SEPARATE BECAUSE THE TWO READS ARE. A crafting time two
// recipes share is ordinary and always was; a research number carries a
// composed description that names ONE dropdown as the thing deciding while it
// is 0, and a setting two technologies priced themselves with would be
// described by whichever of them composed last.
type numberBinding struct {
	count  int
	asCost bool
}

// numberSettingBindings walks the plan once and records who reads each numeric
// setting: a recipe's crafting time, and a custom cost's count and seconds
// under CostFrom.
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
		for _, h := range []IntSettingRef{c.Count, c.Seconds} {
			if l.validIntSetting(h) {
				out[h.index-1].count++
				out[h.index-1].asCost = true
			}
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
	}
	return out
}

// namedCostSources counts the cost sources a technology declares. Exactly one
// is the rule and the data planner is where it is refused, so every walk that
// steps past a technology naming some other number asks this one question.
//
// CostBy AND CostFrom COUNT AS ONE, because they are one cost: the dropdown is
// the tier and the three settings overwrite it field by field. Every other
// pairing is still two, so Unit beside either is refused exactly as it was.
func namedCostSources(spec *TechSpec) int {
	named := 0
	for _, set := range []bool{spec.CostOf != "", spec.Unit != nil, spec.CostBy != nil || spec.CostFrom != nil} {
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
// a text setting beside one, so it needs that pairing to be well formed just as
// much as the data stage does.
//
// IT REFUSES NOTHING THE OTHER TWO VALIDATORS ALREADY OWN. Where a recipe or a
// technology names two costs, or two ingredient sources this file did not add,
// the walk SKIPS that declaration and leaves the sentence to the data
// planner's own loop: a plan with two problems should be answered by the one
// its author is likelier to recognise, and "pick one" is that sentence.
func (l *Lib) validateBindings(prefix string) error {
	at := "fkrecipes: "

	// ONE DROPDOWN COMPOSES ONE DESCRIPTION, so one declaration may put a text
	// setting beside it. Counted in the two walks below, where the pairing is
	// proved, and refused after both of them so the sentence names the first
	// such setting in declaration order rather than whichever walk noticed
	// first.
	armedByRecipe := make([]int, len(l.settings))
	armedByTech := make([]int, len(l.settings))

	for _, r := range l.recipes {
		who := "the recipe " + r.name
		if r.spec.IngredientsFrom.index != 0 {
			if len(r.spec.Ingredients) > 0 {
				return errors.New(at + who + " names both Ingredients and IngredientsFrom; pick one")
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
		if r.spec.IngredientsFrom.index != 0 {
			armedByRecipe[by.Setting.index-1]++
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
		by := t.spec.CostBy
		if by != nil && !l.validDropdownSetting(by.Setting) {
			return errors.New(at + who + " names a cost setting that this plan never declared")
		}
		if c := t.spec.CostFrom; c != nil {
			if by != nil {
				armedByTech[by.Setting.index-1]++
			}
			if err := l.validateCustomCost(at, who, c, by != nil); err != nil {
				return err
			}
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
				" takes a text setting from more than one recipe; one dropdown composes one description")
		}
		if armedByTech[i] > 1 {
			return errors.New(at + "the setting " + full +
				" takes a text setting from more than one technology; one dropdown composes one description")
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

// validateCustomCost checks the three handles and the bounds the engine's own
// reset rule turns into a guarantee.
//
// THE BOUNDS ARE ON THE DECLARED SPEC, not on the value read. MEASURED: a
// numeric setting whose stored value falls outside its own bounds is RESET to
// the default rather than clamped, and a setting whose DEFAULT lies outside its
// own bounds refuses the load. So what a declaration promises here is what
// makes every value the data stage can ever read a legal one, and the data
// stage needs no arm for an illegal one at all.
//
// WHAT IS PROMISED DEPENDS ON WHETHER THERE IS A TIER, which is what hasTier
// carries. Beside a CostBy dropdown, 0 is the number's way of saying the word
// default: the declared default and the minimum are both 0, and the dropdown's
// tier supplies the field. With no dropdown there is nothing to defer to, so a
// minimum of at least 1 on the count and on the time is what keeps a unit the
// engine takes.
//
// AND A MAXIMUM EITHER WAY, because the field is one a player types into: an
// int setting with no maximum_value lets them ask for a research nobody
// finishes, and the settings screen has no other ceiling to offer them.
//
// PER SETTING, count then seconds, and inside each of them the default and the
// minimum before the maximum: a plan with two of these answers with the same
// one every run and in both languages.
func (l *Lib) validateCustomCost(at, who string, c *CustomCost, hasTier bool) error {
	if !l.validPacksSetting(c.Packs) {
		return errors.New(at + who + " reads its science packs from a setting that this plan never declared")
	}
	if !l.validIntSetting(c.Count) {
		return errors.New(at + who + " reads its research count from a setting that this plan never declared")
	}
	if !l.validIntSetting(c.Seconds) {
		return errors.New(at + who + " reads its research time from a setting that this plan never declared")
	}
	count := l.settings[c.Count.index-1]
	if hasTier {
		if count.defNum != 0 || !count.spec.HasMin || count.spec.Min != 0 {
			return errors.New(at + "the setting " + count.name +
				" backs a research count beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)")
		}
	} else if !count.spec.HasMin || count.spec.Min < 1 {
		return errors.New(at + "the setting " + count.name +
			" backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)")
	}
	if err := researchNumberMaximum(at, count); err != nil {
		return err
	}
	seconds := l.settings[c.Seconds.index-1]
	if hasTier {
		if seconds.defNum != 0 || !seconds.spec.HasMin || seconds.spec.Min != 0 {
			return errors.New(at + "the setting " + seconds.name +
				" backs a research time beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)")
		}
	} else if !seconds.spec.HasMin || seconds.spec.Min < 1 {
		return errors.New(at + "the setting " + seconds.name +
			" backs a research time but declares no minimum of at least 1 (the engine refuses a unit time of 0)")
	}
	return researchNumberMaximum(at, seconds)
}

// researchNumberMaximum is the ceiling both research numbers need, written once
// because the sentence is one sentence: the count and the time are the same
// kind of field to a player and the same kind of hole to a modpack.
//
// THE SENTENCE NAMES THE WHOLE PREDICATE, which is not "no maximum": a
// declaration of Between(0, 0) carries one and it is a ceiling no legal value
// can sit under, so a sentence that said the maximum was missing would be
// untrue of half the declarations that reach this line. "No maximum of at least
// 1" is true of both arms, and the Rust half carries it byte for byte.
func researchNumberMaximum(at string, s settingDecl) error {
	if !s.spec.HasMax || s.spec.Max < 1 {
		return errors.New(at + "the setting " + s.name +
			" backs a research number but declares no maximum of at least 1; a number the player types needs a ceiling it can reach")
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
// with, or Nil for the ones that carry none. Three settings get one:
//
//   - a TEXT setting, whose description is the consumer's own key followed by
//     the declared list written out, what the field takes, which setting
//     decides while it says default, and what an unusable text costs;
//   - a RESEARCH NUMBER, the count or the time of a CustomCost, whose
//     description is the consumer's own key followed by the range it takes and,
//     beside a research dropdown, what 0 means;
//   - a DROPDOWN WITH A TEXT SETTING BESIDE IT, whose description is the
//     consumer's own key followed by one line per preset and then the sentence
//     that says the text overrides it.
//
// THE COMPOSITIONS EXIST BECAUSE THE ENGINE FORBIDS THE ALTERNATIVES. Filling
// the text from the player's old dropdown choice is impossible: the settings
// stage sees no stored value (measured: data.raw is empty there), nothing at a
// data stage can write a setting, and settings.startup is read only at the
// control stage. The settings screen has no conditional visibility either
// (measured), so no field can be hidden while the other one decides, and
// saying which is which in the description is what the library can do instead.
func (l *Lib) settingDescriptions(prefix string) []Value {
	out := make([]Value, len(l.settings))
	for i := range out {
		out[i] = Nil()
	}
	numbers := l.researchNumberSettings()
	for i, s := range l.settings {
		if numbers[i].bound {
			out[i] = numberDescription(s.emittedName(prefix),
				l.researchRangeLine(i, s, numbers[i].dropdown))
			continue
		}
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
		out[i] = textDescription(s.emittedName(prefix), l.lang.render(entries),
			l.textSwitchLine(i), s.kind == settingIngredients)
	}
	// Recipes then technologies, in declaration order, which is the order a
	// dropdown's own description is built in when two declarations put a text
	// setting beside one dropdown. validateBindings refuses two recipes and two
	// technologies over one dropdown, each with its own sentence; what it does
	// NOT refuse is one recipe and one technology reaching the same one, and
	// there the technology's preset list is the one that lands, because this
	// walk runs second. That is a plan nobody has written and a sentence nobody
	// has agreed on, so it is written down here rather than answered on a guess.
	for _, r := range l.recipes {
		by := r.spec.IngredientsBy
		// EXACTLY THE CONDITION validateBindings CHECKED UNDER, len(Ingredients)
		// and all: a recipe it stepped past is one whose choices it never
		// validated, and rendering an unvalidated choice would dereference an
		// item handle nothing proved. The data planner refuses that plan by
		// name; this one just says nothing about it.
		if by == nil || len(r.spec.Ingredients) > 0 || !l.validDropdownSetting(by.Setting) {
			continue
		}
		// A DROPDOWN WITH NO TEXT SETTING BESIDE IT COMPOSES NOTHING, because
		// there is nothing the presets have to be read in the language of and
		// nothing to say the text overrides them.
		if !l.validIngredientsSetting(r.spec.IngredientsFrom) {
			continue
		}
		i := by.Setting.index - 1
		full := l.settings[i].emittedName(prefix)
		params := make([]Value, 0, len(by.Choices)+3)
		params = append(params, localeRef("mod-setting-description", full, full))
		// THE TEXT SETTING IS WHAT GUARANTEES THE LANGUAGE. It is an
		// IngredientsSettingRef, which only IngredientsSetting issues, and that
		// constructor installs the renderer.
		for _, c := range by.Choices {
			params = append(params, presetLine(full, c.Value,
				Str(ingredientPresetHead+l.lang.render(l.declaredIngredientEntries(prefix, c.Ingredients)))))
		}
		// THE WRAP, DISCLOSED WHERE THE WRAPPED TEXT IS. Every line above this
		// one is a typeable list, and a list is the only thing in a tooltip a
		// player copies. See listWrapLine.
		params = append(params, Str(listWrapLine))
		params = append(params, Str(dropdownSwitchLine(l.relativeOrder(i, r.spec.IngredientsFrom.index-1))))
		out[i] = localisedGroup(params)
	}
	for _, t := range l.techs {
		if !l.costDropdownComposesPresetLines(&t.spec) {
			continue
		}
		by := t.spec.CostBy
		c := t.spec.CostFrom
		i := by.Setting.index - 1
		full := l.settings[i].emittedName(prefix)
		params := make([]Value, 0, len(by.Choices)+2)
		params = append(params, localeRef("mod-setting-description", full, full))
		for _, c := range by.Choices {
			params = append(params, presetLine(full, c.Value, costPresetTail(c)...))
		}
		params = append(params, Str(dropdownSwitchLine(l.relativeOrder(i, c.Packs.index-1))))
		out[i] = localisedGroup(params)
	}
	return out
}

// costDropdownComposesPresetLines is the ONE SPELLING of "this technology's
// cost dropdown has a preset list composed onto its description". Three readers
// need exactly this predicate and none of them may spell it again:
// settingDescriptions, which composes the lines; the locale checker's
// dropdownsWithComposedDescription, which is what makes that dropdown's
// [mod-setting-description] entry required; and composedGameKeyAdvisories,
// which says the one thing this library has to say about the GAME's key those
// lines name.
//
// A THIRD SPELLING WAS WHAT MADE THE GUARDS UNTESTABLE. Each of the four
// conditions here is the composition's own, so a dropped one is a composed line
// this walk does not know about or a walk that names a line nothing composes;
// with one spelling, the required-description findings exercise every one of
// them and the advisory walk inherits that for free.
func (l *Lib) costDropdownComposesPresetLines(spec *TechSpec) bool {
	by := spec.CostBy
	c := spec.CostFrom
	return by != nil && c != nil && namedCostSources(spec) == 1 &&
		l.validDropdownSetting(by.Setting) && l.validPacksSetting(c.Packs)
}

// relativeOrder is the word that says where the setting at other sits on the
// settings screen relative to the one at self: above or below.
//
// IT COMPARES THE EMITTED ORDER STRINGS, the same ones PlanSettings writes into
// the prototypes, because that is what the engine sorts by. A composed sentence
// that said "the option chosen above" would otherwise be a guess about a
// declaration order the consumer is free to choose, and OrderAfter and the
// Legacy constructors both let them choose one where the guess is wrong.
func (l *Lib) relativeOrder(self, other int) string {
	if l.settings[other].emittedOrder(other) < l.settings[self].emittedOrder(self) {
		return "above"
	}
	return "below"
}

// textSwitchLine is the sentence on a TEXT setting that says what decides while
// it holds the reserved word.
func (l *Lib) textSwitchLine(i int) string {
	if d := l.textSwitchDropdown(i); d >= 0 {
		return "\nWhile this says default the option chosen " + l.relativeOrder(i, d) + " applies; anything else applies instead of it."
	}
	return "\nWhile this says default this mod's own list applies."
}

// dropdownSwitchLine is the sentence appended to a DROPDOWN's composed
// description: the text setting beside it wins whenever it is not on the word.
func dropdownSwitchLine(where string) string {
	return "\nThe setting " + where + " applies instead while it does not say default."
}

// textSwitchDropdown is the dropdown setting that decides while the text
// setting at i says default, or -1 when the declaration that reads it has none.
//
// ONE WALK, TWO READERS: the composition above and the composition on the
// dropdown itself have to agree about which pair they are describing, and a
// second walk spelling the same condition is how the two could describe
// different pairs.
func (l *Lib) textSwitchDropdown(i int) int {
	for _, r := range l.recipes {
		if !l.validIngredientsSetting(r.spec.IngredientsFrom) || r.spec.IngredientsFrom.index-1 != i {
			continue
		}
		by := r.spec.IngredientsBy
		if by != nil && len(r.spec.Ingredients) == 0 && l.validDropdownSetting(by.Setting) {
			return by.Setting.index - 1
		}
		return -1
	}
	for _, t := range l.techs {
		c := t.spec.CostFrom
		if c == nil || namedCostSources(&t.spec) != 1 || !l.validPacksSetting(c.Packs) || c.Packs.index-1 != i {
			continue
		}
		if by := t.spec.CostBy; by != nil && l.validDropdownSetting(by.Setting) {
			return by.Setting.index - 1
		}
		return -1
	}
	return -1
}

// researchNumber is what a research count or time setting composes from: that
// it backs one at all, and which dropdown decides while it is 0.
type researchNumber struct {
	bound    bool
	dropdown int // -1 when the technology declares no CostBy
}

// researchNumberSettings marks the settings a CustomCost prices a research
// with. Both planners and the locale checker ask it, so the description, the
// locale obligation and the range sentence are decided once.
//
// IT STEPS PAST EXACTLY WHAT validateBindings STEPS PAST: a technology naming
// some other number of cost sources is answered by "exactly one", and nothing
// validated its CustomCost, so composing a range out of bounds nobody checked
// would be a description about a declaration the data planner refuses.
func (l *Lib) researchNumberSettings() []researchNumber {
	out := make([]researchNumber, len(l.settings))
	for i := range out {
		out[i].dropdown = -1
	}
	for _, t := range l.techs {
		c := t.spec.CostFrom
		if c == nil || namedCostSources(&t.spec) != 1 {
			continue
		}
		dropdown := -1
		if by := t.spec.CostBy; by != nil && l.validDropdownSetting(by.Setting) {
			dropdown = by.Setting.index - 1
		}
		for _, h := range []IntSettingRef{c.Count, c.Seconds} {
			if l.validIntSetting(h) {
				out[h.index-1] = researchNumber{bound: true, dropdown: dropdown}
			}
		}
	}
	return out
}

// researchRangeLine is what a research number's description says about the
// range it takes, and about what 0 means where a dropdown decides.
//
// THE NUMBERS COME OUT OF THE AMOUNT FORMATTER the language already pins byte
// for byte across the two halves, rather than out of either language's own
// float formatting: this sentence is compared in the mirror transcript, and two
// standard libraries agree about 100000 right up until they do not.
//
// A BOUND IS THERE BECAUSE validateCustomCost PROVED IT. Both planners run it
// in front of this walk, and researchNumberSettings steps past exactly the
// declarations it steps past, so the maximum is declared and the minimum is
// too.
func (l *Lib) researchRangeLine(i int, s settingDecl, dropdown int) string {
	if dropdown >= 0 {
		return "\nA whole number from 0 to " + l.lang.amount(s.spec.Max) +
			". While it is 0 the option chosen " + l.relativeOrder(i, dropdown) + " decides."
	}
	return "\nA whole number from " + l.lang.amount(s.spec.Min) + " to " + l.lang.amount(s.spec.Max) + "."
}

// localeRef is how this library references a locale key: the engine's own
// ALTERNATIVES form, {"?", {"section.key"}, "raw"}, and never the bare
// {"section.key"} it used to emit.
//
// A BARE KEY THE GAME DOES NOT DEFINE COSTS THE WHOLE THING IT SITS IN, which
// is measured and not argued (Factorio 2.0.77 build 84539, this repository's
// client probe and its headless arm agreeing). On a SETTING, a composed
// description holding one undefined key leaves the row with no info icon and no
// tooltip at all, while every other row keeps both; on a RECIPE, the entire
// description block disappears from the tooltip, taking the literal sentences
// this library wrote itself with it. Neither costs an exit code, an engine
// warning or a fkrecipes: line, and the engine's own --dump-data says the
// description is present either way, so no headless gate can see it.
//
// AN UNDEFINED KEY IS A FAILED ALTERNATIVE for `?`, which is the fact the whole
// form rests on: it does not resolve to the text `Unknown key: "..."` and win,
// it fails, and the next alternative renders. Measured on the client beside the
// failure it cures, in one screen: every `?` row renders and the plain join
// does not.
//
// THE RAW FALLBACK IS LAST, AND THAT IS A RULE AND NOT A STYLE. A plain string
// alternative ALWAYS resolves, so a raw string anywhere but the end
// short-circuits every alternative after it and the key would never be
// consulted; and when every alternative fails the result is the LAST
// alternative's own `Unknown key: "..."` marker, so a key in the last slot is
// the marker this form exists to avoid. Both reasons point the same way.
//
// IT COSTS ONE LEVEL OF DEPTH AND NOTHING ELSE. The wrapper occupies exactly
// one parameter slot of the table holding it, the same as the bare key table it
// replaces, against the measured ceilings maxLocalisedParams records (20
// parameters per table, 20 levels of depth, no global table budget). The
// library's realistic worst composition loses nothing it can reach: the
// theoretical preset ceiling drops from 342 to 323 on a COST dropdown, and a
// 323-preset description carrying 647 wrappers both loads and resolves in
// full. An INGREDIENT dropdown's is 322 rather than 323, by arithmetic and not
// by a second measurement: listWrapLine spends one parameter slot a preset
// would otherwise have, and nothing else about the shape differs.
func localeRef(section, key, raw string) Value {
	return Arr(Str("?"), Arr(Str(section+"."+key)), Str(raw))
}

// textDescription is the whole localised_description a TEXT setting is emitted
// with: the consumer's own entry, then the four things this library owes the
// player about the field beside it.
//
// SIX PARAMETERS, and none of them a table beyond the consumer's key and the
// localeRef wrapper around it, so the twenty-parameter ceiling
// maxLocalisedParams records is nowhere near reached and the shape needs no
// nesting rule of its own.
//
// THE FIVE LINES ARE THE ANSWER TO WHAT A CLIENT MEASUREMENT FOUND. A player
// standing in the Mod Settings screen reads the tooltip whole (measured on
// 2.0.77 on a DROPDOWN's composed description, one line per preset: seven
// lines rendered readable and unclipped; the ceilings on a composed
// description are the parameter count maxLocalisedParams holds and the nesting
// depth its comment records, neither of which is a line count), so the
// description is where the library can say what the field takes; the closed
// dropdown's LABEL beside it is truncated at about 37 characters, which is why
// nothing a player needs may live in a label. The default line shows the list
// the word default stands for, in the internal names the field actually takes;
// the wrap line says that a list too long for the tooltip is still one list,
// which is the one thing about that line a player cannot see; the format line
// says so in words and states the ceiling; the switch line says
// which of the two fields is deciding, which the screen cannot show because it
// has no conditional visibility at all (measured); and the fallback line says
// what a text this library cannot use costs, which before it was stated nowhere
// a player looks.
//
// ONE COMPOSITION, TWO READERS. The settings planner emits this; CheckLocale
// asks the same function for the same shape with the list left out, so a line
// deleted here is a finding rather than a silent loss. See guardedTextDescription.
//
// THE WRAP LINE SITS DIRECTLY UNDER THE DEFAULT LINE, because the default line
// is the list it is about and the one a player copies. The format line below it
// still says "as the default line above does", which two lines up is as true as
// one.
//
// INGREDIENTS SAYS WHICH OF THE TWO TEXT SETTINGS THIS IS, and the only thing
// it decides is whether the format line names the word none: see
// textFormatLine.
func textDescription(full, rendered, switchLine string, ingredients bool) Value {
	return Arr(
		Str(""),
		localeRef("mod-setting-description", full, full),
		Str("\ndefault: "+rendered),
		Str(listWrapLine),
		Str(textFormatLine(ingredients)),
		Str(switchLine),
		Str(textFallbackLine),
	)
}

// numberDescription is the whole localised_description a RESEARCH NUMBER is
// emitted with: the consumer's own entry, then the range the field takes.
//
// IT IS THE ONLY PLACE THE RANGE IS STATED. The settings screen shows a numeric
// field with no visible bounds, and 0 there means something the player cannot
// guess: the dropdown beside it decides. Both sentences live in
// researchRangeLine, and this is the shape they are emitted in.
func numberDescription(full, rangeLine string) Value {
	return Arr(
		Str(""),
		localeRef("mod-setting-description", full, full),
		Str(rangeLine),
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
func textFormatLine(ingredients bool) string {
	line := "\nWrite internal names, as the default line above does, in at most " +
		strconv.Itoa(maxListChars) + " characters."
	if ingredients {
		line += ingredientNoneClause
	}
	return line
}

// ingredientNoneClause is the word none, disclosed on the one setting kind that
// takes it.
//
// THE WORD IS A FEATURE AND WAS DOCUMENTED NOWHERE A PLAYER LOOKS. none empties
// an ingredient list, the recipe reaches the game with no ingredients at all,
// and the recycling recipe the engine derives from it goes with it. That is a
// legitimate thing for a player to want, so it is kept and said out loud.
//
// AND IT IS NOT SAID ON A PACKS SETTING, which is why it is a clause of its own
// rather than part of the sentence above it. A packs list REFUSES the word,
// with "research takes at least one science pack": telling a player to type a
// word the library turns down would be worse than saying nothing at all.
const ingredientNoneClause = " The word none empties the list, so the recipe costs nothing to craft."

// listWrapLine is what the engine's own wrapping costs a player who copies a
// line, said where the wrapped line is.
//
// THE CONTINUATION STARTS AT THE LEFT MARGIN (measured on the client): a list
// too long for the tooltip breaks, and the second half is not indented under
// the first, so it reads as a line of its own and a player who copies what
// looks like a whole line loses the last ingredient. The wrap is the engine's
// and no composition can change it; what a composition can do is say that the
// continuation belongs to the line above it.
//
// IT SAYS "THE CONTINUATION" AND NOT "BOTH LINES", AND THAT IS ARITHMETIC AND
// NOT STYLE. Two is not a bound on anything here. The wrap threshold is near
// 57 to 60 characters (measured on the client), and the list this line renders
// is the AUTHOR'S OWN DECLARED LIST, which has no bound short of the
// language's 2000-character parse ceiling, so three and more visual lines are
// reachable. On an ingredient dropdown the line above the list is the
// consumer's LOCALISED LABEL, which can wrap on its own and is not a list at
// all. A player who trusted "both lines are one list" over a list that wrapped
// twice would copy two of three lines and drop the tail, which is the exact
// failure this sentence exists to prevent, so the sentence names the
// RELATIONSHIP (a continuation belongs to what it continues) instead of
// counting lines it cannot count.
//
// IT GOES WHEREVER A TYPEABLE LIST IS RENDERED AND NOWHERE ELSE: a text
// setting's default line, and an ingredient dropdown's preset lines. A cost
// dropdown's preset is a localised label followed by a localised technology
// name, prose in one vocabulary with nothing in it to copy, so the sentence
// there would be about a hazard that preset does not carry.
const listWrapLine = "\nA list too long for one line continues on the next; the continuation is part of the same list."

// textFallbackLine is what happens to a text this library cannot use.
//
// IT IS THE ONE THING THE SCREEN CANNOT SHOW. The text is set aside and the
// field then decides exactly as it does while it holds the reserved word
// (decision 2), so a player whose text went unused sees a settings screen that
// still holds it and a game that ignores it. Saying so in the description is
// the only warning available before the fact.
//
// "BEHAVES AS THOUGH IT SAID DEFAULT" POINTS AT THE SWITCH LINE, and the
// wording is chosen for where it lands on the screen. The line used to read
// "that default applies instead", which is deictic, and its nearest antecedent
// three rows above is the DEFAULT LINE, which renders the author's declared
// list and nothing else; beside a dropdown that is a contradiction a player can
// read in one glance (measured: the tooltip said one list, the recipe the game
// built was another). textSwitchLine composes the row immediately above this
// one and already says what the word default does in THIS field: the option
// chosen above or below where there is a dropdown, this mod's own list where
// there is not. Pointing at that row is what makes this line true on every
// preset.
//
// IT PROMISES THE NARROW CLAIM AND NOT A LOAD, which is what the wording is
// for. Setting a text aside is not the same as loading: the list that then
// decides is held to every rule it always was, so a modpack in which it
// cannot produce a legal result still stops the load, and on a refused load the
// log ops never reach the host at all, so the load error is the only place the
// reason can be: resolution.fallbackFact is what puts it there, which is what
// keeps this line's second clause true. Naming both places, the log or the load
// error, is therefore the whole claim this line is allowed to make.
const textFallbackLine = "\nA text this mod cannot use is set aside and the field behaves as though it said default; the reason is in the log, or in the load error if the load stops anyway."

// presetLine is one preset's line in a composed dropdown description.
//
// THE VALUE IS LABELLED BY ITS OWN LOCALE ENTRY, not by its raw key, because
// the settings screen shows the player the localised label and a description
// naming the key would not match anything they can see. That entry is the one
// the dropdown already needs for the value to be readable at all, so the
// composition adds no locale obligation of its own; and the entry the
// dropdown needs is REQUIRED by the locale checker, so where it is missing the
// checker has already said so and localeRef's raw fallback shows the player
// the internal value, which is a thing they can type.
//
// THE TAIL IS VALUES RATHER THAN A STRING because a cost line ends in a
// localised name and an ingredient line ends in a rendering this library wrote.
// Whatever the tail holds, the whole line is ONE parameter of the group above
// it, so localisedGroup's nesting rule counts it as one; and the tables the
// tail brings are the line's own elements, so they sit BESIDE the label, at
// the same level, not below it. A one-preset cost description is four table
// levels deep whichever tail it carries, which is what
// TestPlanSettingsComposesACostDropdownDescription's golden shows: three, plus
// the one level every localeRef wrapper spends.
func presetLine(setting, value string, tail ...Value) Value {
	items := make([]Value, 0, 3+len(tail))
	items = append(items, Str(""), Str("\n"), localeRef("string-mod-setting", setting+"-"+value, value))
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
// the locale checker requires nothing new. It says something new, once, as an
// ADVISORY: defining that key in the consumer's own .cfg would rename base's
// technology for every mod in the game, because Factorio's locale namespace is
// flat, and this checker's own collision scan exists for that hazard. See
// composedGameKeyAdvisories.
//
// WHERE THE TECHNOLOGY OR ITS ENTRY IS MISSING, THE LINE DEGRADES TO THE RAW
// INTERNAL NAME rather than to the marker or to nothing, because localeRef
// wraps it: the preset line reads "\n<value>: cost of <source>" and the tooltip
// survives whole. Before the wrapper a source no installed mod declared cost
// the consumer the entire tooltip, silently, which is what the client probe
// measured.
func costPresetTail(c CostChoice) []Value {
	if len(c.Sources) == 0 {
		return []Value{Str(": the fallback cost")}
	}
	return []Value{Str(": cost of "), localeRef("technology-name", c.Sources[0], c.Sources[0])}
}

// localisedGroup wraps parameters in a concatenating localised string, nesting
// when there are more than the engine takes. See maxLocalisedParams: each level
// holds at most twenty parameters, and a level that would need more keeps the
// first nineteen and hands the rest to a nested group in the twentieth slot.
//
// THE FILL POINT IS PER DROPDOWN KIND, because the presets are not the only
// thing at the top level. Beside them sit the consumer's own description key
// first and the switch line last, and on an INGREDIENT dropdown the wrap line
// between them, so an ingredient dropdown stays flat up to SEVENTEEN presets
// and a cost dropdown up to EIGHTEEN. Both are far above every dropdown anyone
// has written; the nesting exists so that the one past the fill point is a line
// in a tooltip rather than a load failure naming nothing useful.
// TestIngredientDropdownDescriptionNestsPastSeventeenPresets and
// TestCostDropdownDescriptionNestsPastNineteenPresets pin the two points.
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
// legal result stops the load either way. Such a refusal carries the ONE FACT
// that a stored value was set aside (resolution.fallbackFact) and NO route to
// the settings screen, because the client's error dialog has none: see PlanData
// for the client walk that settled it.
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
// list, or the number of a slider. It is now in the sentence TWICE, because the
// line used to say the mod loaded with its own default instead and that is
// false wherever a preset dropdown sits beside the field: the set-aside value
// leaves the dropdown's CURRENTLY CHOSEN preset deciding, not the declaration.
// Saying the mod loaded as though that field had been left alone is true
// there, true on every preset, true where no dropdown exists, and true of a
// number as well as a text. The ROUTE is unchanged and stays: this line is
// written on a load that SUCCEEDED, so the settings screen really is reachable.
//
// TAIL is what the field costs beyond being wrong, and it is a parameter
// rather than a branch on the reason so that no sentence here is chosen by
// reading another sentence. Only a recipe's ingredient text has one: see
// recipeTextFallback.
func playerFallback(reason, field, tail string) string {
	return messagePrefix + "ERROR: " + strings.TrimPrefix(reason, messagePrefix) +
		". The mod loaded as though that " + field + " had been left alone; fix the " + field +
		" under Settings > Mod settings > Startup, then restart." + tail
}

// textFallback and numberFallback are the two fields that exist, named once so
// no caller spells the word.
func textFallback(reason string) string   { return playerFallback(reason, "text", "") }
func numberFallback(reason string) string { return playerFallback(reason, "number", "") }

// recipeTextFallback is textFallback for a RECIPE'S INGREDIENT LIST, which is
// the one fallback whose fix costs the player something the engine will not
// give back.
//
// THE PACK TEXT AND THE TWO NUMBERS DO NOT CARRY IT. Repricing a research
// destroys nothing; changing a recipe empties every assembling machine holding
// ingredients the new list does not use, measured and irreversible. See
// recipeChangeSentence, which the recipe's own tooltip note carries too.
func recipeTextFallback(reason string) string {
	return playerFallback(reason, "text", " "+recipeChangeSentence)
}

// movesIngredients answers whether a TEXT setting bound to this target decides
// a recipe's ingredient list, which is the one fallback in the library that
// changes what a recipe is made of.
//
// ONE PREDICATE, TWO READERS, and that is the whole reason it has a name. The
// ERROR line's tail and the prototype tooltip's tail are the same measured
// sentence about the same engine cost, so they must be true of exactly the same
// set of fallbacks. Deriving either one from the prototype KIND instead would
// put the sentence on a crafting-time fallback, which moves energy_required and
// leaves the ingredient list byte for byte.
//
// IT IS ASKED OF A TEXT SETTING ONLY. A crafting time and a research number
// never move an ingredient list whatever they are bound to, so those callers
// pass false outright rather than asking.
func movesIngredients(tgt noteTarget) bool { return !tgt.tech }

// textFallbackFor is which of the two a text setting's fallback line is,
// decided by whether the text moves a recipe's ingredient list.
func textFallbackFor(tgt noteTarget, reason string) string {
	if movesIngredients(tgt) {
		return recipeTextFallback(reason)
	}
	return textFallback(reason)
}

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

// deferrableCountFault and deferrableSecondsFault are the same two rules with
// 0 let through, which is what a research number beside a CostBy dropdown means
// by 0: the dropdown decides. Everything else the engine would refuse is still
// refused, so a number that IS in force is one the engine takes.
//
// A SEPARATE PAIR RATHER THAN A FLAG ON THE ORIGINALS, because the originals
// are also the AUTHOR-side post-condition over a built unit, where 0 is a
// research nobody can finish and has to stay a fault.
func deferrableCountFault(v float64) numberFault {
	if v == 0 {
		return faultNone
	}
	return countFault(v)
}

func deferrableSecondsFault(v float64) numberFault {
	if v == 0 {
		return faultNone
	}
	return secondsFault(v)
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
// FOUR WAYS IN AND THREE OF THEM LEAVE THE FIELD WHERE A PLAYER WHO TYPED
// NOTHING LEFT IT, which beside a dropdown is the dropdown's chosen preset and
// on its own is the author's declared list with its ladders:
//
//   - unreadable: the field decides as though it held the reserved word, with
//     the ordinary log line the rest of the library uses for a setting it
//     could not read;
//   - the word default: the same path, which is the pre-existing one and gets
//     no line of its own;
//   - not text at all, or a text the language refuses: the same path again,
//     with ONE fallback line naming the setting and quoting the reason;
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
func (l *Lib) resolveTextList(w World, res *resolution, s settingDecl, prefix, category string, kind listKind, tgt noteTarget) parsedList {
	full := s.emittedName(prefix)
	// THE TARGET IS WHAT PICKS THE SENTENCE, not the list kind beside it, and
	// the two agree by construction: a recipe target reaches this function only
	// for its own ingredient list and a technology target only for its pack
	// text. Choosing off the target is what keeps the ERROR line and the
	// prototype note the target also fills saying the same thing about the same
	// prototype. See textFallbackFor.
	v, ok := w.StartupSetting(full)
	if !ok {
		res.logs = append(res.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
		return parsedList{isDefault: true}
	}
	if v.Kind != KindStr {
		res.noteFallback(tgt, full, textFallbackFor(tgt, notTextSentence(full)), movesIngredients(tgt))
		return parsedList{isDefault: true}
	}
	parsed, problem := l.lang.parse(v.Str, kind, category, full, w)
	if problem != "" {
		// VERBATIM INSIDE THE LINE. The language already composed the whole
		// sentence, naming the setting, the entry and the problem, and it is
		// the same sentence the corpus pins in both languages; the fallback
		// line carries it with the shared prefix trimmed off, because the line
		// it sits in already opens with one.
		res.noteFallback(tgt, full, textFallbackFor(tgt, problem), movesIngredients(tgt))
		return parsedList{isDefault: true}
	}
	return parsed
}

// typedIngredients is a list the player wrote, in the typed order, ready for a
// recipe. The language has already resolved every name against the overlay, so
// nothing here walks a ladder or drops an entry.
func typedIngredients(parsed parsedList) []resolvedIngredient {
	list := make([]resolvedIngredient, 0, len(parsed.entries))
	for _, e := range parsed.entries {
		if e.kind == kindFluid {
			list = append(list, resolvedIngredient{kind: kindFluid, name: e.name, fluid: e.fluid})
			continue
		}
		list = append(list, resolvedIngredient{name: e.name, amount: e.amount})
	}
	return list
}

// ingredientsFromLine is the ONE line a recipe whose text is in force logs, and
// it is the CANONICAL RENDERING rather than the text as typed: a player who
// wrote "iron-plate x2" reads back "2 iron-plate" and learns the form the
// library would have written.
//
// THE CLAUSE IS THE MERGE LINE'S VOICE, and it is only there when a dropdown is
// also declared. The ingredient override is TOTAL, so the clause says the
// choice is set aside; a research cost's override is per field, and its clause
// says what the tier still supplies instead. One sentence cannot serve both.
func (l *Lib) ingredientsFromLine(recipe, setting, dropdown, chosen string, parsed parsedList) string {
	line := "fkrecipes: " + recipe + " takes its ingredients from " + setting + ": " + l.lang.render(parsed.entries)
	if dropdown == "" {
		return line
	}
	return line + "; the " + dropdown + " choice " + chosen + " is set aside"
}

// costTier is what a technology's CostBy dropdown settled on, handed to a
// custom cost so the fields the player left at their default can come from it.
//
// A ZERO VALUE IS NO TIER AT ALL, which is TechSpec.CostFrom on its own: there
// is nothing to defer to, so the three settings are the whole price and the
// cost is built fresh from them exactly as it always was.
type costTier struct {
	has bool
	// unit is the tier's own unit map, whatever it holds: a count_formula, a
	// max_level, fields no version of this library knows about. Overriding a
	// field rather than rebuilding the map is what keeps every one of them.
	unit     Value
	dropdown string
	chosen   string
	// source is the technology the tier settled on, and it rides here for ONE
	// reason: the tier arm may already have said, IN THE LOG, that the source
	// named no science pack this game has and that the mod's declared cost
	// applies instead. A typed pack list makes that false, and taking the line
	// back needs the name it was composed from. Empty where the tier fell back
	// without settling on a source, which is the arm that says nothing of the
	// kind.
	source string
	// note is the technology's note slot AS IT STOOD BEFORE THE TIER WAS
	// PRICED, and it is what a typed pack list puts back instead of retracting
	// the tier's sentences one at a time. It is the TOOLTIP half of what source
	// used to do, and it covers what naming sentences cannot: see
	// resolution.noteAt.
	note string
}

// resolveCustomCost is a research cost the player writes: the count and the
// time from their own settings, the packs from the text.
//
// THE THREE ARE ALWAYS READ, whatever the tier says, because reading them is
// how the library finds out whether any of them is in force. Each one that is
// not at its declared default overrides the tier; each one that is comes FROM
// the tier, or from the setting's own declared default where there is no tier.
// Nothing is ever edited and ignored, which is the whole point of the shape.
//
// THE PACK LADDERS ARE WALKED AGAINST THE REAL WORLD AND THE PACK TEXT AGAINST
// THE OVERLAY, for the reason the recipe path carries two of them.
//
// IT ANSWERS WHETHER THE CUSTOM COST APPLIES AT ALL. With a tier and nothing
// non-default the answer is false and the caller emits the tier byte for byte,
// which is the load a player who never opened the settings screen gets.
//
// THE DATA PLANNER REACHES IT THROUGH Lib.customCost AND NEVER BY NAME, which
// is what keeps a plan that declares no pack setting from shipping it. See
// language: packsSetting is the only place this function's name appears
// outside this line.
func (l *Lib) resolveCustomCost(w, text World, res *resolution, prefix string, t techDecl, c *CustomCost, tier costTier, tgt noteTarget) (Value, bool) {
	countSetting := l.settings[c.Count.index-1]
	secondsSetting := l.settings[c.Seconds.index-1]
	packsSetting := l.settings[c.Packs.index-1]

	// EACH NUMBER IS HELD TO WHAT THE ENGINE TAKES WHERE IT IS READ, and one
	// that is not takes the setting's DECLARED DEFAULT with a line naming it,
	// which is also how it stops being in force. The rules differ by one value:
	// beside a tier, 0 is the number's reserved word and is legal, and every
	// other number still has to be one the engine would take.
	countRule, secondsRule := countFault, secondsFault
	if tier.has {
		countRule, secondsRule = deferrableCountFault, deferrableSecondsFault
	}
	count := res.costNumber(w, countSetting, prefix, countRule, tgt)
	seconds := res.costNumber(w, secondsSetting, prefix, secondsRule, tgt)
	parsed := l.resolveTextList(text, res, packsSetting, prefix, "", listPacks, tgt)

	countSet := count != countSetting.defNum
	secondsSet := seconds != secondsSetting.defNum
	if tier.has && !countSet && !secondsSet && parsed.isDefault {
		return Nil(), false
	}

	// THE PACKS, and the three sources in the order the rule names them: what
	// the player typed, then the tier's own ingredients, then the author's
	// declared list with its ladders walked and its drops logged.
	entries := parsed.entries
	var tried []string
	if parsed.isDefault {
		if tier.has {
			entries = tierPackEntries(tier.unit)
		} else {
			entries, tried = resolvePackLadders(w, res, tgt, t.name, packsSetting.defPacks)
		}
	} else if tier.has {
		// A TYPED PACK LIST IS WHAT THIS TECHNOLOGY IS PRICED IN, so everything
		// the tier arm said about the TIER'S packs is now about a price nothing
		// emits, and each piece of it is taken back here.
		//
		// THE TOOLTIP FIRST, IN ONE CALL. Everything the tier arm wrote into
		// this technology's note slot is about a price nothing emits now, so
		// the slot goes back to what it held before that arm ran. A snapshot
		// rather than a list of named retractions, because the tier arm can
		// leave a sentence this site cannot compose: its fallback's own pack
		// ladders can land twice on one name and CLAMP, that note takes the
		// slot, and a by-name retraction would find nothing and leave a
		// capped-amount tooltip on a technology whose emitted price is the
		// player's own list. See resolution.noteAt.
		res.restoreNote(tgt, tier.note)
		// AND THEN THE LINES, WHICH HAVE NO SLOT TO PUT BACK. A log line is a
		// stream, so each one is named: the packless line the CostBy fallback
		// logged when it lost every pack it declared a moment ago, and the
		// tier's own sentence for a source that lost every pack or carried a
		// list this library could not read. ALL of them are named because only
		// one can have been written, and retracting a line that was never
		// written matches nothing.
		//
		// THE DROP LINES STAY, because they are true: those packs really are
		// absent from this game, and the line says only that.
		res.retractPacklessLine(tgt)
		if tier.source != "" {
			res.retractLog(packlessSourceLine(t.name, tier.source))
			res.retractLog(unreadableSourceLine(t.name, tier.source))
		}
	}
	if !tier.has {
		// AND THE POST-CONDITION, on whatever the two reads settled on. After a
		// fallback the value IS the declared default, so the only world this can
		// still refuse is a plan whose declared default is itself outside what
		// the engine takes. That is an AUTHOR bug: validateSettings refuses it
		// at the settings stage, which the engine runs before the data stage, so
		// it reaches here only through a host test that calls PlanData on its
		// own. It refuses, because an author's declaration is not a player's
		// typing.
		//
		// BESIDE A TIER THERE IS NOTHING FOR IT TO ANSWER: a number that is in
		// force there has already cleared the same rule with 0 excluded, and a
		// number that is not in force is the tier's own, which belongs to
		// whoever declared that technology.
		if !res.refuseCostNumbers(countSetting.emittedName(prefix), secondsSetting.emittedName(prefix), count, seconds) {
			return refusedCost(count, seconds), true
		}
		// THE LADDER PATH AND ONLY THE LADDER PATH, which is the arm the Rust
		// mirror tests for by name. A list the PLAYER typed can never be empty
		// (the language refuses both none and an empty field in a pack list,
		// and testdata/ingredient-list/cases.txt pins both sentences), but the
		// two halves agreeing must not rest on that: this arm names the path it
		// is about, so a change to the language cannot silently make one half
		// price a technology at no science pack while the other does not. A
		// TIER'S OWN PACKS ARE NOT ASKED EITHER, because they are another
		// technology's declaration.
		if parsed.isDefault && len(entries) == 0 {
			res.packlessAt(tgt, t.name, tried)
		}
	}

	// THE NUMBERS THE LINE AND THE UNIT CARRY, which are the settings' where
	// they are in force and the tier's where they are not. A tier that carries
	// no count at all (a count_formula prices it instead) leaves the setting's
	// declared default standing in the line, and leaves the tier's own field
	// alone in the unit: the override below is what writes one.
	// COUNT BY FORMULA IS A PRICE WITH NO NUMBER IN IT, and the line says so
	// rather than printing one. A tier priced by count_formula carries no count
	// key at all, so a count setting that DEFERS leaves nothing for the line to
	// name: printing the setting's declared default there (0, beside a
	// dropdown) is a number nothing in the emitted unit is using. Both terms
	// are load-bearing. The formula test is what keeps the phrase true: a unit
	// with neither field is one the engine refuses outright (measured on
	// 2.0.77: `Key "count_formula" not found in property tree`), so it is
	// reachable only from a fixture World, and there the honest answer is the
	// number rather than a formula that is not there.
	byFormula := false
	if tier.has {
		if !countSet {
			if v, ok := tierNumber(tier.unit, "count"); ok {
				count = v
			} else {
				byFormula = hasUnitField(tier.unit, "count_formula")
			}
		}
		if !secondsSet {
			if v, ok := tierNumber(tier.unit, "time"); ok {
				seconds = v
			}
		}
	}

	countText := l.lang.amount(count)
	if byFormula {
		countText = "by formula"
	}
	// THE EMPTY LIST IS NOT SPELLED WITH THE WORD THE FIELD REFUSES. The
	// renderer's answer for an empty list is the language's reserved word, and
	// in an INGREDIENT field that word is legal and means "the mod's own
	// choice"; in a PACK field the language refuses it, and testdata's corpus
	// pins the sentence that does the refusing. Printing it here would be
	// inviting the player to paste back into the field the one text it will not
	// take. The renderer and the corpus are the language's contract and neither
	// moves for this: the substitution is the LINE'S, at the line's own
	// composer, and nothing reads it back.
	packsText := l.lang.render(entries)
	if len(entries) == 0 {
		packsText = "no science pack"
	}
	line := "fkrecipes: " + t.emittedName(prefix) + " takes its research cost from " +
		packsSetting.emittedName(prefix) + ": count " + countText +
		", time " + l.lang.amount(seconds) + ", packs " + packsText
	if tier.has {
		line += "; the " + tier.dropdown + " choice " + tier.chosen + " supplies what the settings leave at default"
	}

	packs := make([]Value, 0, len(entries))
	for _, e := range entries {
		packs = append(packs, Arr(Str(e.name), Num(float64(e.amount))))
	}
	if !tier.has {
		res.logs = append(res.logs, line)
		return Obj(
			kv("count", Num(count)),
			kv("time", Num(seconds)),
			kv("ingredients", Arr(packs...)),
		), true
	}

	// THE TIER'S UNIT WITH THE PLAYER'S FIELDS WRITTEN OVER IT, so a
	// count_formula, a max_level and anything else it carried survive a player
	// who moved one slider.
	unit := tier.unit
	if countSet {
		unit = setUnitField(unit, "count", Num(count))
		// ONE WRINKLE, AND THE ENGINE DECIDES IT: a unit carrying both a count
		// and a count_formula is priced by the FORMULA, so the number the
		// player typed would be read by nobody and nothing would say so. The
		// formula goes, and the line says which setting took it.
		if hasUnitField(unit, "count_formula") {
			unit = withoutUnitField(unit, "count_formula")
			res.logs = append(res.logs, "fkrecipes: "+t.name+": "+countSetting.emittedName(prefix)+
				" replaces the count_formula the "+tier.chosen+" cost carries")
		}
	}
	if secondsSet {
		unit = setUnitField(unit, "time", Num(seconds))
	}
	if !parsed.isDefault {
		unit = setUnitField(unit, "ingredients", Arr(packs...))
	}
	res.logs = append(res.logs, line)
	return unit, true
}

// tierNumber reads one numeric field out of a tier's unit map.
//
// A SLICE AND A SCAN, like every other lookup in this library: a unit is a
// handful of fields and nothing here may depend on an iteration order.
func tierNumber(unit Value, key string) (float64, bool) {
	if unit.Kind != KindMap {
		return 0, false
	}
	for _, e := range unit.Map {
		if e.Key == key && e.Val.Kind == KindNum {
			return e.Val.Num, true
		}
	}
	return 0, false
}

// hasUnitField reports whether a tier's unit carries a field at all, whatever
// its shape. count_formula is a STRING in every unit the engine ships, so the
// numeric reader above cannot answer this question.
func hasUnitField(unit Value, key string) bool {
	if unit.Kind != KindMap {
		return false
	}
	for _, e := range unit.Map {
		if e.Key == key {
			return true
		}
	}
	return false
}

// setUnitField replaces a field of a tier's unit IN PLACE IN THE ORDER IT
// ALREADY HAD, or appends it at the end when the unit does not carry one.
//
// THE ORDER IS PART OF THE EMITTED VALUE, so a field that moved would be a
// prototype that differs between a plan that overrode it and one that did not,
// and the two halves would have to agree about the move as well as about the
// value.
func setUnitField(unit Value, key string, val Value) Value {
	if unit.Kind != KindMap {
		return Obj(kv(key, val))
	}
	pairs := make([]KV, 0, len(unit.Map)+1)
	replaced := false
	for _, e := range unit.Map {
		if e.Key == key {
			pairs = append(pairs, kv(key, val))
			replaced = true
			continue
		}
		pairs = append(pairs, e)
	}
	if !replaced {
		pairs = append(pairs, kv(key, val))
	}
	return Obj(pairs...)
}

// withoutUnitField drops a field of a tier's unit, keeping the rest in order.
func withoutUnitField(unit Value, key string) Value {
	if unit.Kind != KindMap {
		return unit
	}
	pairs := make([]KV, 0, len(unit.Map))
	for _, e := range unit.Map {
		if e.Key == key {
			continue
		}
		pairs = append(pairs, e)
	}
	return Obj(pairs...)
}

// tierPackEntries is a tier unit's science packs in the shape the renderer
// takes, so a cost line can say what the tier is paying with.
//
// BOTH SPELLINGS, because a unit this library copies is somebody else's
// declaration: the engine takes the short tuple {"name", amount} and the long
// {name = ..., amount = ...} alike, and base writes the short one. An entry in
// neither shape is skipped rather than guessed at; it is the tier's own
// ingredients that are emitted, so nothing this reader misses changes the
// prototype, only the line that describes it.
func tierPackEntries(unit Value) ingredientList {
	out := ingredientList{}
	if unit.Kind != KindMap {
		return out
	}
	for _, e := range unit.Map {
		if e.Key != "ingredients" || e.Val.Kind != KindArr {
			continue
		}
		for _, item := range e.Val.Arr {
			if entry, ok := tierPackEntry(item); ok {
				out = append(out, entry)
			}
		}
	}
	return out
}

func tierPackEntry(v Value) (listEntry, bool) {
	if v.Kind == KindArr && len(v.Arr) >= 2 && v.Arr[0].Kind == KindStr && v.Arr[1].Kind == KindNum {
		return listEntry{name: v.Arr[0].Str, amount: int64(v.Arr[1].Num)}, true
	}
	if v.Kind != KindMap {
		return listEntry{}, false
	}
	name, amount := "", 0.0
	named, counted := false, false
	for _, e := range v.Map {
		if e.Key == "name" && e.Val.Kind == KindStr {
			name, named = e.Val.Str, true
		}
		if e.Key == "amount" && e.Val.Kind == KindNum {
			amount, counted = e.Val.Num, true
		}
	}
	if !named || !counted {
		return listEntry{}, false
	}
	return listEntry{name: name, amount: int64(amount)}, true
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
//
// A REPRICED RESEARCH DESTROYS NOTHING, so the note below carries no
// recipe-change sentence: see noteOn.
func (r *resolution) costNumber(w World, s settingDecl, prefix string, fault func(float64) numberFault, tgt noteTarget) float64 {
	full := s.emittedName(prefix)
	v, held := r.readNumber(w, s, prefix)
	if !held {
		return v
	}
	if f := fault(v); f != faultNone {
		r.noteFallback(tgt, full, numberFallback(storedNumberProblem(full, f)), false)
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
//
// A CRAFTING TIME DESTROYS NOTHING, which is why the note below asks for none
// of the recipe-change sentence: energy_required moves and the ingredient list
// this recipe emits is byte for byte what it would have been. The one fallback
// that empties an assembler is the one that changes the list itself.
func (r *resolution) craftTimeNumber(w World, s settingDecl, prefix, recipe string, tgt noteTarget) float64 {
	full := s.emittedName(prefix)
	v, held := r.readNumber(w, s, prefix)
	if !held {
		return v
	}
	if f := craftTimeFault(v); f != faultNone {
		r.noteFallback(tgt, full, numberFallback(storedCraftTimeProblem(recipe, full, f)), false)
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
