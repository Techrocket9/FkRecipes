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
		if len(ing.candidates) == 0 {
			out = append(out, listEntry{name: l.items[ing.item.index-1].emittedName(prefix), amount: ing.amount})
			continue
		}
		if ing.kind == kindFluid {
			out = append(out, listEntry{kind: kindFluid, name: ing.candidates[0], fluid: ing.fluidAmount})
			continue
		}
		out = append(out, listEntry{name: ing.candidates[0], amount: ing.amount})
	}
	return out
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
// Which recipe or technology reads which text setting.
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
		}
	}

	for _, t := range l.techs {
		who := "the technology " + t.name
		named := 0
		for _, set := range []bool{t.spec.CostOf != "", t.spec.Unit != nil, t.spec.CostBy != nil, t.spec.CostFrom != nil} {
			if set {
				named++
			}
		}
		// Exactly one cost source is the data planner's sentence, and it is the
		// one an author reads best; everything below assumes it held.
		if named != 1 {
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
		out[i] = Arr(
			Str(""),
			localeRef("mod-setting-description", s.emittedName(prefix)),
			Str("\ndefault: "+l.lang.render(entries)),
		)
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
				l.lang.render(l.declaredIngredientEntries(prefix, c.Ingredients))))
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
			params = append(params, presetLine(full, c.Value, costPresetText(c)))
		}
		out[i] = localisedGroup(params)
	}
	return out
}

// localeRef is a localised string that is nothing but a key: {"section.key"}.
func localeRef(section, key string) Value { return Arr(Str(section + "." + key)) }

// presetLine is one preset's line in a composed dropdown description.
//
// THE VALUE IS LABELLED BY ITS OWN LOCALE ENTRY, not by its raw key, because
// the settings screen shows the player the localised label and a description
// naming the key would not match anything they can see. That entry is the one
// the dropdown already needs for the value to be readable at all, so the
// composition adds no locale obligation of its own.
func presetLine(setting, value, rendering string) Value {
	return Arr(
		Str(""),
		Str("\n"),
		localeRef("string-mod-setting", setting+"-"+value),
		Str(": "+rendering),
	)
}

// costPresetText is what a research preset says it costs: the ladder's FIRST
// source, which is the technology whose unit would be copied. A ladder with no
// rungs at all falls back, and says so.
func costPresetText(c CostChoice) string {
	if len(c.Sources) == 0 {
		return "the fallback cost"
	}
	return "cost of " + c.Sources[0]
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
// THREE OUTCOMES, and the second is the one the whole design turns on:
//
//   - unreadable: the declared default applies, with the ordinary log line the
//     rest of the library uses for a setting it could not read;
//   - the word default: the AUTHOR's declared list with its ladders, which is
//     the pre-existing resolution path and gets no line of its own;
//   - anything else: parsed and resolved by the language, refused on the first
//     problem with the message the reference documents.
//
// A READABLE NON-STRING IS REFUSED rather than degraded to the default. The
// engine resets a wrong-typed stored value before any stage runs (measured), so
// this is unreachable through the settings screen and reachable only through a
// hand-edited file; a silent default there would hide a file somebody broke.
//
// THE WORLD HERE IS THE OVERLAY, planItemWorld, and every caller passes it: the
// language's resolver, its tag hint and its suggestion fold all have to see the
// items this plan is about to emit. Reading the setting itself goes through the
// overlay too, which delegates it.
func (l *Lib) resolveTextList(w World, res *resolution, s settingDecl, prefix, category string, kind listKind) (parsedList, bool) {
	full := s.emittedName(prefix)
	v, ok := w.StartupSetting(full)
	if !ok {
		res.logs = append(res.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
		return parsedList{isDefault: true}, true
	}
	if v.Kind != KindStr {
		res.refuse("fkrecipes: " + full + " is not text")
		return parsedList{}, false
	}
	parsed, problem := l.lang.parse(v.Str, kind, category, full, w)
	if problem != "" {
		// VERBATIM. The language already composed the whole sentence, naming
		// the setting, the entry and the problem, and it is the same sentence
		// the corpus pins in both languages.
		res.refuse(problem)
		return parsedList{}, false
	}
	return parsed, true
}

// resolveIngredientsFrom is a recipe whose ingredients the player writes.
//
// TWO WORLDS, AND THE DIFFERENCE IS DELIBERATE. A text the player typed is
// resolved against the overlay, because it may name this plan's own items; the
// DECLARED ladders are walked against the real World, because a ladder is the
// author's list of things other mods might provide and its rungs are answered
// exactly as they were before the overlay existed.
func (l *Lib) resolveIngredientsFrom(w, text World, res *resolution, prefix string, r recipeDecl, s settingDecl) []resolvedIngredient {
	parsed, ok := l.resolveTextList(text, res, s, prefix, r.spec.Category, listRecipe)
	if !ok {
		return nil
	}
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

	count := res.readNumber(w, countSetting, prefix)
	seconds := res.readNumber(w, secondsSetting, prefix)

	parsed, ok := l.resolveTextList(text, res, packsSetting, prefix, "", listPacks)
	if !ok {
		// The refusal is recorded; the value is never emitted.
		return refusedCost(count, seconds)
	}
	// THE TWO NUMBERS ARE CHECKED BEFORE ANYTHING IS BUILT OUT OF THEM. The
	// declared minima and the engine's own reset rule keep a player from
	// producing one of these, and a World is still an interface: a fixture that
	// answers a NaN used to reach the amount formatter, and one that answers an
	// infinity used to be rendered into the unit. They are refused by the name
	// of the SETTING that holds them, because that is the field somebody would
	// go and fix.
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
func (r *resolution) readNumber(w World, s settingDecl, prefix string) float64 {
	full := s.emittedName(prefix)
	if v, ok := w.StartupSetting(full); ok && v.Kind == KindNum {
		return v.Num
	}
	r.logs = append(r.logs, "fkrecipes: the setting "+full+" was not readable, so its default applies")
	return s.defNum
}

// refuseCostNumbers holds a research count and a research time a World answered
// to what the engine takes, and answers whether the cost may be built.
//
// THE ORDER IS FINITENESS FIRST, both numbers, and only then the two floors:
// a NaN is neither below 1 nor at or below zero, so a floor arm reached first
// would wave it through, and an infinity is above every floor there is.
//
// The sentences are the declared path's, with the SETTING in the place of the
// technology: the number came out of a field, and the technology did not
// declare it.
func (r *resolution) refuseCostNumbers(countName, secondsName string, count, seconds float64) bool {
	switch {
	case !finite(count):
		r.refuse("fkrecipes: " + countName + " holds a value that is not a finite number")
	case !finite(seconds):
		r.refuse("fkrecipes: " + secondsName + " holds a value that is not a finite number")
	case count < 1:
		r.refuse("fkrecipes: " + countName + " holds a research count below 1")
	case seconds <= 0:
		r.refuse("fkrecipes: " + secondsName + " holds a research time at or below zero")
	default:
		return true
	}
	return false
}

// refusedCost is the unit a refused custom cost hands back. It is never
// emitted: the refusal is recorded and PlanData stops the load before any op
// reaches the host. It exists so the two refusal arms answer in one shape.
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
