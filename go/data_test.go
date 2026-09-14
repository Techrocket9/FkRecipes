package fkrecipes

import (
	"math"
	"strings"
	"testing"
)

// A believable little mod: an axe head, a steel axe made from it, and the
// research that unlocks the recipe.

// steel-processing's unit as the WORLD hands it back: sorted by key, because
// fkdata sorts every dictionary at every level on the way out. A unit this
// library builds itself is pre-sort and keeps its authored order.
const steelProcessingUnit = `{count=50, ingredients=[["automation-science-pack", 1]], time=15}`

// The same, for the two technologies the split-emission test prices from.
const logistics2Unit = `{count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}`
const logistics3Unit = `{count=400, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], time=60}`

func logistics2UnitValue() Value {
	return unitOf(200, 30, "automation-science-pack", "logistic-science-pack")
}

func TestPlanDataItemAndRecipeShapes(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{
		Icon:        "__steelworks__/graphics/icons/steel-axe.png",
		IconSize:    64,
		StackSize:   20,
		Subgroup:    "tool",
		DisplayName: "Steel axe",
		Description: "Chops trees at twice the speed.",
	})
	head := lib.Item("axe-head", ItemSpec{Icon: "__steelworks__/graphics/icons/axe-head.png"})
	lib.Recipe(head, RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(2, "steel-plate", "iron-plate")},
	})
	lib.Recipe(axe, RecipeSpec{
		CraftTime:   2.5,
		Category:    "crafting",
		ResultCount: 2,
		Ingredients: []Ingredient{
			IngredientOf(head, 1),
			IngredientNamed(4, "steel-plate", "iron-plate"),
		},
		DisplayName: "Steel axe",
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", localised_name=["", "Steel axe"], localised_description=["", "Chops trees at twice the speed."], icon="__steelworks__/graphics/icons/steel-axe.png", icon_size=64, stack_size=20, subgroup="tool"}`,
		`extend {type="item", name="steelworks-axe-head", icon="__steelworks__/graphics/icons/axe-head.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-axe-head", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-axe-head", amount=1}]}`,
		`extend {type="recipe", name="steelworks-steel-axe", localised_name=["", "Steel axe"], category="crafting", energy_required=2.5000000000000000e0, enabled=true, ingredients=[{type="item", name="steelworks-axe-head", amount=1}, {type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=2}]}`,
	})
}

// The ladder resolves to the first candidate the game actually has, and drops
// the ingredient when it has none of them. Never a guess: a name the game
// does not have is a hard load failure naming the consumer's mod.
func TestIngredientLadderFallsBackAndDrops(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{Icon: "__steelworks__/graphics/icons/steel-axe.png"})
	lib.Recipe(axe, RecipeSpec{
		Ingredients: []Ingredient{
			IngredientNamed(4, "steel-plate", "iron-plate"),
			IngredientNamed(1, "tungsten-plate", "titanium-plate"),
		},
	})

	// The world has iron but no steel, so the ladder takes its second rung.
	ops, err := lib.PlanData(baseWorld().withoutItem("steel-plate"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axe: none of tungsten-plate, titanium-plate is present, so the ingredient is dropped`,
		`extend {type="item", name="steelworks-steel-axe", icon="__steelworks__/graphics/icons/steel-axe.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="iron-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// TWO LADDERS CAN LAND ON ONE RUNG, and what comes out names it once.
//
// MEASURED, on the pilot's own recipe: an ingredient list that names one item
// twice refuses the WHOLE load with "Error while running setup for recipe
// prototype "bbb-balancer-part" (recipe): Duplicate item ingredients are not
// allowed (iron-plate exists 2 or more times)", exit 1, no dump, and no line
// naming a setting or a missing item. Its ladders all end on iron-plate, so a
// mod set without transport-belt is a player who never opened the settings and
// cannot load the game.
//
// THE MERGED ENTRY KEEPS THE FIRST OCCURRENCE'S POSITION, which is why
// iron-gear-wheel is still second here: a merge that moved the line would be
// the declaration order changing under a mod set, and order is host-visible.
func TestIngredientLaddersThatLandOnOneNameMerge(t *testing.T) {
	lib := New()
	part := lib.Item("balancer-part", ItemSpec{})
	lib.Recipe(part, RecipeSpec{
		Ingredients: []Ingredient{
			IngredientNamed(4, "iron-plate"),
			IngredientNamed(2, "iron-gear-wheel"),
			IngredientNamed(2, "transport-belt", "iron-plate"),
		},
	})

	// The world has no transport-belt, so the third ladder takes a rung the
	// list already carries.
	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: balancer-part: iron-plate is in the list twice after the fallbacks, so the amounts are added: 4 plus 2 is 6`,
		`extend {type="item", name="steelworks-balancer-part", stack_size=50}`,
		`extend {type="recipe", name="steelworks-balancer-part", enabled=true, ingredients=[{type="item", name="iron-plate", amount=6}, {type="item", name="iron-gear-wheel", amount=2}], results=[{type="item", name="steelworks-balancer-part", amount=1}]}`,
	})
}

// A FLUID LADDER MERGES THE SAME WAY, and an item and a fluid of one name do
// NOT: the kind is part of the identity. MEASURED: an item and a fluid of the
// same name in one recipe load (base carries parameter-0 to parameter-9 as
// both), so merging them would be adding two different things.
//
// THE FLUID LINE NAMES THE RUNG WHERE THE ITEM LINE NAMES THE NUMBERS, because
// rendering a fluid amount is the language's job and this path may not link
// it. Without the rung a three-way collapse writes one line twice and names
// nothing the author can go and change.
//
// BOTH ORDERS OF THE MISMATCHED PAIR, item then fluid and fluid then item: the
// guard is a comparison over two kinds, and a test that only ever puts them
// one way round leaves the other arm free to answer.
func TestFluidLaddersMergeAndDoNotMergeWithAnItem(t *testing.T) {
	lib := New()
	mix := lib.Item("sulfuric-mix", ItemSpec{})
	lib.Recipe(mix, RecipeSpec{
		Category: "chemistry",
		Ingredients: []Ingredient{
			FluidIngredient(0.5, "water"),
			IngredientNamed(2, "iron-plate"),
			FluidIngredient(1.5, "steam", "water"),
		},
	})

	ops, err := lib.PlanData(baseWorld().withoutFluid("steam"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: sulfuric-mix: water is in the list twice after the fallbacks, so the amounts are added; the ladder from steam resolved onto it`,
		`extend {type="item", name="steelworks-sulfuric-mix", stack_size=50}`,
		`extend {type="recipe", name="steelworks-sulfuric-mix", category="chemistry", enabled=true, ingredients=[{type="fluid", name="water", amount=2}, {type="item", name="iron-plate", amount=2}], results=[{type="item", name="steelworks-sulfuric-mix", amount=1}]}`,
	})

	// THREE LADDERS ONTO ONE FLUID, and the two lines it writes are different
	// lines: each names the ladder that collapsed, which is the declaration
	// the author edits.
	three := New()
	mix3 := three.Item("sulfuric-mix", ItemSpec{})
	three.Recipe(mix3, RecipeSpec{
		Category: "chemistry",
		Ingredients: []Ingredient{
			FluidIngredient(0.5, "water"),
			FluidIngredient(1.5, "steam", "water"),
			FluidIngredient(2, "lubricant", "water"),
		},
	})

	// steam is taken away and the fixture has no lubricant at all, so both
	// ladders fall through to water.
	ops, err = three.PlanData(baseWorld().withoutFluid("steam"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: sulfuric-mix: water is in the list twice after the fallbacks, so the amounts are added; the ladder from steam resolved onto it`,
		`log fkrecipes: sulfuric-mix: water is in the list twice after the fallbacks, so the amounts are added; the ladder from lubricant resolved onto it`,
		`extend {type="item", name="steelworks-sulfuric-mix", stack_size=50}`,
		`extend {type="recipe", name="steelworks-sulfuric-mix", category="chemistry", enabled=true, ingredients=[{type="fluid", name="water", amount=4}], results=[{type="item", name="steelworks-sulfuric-mix", amount=1}]}`,
	})

	// The same name twice, once in each namespace. Two entries come out, and
	// no line says anything was added.
	both := New()
	mix2 := both.Item("sulfuric-mix", ItemSpec{})
	both.Recipe(mix2, RecipeSpec{
		Category: "chemistry",
		Ingredients: []Ingredient{
			IngredientNamed(2, "water"),
			FluidIngredient(0.5, "water"),
		},
	})

	ops, err = both.PlanData(baseWorld().withItem("water"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-sulfuric-mix", stack_size=50}`,
		`extend {type="recipe", name="steelworks-sulfuric-mix", category="chemistry", enabled=true, ingredients=[{type="item", name="water", amount=2}, {type="fluid", name="water", amount=5.0000000000000000e-1}], results=[{type="item", name="steelworks-sulfuric-mix", amount=1}]}`,
	})

	// AND THE OTHER ORDER, fluid first. The kind guard is a comparison, not a
	// preference: an item arriving at a fluid of the same name has to fall
	// through exactly as a fluid arriving at an item does.
	rev := New()
	mix4 := rev.Item("sulfuric-mix", ItemSpec{})
	rev.Recipe(mix4, RecipeSpec{
		Category: "chemistry",
		Ingredients: []Ingredient{
			FluidIngredient(0.5, "water"),
			IngredientNamed(2, "water"),
		},
	})

	ops, err = rev.PlanData(baseWorld().withItem("water"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-sulfuric-mix", stack_size=50}`,
		`extend {type="recipe", name="steelworks-sulfuric-mix", category="chemistry", enabled=true, ingredients=[{type="fluid", name="water", amount=5.0000000000000000e-1}, {type="item", name="water", amount=2}], results=[{type="item", name="steelworks-sulfuric-mix", amount=1}]}`,
	})
}

// THE MERGE IS THE ONE PLACE A CEILING IS RE-ASKED AFTER RESOLUTION. Both
// declarations are legal apart, the plan validates, and the sum is a number no
// author wrote: 40000 and 30000 are each under the engine's 65535 and 70000 is
// not.
//
// AND IT CLAMPS RATHER THAN REFUSING, because WHICH RUNGS the ladders landed on
// is a fact about the mod set and not about the declaration. The line says what
// the number became and the recipe's own tooltip carries the note, with the
// destruction sentence on it: the ingredient list is what moved.
func TestMergedItemAmountAboveTheCeilingIsClamped(t *testing.T) {
	lib := New()
	part := lib.Item("balancer-part", ItemSpec{})
	lib.Recipe(part, RecipeSpec{
		Ingredients: []Ingredient{
			IngredientNamed(40000, "iron-plate"),
			IngredientNamed(30000, "transport-belt", "iron-plate"),
		},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: balancer-part: iron-plate is in the list twice after the fallbacks, so the amounts are added: 40000 plus 30000 is 70000`,
		`log fkrecipes: balancer-part: iron-plate is in the list twice after the fallbacks, and 40000 plus 30000 is above the item ceiling of 65535, so it is capped there`,
		`extend {type="item", name="steelworks-balancer-part", stack_size=50}`,
		`extend {type="recipe", name="steelworks-balancer-part", ` +
			`localised_description=["", ` + chunkedParams(
			`Two ingredients resolved onto iron-plate and the total was above what one slot holds, so it was capped at 65535. The reason is in the log. `+
				`Changing a recipe empties an assembling machine's input slots of anything the new list does not use.`) + `], ` +
			`enabled=true, ingredients=[{type="item", name="iron-plate", amount=65535}], ` +
			`results=[{type="item", name="steelworks-balancer-part", amount=1}]}`,
	})
}

// The fluid twin. Above its ceiling the engine does not refuse, it ABORTS
// inside FixedPointNumber and hands the player the crash handler, which is why
// a merged fluid amount may not be emitted as it stands; the cap is what keeps
// the load standing without one.
func TestMergedFluidAmountAboveTheCeilingIsClamped(t *testing.T) {
	lib := New()
	mix := lib.Item("sulfuric-mix", ItemSpec{})
	lib.Recipe(mix, RecipeSpec{
		Category: "chemistry",
		Ingredients: []Ingredient{
			FluidIngredient(5e300, "water"),
			FluidIngredient(6e300, "steam", "water"),
		},
	})

	ops, err := lib.PlanData(baseWorld().withoutFluid("steam"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: sulfuric-mix: water is in the list twice after the fallbacks, so the amounts are added; the ladder from steam resolved onto it`,
		`log fkrecipes: sulfuric-mix: water is in the list twice after the fallbacks, and the added amount is above the fluid ceiling of 1e301, so it is capped there; the ladder from steam resolved onto it`,
		`extend {type="item", name="steelworks-sulfuric-mix", stack_size=50}`,
		`extend {type="recipe", name="steelworks-sulfuric-mix", ` +
			`localised_description=["", ` + chunkedParams(
			`Two ingredients resolved onto water and the total was above the largest amount the game can hold, so it was capped at 1e301. The reason is in the log. `+
				`Changing a recipe empties an assembling machine's input slots of anything the new list does not use.`) + `], ` +
			`category="chemistry", enabled=true, ingredients=[{type="fluid", name="water", amount=1.0000000000000001e301}], ` +
			`results=[{type="item", name="steelworks-sulfuric-mix", amount=1}]}`,
	})
}

// THE BOUNDARY, THREE TIMES, because a ceiling written with the wrong
// comparison refuses a legal plan and fails the whole load: an item merge, a
// pack merge and a fluid merge landing EXACTLY on their ceilings are accepted
// and emitted with the number they landed on.
func TestAMergeLandingExactlyOnItsCeilingIsAccepted(t *testing.T) {
	lib := New()
	part := lib.Item("balancer-part", ItemSpec{})
	lib.Recipe(part, RecipeSpec{
		Ingredients: []Ingredient{
			IngredientNamed(65534, "iron-plate"),
			IngredientNamed(1, "transport-belt", "iron-plate"),
		},
	})
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs: []Pack{
			{Name: "automation-science-pack", Amount: 65000},
			{Name: "military-science-pack", Amount: 535, Fallbacks: []string{"automation-science-pack"}},
		},
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: balancer-part: iron-plate is in the list twice after the fallbacks, so the amounts are added: 65534 plus 1 is 65535`,
		`log fkrecipes: steel-axes: automation-science-pack is in the list twice after the fallbacks, so the amounts are added: 65000 plus 535 is 65535`,
		`extend {type="item", name="steelworks-balancer-part", stack_size=50}`,
		`extend {type="recipe", name="steelworks-balancer-part", enabled=true, ingredients=[{type="item", name="iron-plate", amount=65535}], results=[{type="item", name="steelworks-balancer-part", amount=1}]}`,
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 65535]]}}`,
	})

	// The fluid twin, landing on maxFluidAmount exactly. Its halves are chosen
	// so the double addition is exact: 1e301 = 5e300 + 5e300.
	mixLib := New()
	mix := mixLib.Item("sulfuric-mix", ItemSpec{})
	mixLib.Recipe(mix, RecipeSpec{
		Category: "chemistry",
		Ingredients: []Ingredient{
			FluidIngredient(5e300, "water"),
			FluidIngredient(5e300, "steam", "water"),
		},
	})

	ops, err = mixLib.PlanData(baseWorld().withoutFluid("steam"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: sulfuric-mix: water is in the list twice after the fallbacks, so the amounts are added; the ladder from steam resolved onto it`,
		`extend {type="item", name="steelworks-sulfuric-mix", stack_size=50}`,
		`extend {type="recipe", name="steelworks-sulfuric-mix", category="chemistry", enabled=true, ingredients=[{type="fluid", name="water", amount=1.0000000000000001e301}], results=[{type="item", name="steelworks-sulfuric-mix", amount=1}]}`,
	})
}

// ---------------------------------------------------------------------------
// A recipe whose resolved list names its own product.
// ---------------------------------------------------------------------------

// selfProductWant is the line under test, built the way the tests below read
// it: one string, so a wording change is one edit here and one in data.go
// rather than a sweep over eight literals that could drift apart.
func selfProductWant(subject, name string) string {
	return "log fkrecipes: " + subject + ": " + name +
		" is in the list and is also what this recipe makes," +
		" so nothing can craft the first one unless something else produces it"
}

// A LIST THAT NAMES THE RECIPE'S OWN PRODUCT IS ACCEPTED AND SAID OUT LOUD.
//
// MEASURED on 2.0.77 (build 84539, mac-arm64, steam), base alone under a
// private user directory: kovarex-enrichment-process takes 40 uranium-235 and
// 5 uranium-238 and gives back 41 uranium-235 and 2 uranium-238, and
// coal-liquefaction takes 25 heavy-oil and gives back 90. A sweep of the same
// dump for recipes naming one type and name in both ingredients and results
// returns exactly those two of base's 217 recipes. So the shape is legal, a
// library that refused it would be wrong, and what was missing was the signal.
//
// FOUR ARMS ADD A RESOLVED LIST AND ALL FOUR ARE HERE. A player's text with no
// dropdown beside it, a player's text that takes a dropdown's choice over, an
// author's preset behind a dropdown, and a plain declared list: the check sits
// in resolution.addRecipe, which every one of them hands its list to, and this
// is the witness that none of them goes round it.
//
// THE SUBJECT IS THE DECLARED NAME, which the first arm shows twice over: the
// same transcript carries the "takes its ingredients from" line, and that one
// names the EMITTED recipe. Two lines about one recipe, two different names,
// each the one its own sentence has always used.
func TestARecipeWhoseListNamesItsOwnProductSaysSo(t *testing.T) {
	// IngredientsFrom: the whole list is the player's, and they typed the
	// product. The overlay is why they could: the text world knows the names
	// this plan is about to emit.
	from := New()
	axe := from.Item("steel-axe", ItemSpec{})
	parts := from.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
	from.Recipe(axe, RecipeSpec{IngredientsFrom: parts})

	ops, err := from.PlanData(baseWorld().
		withSetting("steelworks-axe-ingredients", Str("1 steelworks-steel-axe, 2 iron-plate")))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-steel-axe takes its ingredients from steelworks-axe-ingredients: 1 steelworks-steel-axe, 2 iron-plate`,
		selfProductWant("steel-axe", "steelworks-steel-axe"),
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steelworks-steel-axe", amount=1}, {type="item", name="iron-plate", amount=2}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})

	// The same text path with a dropdown beside it, taken over by the text.
	custom := New()
	plate := custom.Item("hardened-steel-plate", ItemSpec{})
	style := custom.DropdownSettingNeedingLocale("style", "plain", []string{"plain"})
	quench := custom.IngredientsSetting("quench-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	custom.Recipe(plate, RecipeSpec{
		IngredientsBy: &IngredientChoices{
			Setting: style,
			Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}}},
		},
		IngredientsFrom: quench,
	})

	ops, err = custom.PlanData(baseWorld().
		withSetting("steelworks-style", Str("plain")).
		withSetting("steelworks-quench-ingredients", Str("3 steelworks-hardened-steel-plate")))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-steel-plate takes its ingredients from steelworks-quench-ingredients:` +
			` 3 steelworks-hardened-steel-plate; the steelworks-style choice plain is set aside`,
		selfProductWant("hardened-steel-plate", "steelworks-hardened-steel-plate"),
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steelworks-hardened-steel-plate", amount=3}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})

	// The dropdown on a PRESET, which is the author's own declaration and not
	// the player's typing: nobody typed anything here.
	preset := New()
	plate2 := preset.Item("hardened-steel-plate", ItemSpec{})
	grade := preset.DropdownSettingNeedingLocale("grade", "plain", []string{"plain", "recycled"})
	preset.Recipe(plate2, RecipeSpec{IngredientsBy: &IngredientChoices{
		Setting: grade,
		Choices: []IngredientChoice{
			{Value: "plain", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
			{Value: "recycled", Ingredients: []Ingredient{IngredientOf(plate2, 1), IngredientNamed(1, "steel-plate")}},
		},
	}})

	ops, err = preset.PlanData(baseWorld().withSetting("steelworks-grade", Str("recycled")))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		selfProductWant("hardened-steel-plate", "steelworks-hardened-steel-plate"),
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steelworks-hardened-steel-plate", amount=1}, {type="item", name="steel-plate", amount=1}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})

	// The plain declared list, with no setting anywhere near it.
	plain := New()
	part := plain.Item("balancer-part", ItemSpec{})
	plain.Recipe(part, RecipeSpec{Ingredients: []Ingredient{
		IngredientNamed(2, "iron-plate"),
		IngredientOf(part, 1),
	}})

	ops, err = plain.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		selfProductWant("balancer-part", "steelworks-balancer-part"),
		`extend {type="item", name="steelworks-balancer-part", stack_size=50}`,
		`extend {type="recipe", name="steelworks-balancer-part", enabled=true, ingredients=[{type="item", name="iron-plate", amount=2}, {type="item", name="steelworks-balancer-part", amount=1}], results=[{type="item", name="steelworks-balancer-part", amount=1}]}`,
	})
}

// THE PRODUCT HAS TWO DECLARATION SHAPES AND BOTH ARE COMPARED. A handle names
// an item this plan emits, so the product is that item's EMITTED name; a
// ResultNamed names somebody else's item verbatim. The arm above covers the
// handle; this is the other one, and without it a check reading only the
// handle would be green.
func TestAResultNamedProductInTheListSaysSoToo(t *testing.T) {
	lib := New()
	lib.Recipe(ItemRef{}, RecipeSpec{
		Name:        "steel-refining",
		ResultCount: 2,
		Ingredients: []Ingredient{
			IngredientNamed(1, "steel-plate"),
			IngredientNamed(3, "iron-plate"),
		},
		ResultNamed: "steel-plate",
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		selfProductWant("steel-refining", "steel-plate"),
		`extend {type="recipe", name="steelworks-steel-refining", enabled=true, ingredients=[{type="item", name="steel-plate", amount=1}, {type="item", name="iron-plate", amount=3}], results=[{type="item", name="steel-plate", amount=2}]}`,
	})
}

// THE KIND IS PART OF THE IDENTITY HERE EXACTLY AS IT IS IN A MERGE. A product
// is always an item: recipeProto writes type="item" and nothing chooses
// otherwise. An INGREDIENT can be a fluid, and an item and a fluid of one name
// genuinely coexist (base carries parameter-0 to parameter-9 as both), so a
// fluid sharing the product's name is a different thing with the same label
// and says nothing.
//
// ONE RECIPE, BOTH KINDS, ONE LINE, and this sub-case is the WITNESS the kind
// test has: with the kind dropped from the comparison both entries match by
// name and the line comes out twice, which is exactly what addRecipe's missing
// early exit lets it do. The fixture answers yes to `water` as an item and as a
// fluid at once. Base does not ship a `water` item, it ships the fluid; the
// coexistence shape base actually ships is parameter-0 to parameter-9, cited
// two lines above, and `water` is only the convenient name to write it with.
func TestAFluidNamedLikeTheProductSaysNothing(t *testing.T) {
	// The fluid alone: no line at all.
	fluid := New()
	fluid.Recipe(ItemRef{}, RecipeSpec{
		Name:        "quenching",
		Category:    "chemistry",
		ResultNamed: "water",
		Ingredients: []Ingredient{FluidIngredient(10, "water")},
	})

	ops, err := fluid.PlanData(baseWorld().withItem("water"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="recipe", name="steelworks-quenching", category="chemistry", enabled=true, ingredients=[{type="fluid", name="water", amount=10}], results=[{type="item", name="water", amount=1}]}`,
	})

	// Both kinds of water in one list. The ITEM fires and the FLUID does not,
	// so exactly one line comes out.
	both := New()
	both.Recipe(ItemRef{}, RecipeSpec{
		Name:        "quenching",
		Category:    "chemistry",
		ResultNamed: "water",
		Ingredients: []Ingredient{
			FluidIngredient(10, "water"),
			IngredientNamed(2, "water"),
		},
	})

	ops, err = both.PlanData(baseWorld().withItem("water"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		selfProductWant("quenching", "water"),
		`extend {type="recipe", name="steelworks-quenching", category="chemistry", enabled=true, ingredients=[{type="fluid", name="water", amount=10}, {type="item", name="water", amount=2}], results=[{type="item", name="water", amount=1}]}`,
	})
}

// ANTI-VACUITY. A check that fires on every recipe would pass every test
// above; this is the one that says an ordinary recipe is silent, and the plan
// under it is deliberately the shape a mod actually ships: a product made of
// things that are not it.
func TestAnOrdinaryRecipeSaysNothingAboutItsProduct(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	head := lib.Item("axe-head", ItemSpec{})
	lib.Recipe(head, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}})
	lib.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{
		IngredientOf(head, 1),
		IngredientNamed(4, "steel-plate"),
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="item", name="steelworks-axe-head", stack_size=50}`,
		`extend {type="recipe", name="steelworks-axe-head", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-axe-head", amount=1}]}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steelworks-axe-head", amount=1}, {type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// THE POSITION IN THE STREAM IS CONTRACTUAL, and it is a position on both
// sides. The line is evaluated on the FINAL list, so it comes after every merge
// and drop line its own recipe wrote; the stream is recipes in declaration
// order, so it comes before the next recipe's first line.
//
// THE FIRST RECIPE WRITES BOTH KINDS OF LINE AND THE SECOND WRITES A DROP, so
// a check placed one step too early or one recipe too late shows up as an order
// failure rather than as a missing line.
func TestTheSelfProductLineSitsAfterItsOwnRecipeAndBeforeTheNext(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	// Two ladders land on the product, so this recipe writes a merge line AND
	// the self-product line, in that order. The second rung is reached
	// because the fixture has no transport-belt.
	lib.Recipe(ItemRef{}, RecipeSpec{
		Name:        "steel-refining",
		ResultNamed: "steel-plate",
		Ingredients: []Ingredient{
			IngredientNamed(4, "steel-plate"),
			IngredientNamed(2, "transport-belt", "steel-plate"),
		},
	})
	// The second recipe drops an ingredient, which is the line that must come
	// after both of the first recipe's.
	lib.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{
		IngredientNamed(1, "tungsten-plate", "titanium-plate"),
		IngredientNamed(3, "steel-plate"),
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-refining: steel-plate is in the list twice after the fallbacks, so the amounts are added: 4 plus 2 is 6`,
		selfProductWant("steel-refining", "steel-plate"),
		`log fkrecipes: steel-axe: none of tungsten-plate, titanium-plate is present, so the ingredient is dropped`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-refining", enabled=true, ingredients=[{type="item", name="steel-plate", amount=6}], results=[{type="item", name="steel-plate", amount=1}]}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steel-plate", amount=3}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// THE ONE ARM THAT RESOLVES A LIST TWICE, and the line has to land after BOTH
// resolutions rather than after the first. A dropdown on a preset whose chosen
// plan names nothing this game has falls back to the DEFAULT option's plan,
// which is a second resolveIngredients over the same recipe; only the second
// list is the one the recipe is emitted with, so only the second list is the
// one the product is compared against.
//
// THREE LINES IN ONE ORDER, and the whole stream is compared rather than
// searched: the chosen plan's drop line, then the fallback line, then the
// self-product line the default plan earned. A check that ran on the chosen
// plan would put its line first or would say nothing at all.
func TestThePresetFallbackResolvesTwiceAndTheLineFollowsTheSecond(t *testing.T) {
	lib := New()
	part := lib.Item("balancer-part", ItemSpec{})
	grade := lib.DropdownSettingNeedingLocale("grade", "plain", []string{"plain", "exotic"})
	lib.Recipe(part, RecipeSpec{IngredientsBy: &IngredientChoices{
		Setting: grade,
		Choices: []IngredientChoice{
			// The DEFAULT plan is the one that names the product.
			{Value: "plain", Ingredients: []Ingredient{IngredientOf(part, 1), IngredientNamed(2, "iron-plate")}},
			// The CHOSEN plan names nothing the fixture has, so it resolves
			// to an empty list and the default applies instead.
			{Value: "exotic", Ingredients: []Ingredient{IngredientNamed(1, "tungsten-plate")}},
		},
	}})

	ops, err := lib.PlanData(baseWorld().withSetting("steelworks-grade", Str("exotic")))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: balancer-part: none of tungsten-plate is present, so the ingredient is dropped`,
		`log fkrecipes: balancer-part: the exotic ingredients name nothing this game has, so the plain ingredients apply`,
		selfProductWant("balancer-part", "steelworks-balancer-part"),
		`extend {type="item", name="steelworks-balancer-part", stack_size=50}`,
		`extend {type="recipe", name="steelworks-balancer-part", enabled=true, ingredients=[{type="item", name="steelworks-balancer-part", amount=1}, {type="item", name="iron-plate", amount=2}], results=[{type="item", name="steelworks-balancer-part", amount=1}]}`,
	})
}

// A DUPLICATE THE AUTHOR WROTE OUT IS A DIFFERENT PROBLEM FROM A LADDER THAT
// COLLAPSED, and it is refused rather than added up. MEASURED, and it is why:
// a dropdown preset declared `4 iron-plate, 2 iron-plate` composed a
// description reading ": 4 iron-plate, 2 iron-plate", which the player's own
// custom field refuses with "entries 1 and 2 both name iron-plate", while the
// recipe that reached the game quietly said 6.
//
// THE PLAIN LIST AND THE PRESET BOTH, because a preset is validated in two
// places (the binding validator, which both planners run, and the data
// planner's own loop) and a plain list in one.
func TestADeclaredListNamingOneThingTwiceIsRefused(t *testing.T) {
	plain := New()
	part := plain.Item("balancer-part", ItemSpec{})
	plain.Recipe(part, RecipeSpec{
		Ingredients: []Ingredient{
			IngredientNamed(4, "iron-plate"),
			IngredientNamed(2, "iron-plate"),
		},
	})

	_, err := plain.PlanData(baseWorld())
	if err == nil {
		t.Fatal("a plain list naming one item twice was accepted")
	}
	want := "fkrecipes: the recipe balancer-part names iron-plate twice; each ingredient is taken once"
	if err.Error() != want {
		t.Errorf("plain list\n got: %s\nwant: %s", err.Error(), want)
	}

	// A FLUID IS ITS OWN NAMESPACE HERE TOO: the same two entries as items and
	// as fluids are two refusals, and one of each is none.
	fluids := New()
	mix := fluids.Item("sulfuric-mix", ItemSpec{})
	fluids.Recipe(mix, RecipeSpec{
		Category: "chemistry",
		Ingredients: []Ingredient{
			FluidIngredient(0.5, "water"),
			FluidIngredient(1.5, "water"),
		},
	})

	_, err = fluids.PlanData(baseWorld())
	if err == nil {
		t.Fatal("a plain list naming one fluid twice was accepted")
	}
	want = "fkrecipes: the recipe sulfuric-mix names water twice; each ingredient is taken once"
	if err.Error() != want {
		t.Errorf("fluid list\n got: %s\nwant: %s", err.Error(), want)
	}

	// A PRESET, AT BOTH OF ITS CHECKS. The data planner's own recipe loop owns
	// a dropdown with no text setting beside it; the binding validator, which
	// BOTH planners run, owns one that has one, because the settings stage
	// renders those presets into the dropdown's description.
	dup := []Ingredient{IngredientNamed(4, "iron-plate"), IngredientNamed(2, "iron-plate")}
	want = "fkrecipes: the recipe balancer-part names iron-plate twice; each ingredient is taken once"

	bare := New()
	bp := bare.Item("balancer-part", ItemSpec{})
	bareStyle := bare.DropdownSettingNeedingLocale("style", "vanilla", []string{"vanilla"})
	bare.Recipe(bp, RecipeSpec{IngredientsBy: &IngredientChoices{
		Setting: bareStyle,
		Choices: []IngredientChoice{{Value: "vanilla", Ingredients: dup}},
	}})

	_, err = bare.PlanData(baseWorld())
	if err == nil {
		t.Fatal("a preset naming one item twice was accepted by the data planner")
	}
	if err.Error() != want {
		t.Errorf("preset, data plan\n got: %s\nwant: %s", err.Error(), want)
	}

	armed := func() *Lib {
		l := New()
		p := l.Item("balancer-part", ItemSpec{})
		style := l.DropdownSettingNeedingLocale("style", "vanilla", []string{"vanilla"})
		parts := l.IngredientsSetting("part-ingredients", []Ingredient{IngredientNamed(1, "iron-plate")})
		l.Recipe(p, RecipeSpec{
			IngredientsBy: &IngredientChoices{
				Setting: style,
				Choices: []IngredientChoice{{Value: "vanilla", Ingredients: dup}},
			},
			IngredientsFrom: parts,
		})
		return l
	}

	_, err = armed().PlanData(baseWorld())
	if err == nil {
		t.Fatal("an armed preset naming one item twice was accepted by the data planner")
	}
	if err.Error() != want {
		t.Errorf("armed preset, data plan\n got: %s\nwant: %s", err.Error(), want)
	}

	_, err = armed().PlanSettings(settingsWorld())
	if err == nil {
		t.Fatal("an armed preset naming one item twice was accepted by the settings planner")
	}
	if err.Error() != want {
		t.Errorf("armed preset, settings plan\n got: %s\nwant: %s", err.Error(), want)
	}
}

// CostOf copies the named technology's whole unit VERBATIM: a count_formula
// is a string, so an infinite technology's cost comes along with no evaluator
// and no key of it rewritten or reordered.
func TestTechnologyCostOfCopiesUnitVerbatim(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{Icon: "__steelworks__/graphics/icons/steel-axe.png"})
	rec := lib.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(4, "steel-plate")}})
	lib.Technology("steel-axes", TechSpec{
		Icon:        "__steelworks__/graphics/technology/steel-axes.png",
		IconSize:    128,
		CostOf:      "mining-productivity-4",
		After:       "steel-processing",
		Unlocks:     []RecipeRef{rec},
		DisplayName: "Steel axes",
		Description: "Sharper edges, fewer swings.",
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", icon="__steelworks__/graphics/icons/steel-axe.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=false, ingredients=[{type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
		`extend {type="technology", name="steelworks-steel-axes", localised_name=["", "Steel axes"], localised_description=["", "Sharper edges, fewer swings."], icon="__steelworks__/graphics/technology/steel-axes.png", icon_size=128, prerequisites=["steel-processing"], unit={count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60}, max_level="infinite", effects=[{type="unlock-recipe", recipe="steelworks-steel-axe"}]}`,
	})
}

// The level cap is a TECHNOLOGY field, not a unit field, so the verbatim unit
// copy cannot carry it: CostOf reads it separately, or an infinite source
// would produce a one-level copy.
func TestTechnologyCostOfCarriesMaxLevel(t *testing.T) {
	const miningUnit = `{count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60}`

	cases := []struct {
		name   string
		costOf string
		world  func(*fixtureWorld) *fixtureWorld
		want   string
	}{
		{
			name:   "an infinite source keeps its infinity",
			costOf: "mining-productivity-4",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want:   `extend {type="technology", name="steelworks-steel-axes", unit=` + miningUnit + `, max_level="infinite"}`,
		},
		{
			// An overhaul that caps the endless research at a finite level.
			name:   "a numeric cap comes across as a number",
			costOf: "mining-productivity-4",
			world:  func(w *fixtureWorld) *fixtureWorld { return w.withMaxLevel("mining-productivity-4", Num(20)) },
			want:   `extend {type="technology", name="steelworks-steel-axes", unit=` + miningUnit + `, max_level=20}`,
		},
		{
			name:   "a single-level source gets no cap at all",
			costOf: "steel-processing",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want:   `extend {type="technology", name="steelworks-steel-axes", unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}`,
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			lib.Technology("steel-axes", TechSpec{CostOf: c.costOf})

			ops, err := lib.PlanData(c.world(baseWorld()))
			assertNoError(t, err)

			assertLines(t, transcript(ops), []string{c.want})
		})
	}
}

// A hand-rolled unit has no source technology to read a cap from, so the
// escape hatch never emits max_level.
func TestTechnologyUnitSpecCarriesNoMaxLevel(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{
		Unit: &UnitSpec{Count: 75, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
	})

	// This world answers a cap for any name at all, so the only thing keeping
	// max_level off the prototype is the planner not asking.
	ops, err := lib.PlanData(baseWorld().answeringMaxLevelForAnyName(Num(20)))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=75, time=30, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// The two ingredient shapes are not interchangeable: a recipe takes the long
// dict form, a technology unit takes the short tuple form, and the engine
// refuses each in the other's place.
func TestTechnologyUnitSpecUsesShortTupleForm(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{
		Unit: &UnitSpec{
			Count:   75,
			Seconds: 30,
			Packs: []Pack{
				{Name: "automation-science-pack", Amount: 1},
				{Name: "logistic-science-pack", Amount: 2},
			},
		},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=75, time=30, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 2]]}}`,
	})
}

func TestTechnologyEnabledBy(t *testing.T) {
	cases := []struct {
		name    string
		def     bool
		world   func(*fixtureWorld) *fixtureWorld
		tail    string
		logLine string
	}{
		{
			name:  "the setting is on",
			def:   false,
			world: func(w *fixtureWorld) *fixtureWorld { return w.withSetting("steelworks-hardened-tools", Bool(true)) },
			tail:  `, enabled=true}`,
		},
		{
			name:  "the setting is off",
			def:   true,
			world: func(w *fixtureWorld) *fixtureWorld { return w.withSetting("steelworks-hardened-tools", Bool(false)) },
			tail:  `, enabled=false, hidden=true}`,
		},
		{
			name:    "the setting is unreadable and defaults on",
			def:     true,
			world:   func(w *fixtureWorld) *fixtureWorld { return w },
			tail:    `, enabled=true}`,
			logLine: "log fkrecipes: the setting steelworks-hardened-tools was not readable, so its default applies",
		},
		{
			name:    "the setting is unreadable and defaults off",
			def:     false,
			world:   func(w *fixtureWorld) *fixtureWorld { return w },
			tail:    `, enabled=false, hidden=true}`,
			logLine: "log fkrecipes: the setting steelworks-hardened-tools was not readable, so its default applies",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			on := lib.BoolSetting("hardened-tools", c.def)
			lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", EnabledBy: on})

			ops, err := lib.PlanData(c.world(baseWorld()))
			assertNoError(t, err)

			want := []string{}
			if c.logLine != "" {
				want = append(want, c.logLine)
			}
			want = append(want, `extend {type="technology", name="steelworks-steel-axes", unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}`+c.tail)
			assertLines(t, transcript(ops), want)
		})
	}
}

func TestTreePlacement(t *testing.T) {
	cases := []struct {
		name  string
		after string
		// before is empty for the plain After case.
		before string
		world  func(*fixtureWorld) *fixtureWorld
		want   []string
	}{
		{
			name:  "after alone",
			after: "logistics-2",
			world: func(w *fixtureWorld) *fixtureWorld { return w },
			want:  []string{`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}`},
		},
		{
			name:  "after alone, absent",
			after: "quarry-drills",
			world: func(w *fixtureWorld) *fixtureWorld { return w },
			want: []string{
				`log fkrecipes: steel-axes: quarry-drills is absent, so the prerequisite is dropped`,
				`extend {type="technology", name="steelworks-steel-axes", unit=UNIT}`,
			},
		},
		{
			name:   "insert between, the splice replaces the edge",
			after:  "logistics-2",
			before: "logistics-3",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want: []string{
				`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}`,
				`set technology.logistics-3.prerequisites = ["steelworks-steel-axes"]`,
			},
		},
		{
			name:   "insert between, the far end does not require the near one",
			after:  "electronics",
			before: "logistics-3",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want: []string{
				`log fkrecipes: steel-axes: logistics-3 does not require electronics, so the new technology is appended to its prerequisites`,
				`extend {type="technology", name="steelworks-steel-axes", prerequisites=["electronics"], unit=UNIT}`,
				`set technology.logistics-3.prerequisites = ["logistics-2", "steelworks-steel-axes"]`,
			},
		},
		{
			// Another mod removed the technology this plan meant to splice
			// in front of.
			name:   "insert between, the far end is absent",
			after:  "logistics-2",
			before: "logistics-3",
			world:  func(w *fixtureWorld) *fixtureWorld { return w.withoutTech("logistics-3") },
			want: []string{
				`log fkrecipes: steel-axes: logistics-3 is absent, so InsertBetween degrades to After(logistics-2)`,
				`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}`,
			},
		},
		{
			name:   "insert between, the near end is absent",
			after:  "quarry-drills",
			before: "logistics-3",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want: []string{
				`log fkrecipes: steel-axes: quarry-drills is absent, so the prerequisite is dropped`,
				`extend {type="technology", name="steelworks-steel-axes", unit=UNIT}`,
			},
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: c.after, Before: c.before})

			ops, err := lib.PlanData(c.world(baseWorld()))
			assertNoError(t, err)

			want := make([]string, 0, len(c.want))
			for _, line := range c.want {
				want = append(want, strings.Replace(line, "UNIT", steelProcessingUnit, 1))
			}
			assertLines(t, transcript(ops), want)
		})
	}
}

// Two splices into the same technology chain: the second reads what the first
// planned, or the second Set op would silently undo the first.
func TestTwoSplicesIntoOneTechnologyChain(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", Before: "logistics-3"})
	lib.Technology("bronze-axes", TechSpec{CostOf: "steel-processing", After: "electronics", Before: "logistics-3"})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: bronze-axes: logistics-3 does not require electronics, so the new technology is appended to its prerequisites`,
		`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}`,
		`extend {type="technology", name="steelworks-bronze-axes", prerequisites=["electronics"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}`,
		`set technology.logistics-3.prerequisites = ["steelworks-steel-axes"]`,
		`set technology.logistics-3.prerequisites = ["steelworks-steel-axes", "steelworks-bronze-axes"]`,
	})
}

// Nothing this library emits can carry a name it did not prefix. The check is
// a property over the whole stream, not one assertion per prototype.
func TestEveryEmittedNameIsPrefixed(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{Icon: "__steelworks__/graphics/icons/steel-axe.png"})
	rec := lib.Recipe(axe, RecipeSpec{Name: "steel-axe-forging", Ingredients: []Ingredient{IngredientNamed(4, "steel-plate")}})
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", Before: "logistics-3", Unlocks: []RecipeRef{rec}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	extends := 0
	for _, op := range ops {
		if op.Kind != OpExtend {
			continue
		}
		extends++
		name, ok := field(op.Proto, "name")
		if !ok {
			t.Fatalf("a prototype carries no name: %s", renderValue(op.Proto))
		}
		if !strings.HasPrefix(name.Str, "steelworks-") {
			t.Errorf("prototype name %q is not prefixed", name.Str)
		}
	}
	if extends != 3 {
		t.Fatalf("expected three prototypes, got %d", extends)
	}

	// The splice writes into somebody else's technology, so the PATH keeps
	// their unprefixed name and only the value carries ours.
	last := ops[len(ops)-1]
	if last.Kind != OpSet {
		t.Fatalf("the last op is not the splice: %v", last.Kind)
	}
	if got := renderPath(last.Path); got != "technology.logistics-3.prerequisites" {
		t.Errorf("splice path is %q", got)
	}
	if got := renderValue(last.Val); got != `["steelworks-steel-axes"]` {
		t.Errorf("splice value is %q", got)
	}
}

func TestPlanDataRefusals(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		world func(*fixtureWorld) *fixtureWorld
		want  string
	}{
		{
			name: "two items share a name",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{})
				l.Item("steel-axe", ItemSpec{})
			},
			want: "fkrecipes: two items share the name steelworks-steel-axe; the second would overwrite the first",
		},
		{
			name: "two recipes share a name",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				head := l.Item("axe-head", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Name: "steel-axe-forging"})
				l.Recipe(head, RecipeSpec{Name: "steel-axe-forging"})
			},
			want: "fkrecipes: two recipes share the name steelworks-steel-axe-forging; the second would overwrite the first",
		},
		{
			name: "two technologies share a name",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
				l.Technology("steel-axes", TechSpec{CostOf: "logistics-2"})
			},
			want: "fkrecipes: two technologies share the name steelworks-steel-axes; the second would overwrite the first",
		},
		{
			name: "a recipe with no result item",
			build: func(l *Lib) {
				l.Recipe(ItemRef{}, RecipeSpec{Name: "steel-axe-forging"})
			},
			want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
		},
		{
			name: "an ingredient item from no plan",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientOf(ItemRef{}, 1)}})
			},
			want: "fkrecipes: the recipe steel-axe names an ingredient item that this plan never declared",
		},
		{
			name: "neither CostOf nor Unit",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{After: "steel-processing"})
			},
			want: "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit, CostBy or CostFrom",
		},
		{
			name: "both CostOf and Unit",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{
					CostOf: "steel-processing",
					Unit:   &UnitSpec{Count: 50, Seconds: 15, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
				})
			},
			want: "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit, CostBy or CostFrom",
		},
		{
			name: "Before without After",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", Before: "logistics-3"})
			},
			want: "fkrecipes: the technology steel-axes names Before without After; InsertBetween needs both ends",
		},
		{
			name: "a unit count below one",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{Count: 0, Seconds: 15, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}}})
			},
			want: "fkrecipes: the technology steel-axes has a unit count below 1, which the engine refuses",
		},
		// A UNIT WHOSE EVERY SCIENCE PACK THE GAME LACKS IS NOT HERE ANY MORE.
		// A pack the game lacks is DROPPED, and a unit that kept none of them
		// is EMITTED EMPTY with a line and a tooltip rather than refused: see
		// TestAUnitWithEveryPackDroppedIsEmittedEmptyAndSaysSo in
		// packladder_test.go, which is where every case of it lives now.
		{
			// An empty rung is a ladder that can never answer, and it would
			// reach a log line with a hole in it.
			name: "a pack ladder with an empty rung",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{Count: 50, Seconds: 15,
					Packs: []Pack{{Name: "automation-science-pack", Amount: 1, Fallbacks: []string{""}}}}})
			},
			want: "fkrecipes: the technology steel-axes prices itself in a pack with an empty name",
		},
		{
			name: "CostOf names a technology that is not there",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "logistics-4"})
			},
			want: "fkrecipes: CostOf(logistics-4): no technology of that name exists",
		},
		{
			name: "CostOf names a research_trigger technology",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steam-power"})
			},
			want: "fkrecipes: CostOf(steam-power): steam-power is a research_trigger technology with no unit to copy; name a unit-carrying technology instead",
		},
		{
			name: "unlocking a recipe from no plan",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", Unlocks: []RecipeRef{{}}})
			},
			want: "fkrecipes: the technology steel-axes unlocks a recipe that this plan never declared",
		},
		{
			// Out of range for THIS plan, which is the shape that used to
			// index straight into a shorter slice and panic.
			name: "an EnabledBy setting out of range",
			build: func(l *Lib) {
				other := New()
				other.BoolSetting("hardened-tools", true)
				other.BoolSetting("brittle-heads", false)
				stray := other.BoolSetting("sharpened-edges", true)
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", EnabledBy: stray})
			},
			want: "fkrecipes: the technology steel-axes names an EnabledBy setting that this plan never declared",
		},
		{
			name: "an item this plan would overwrite",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{})
			},
			world: func(w *fixtureWorld) *fixtureWorld { return w.withItem("steelworks-steel-axe") },
			want:  "fkrecipes: the item steelworks-steel-axe already exists in data.raw; this plan would overwrite it",
		},
		{
			name: "a recipe this plan would overwrite",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Name: "steel-axe-forging"})
			},
			world: func(w *fixtureWorld) *fixtureWorld { return w.withRecipe("steelworks-steel-axe-forging") },
			want:  "fkrecipes: the recipe steelworks-steel-axe-forging already exists in data.raw; this plan would overwrite it",
		},
		{
			name: "a technology this plan would overwrite",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withTech(fixtureTech{name: "steelworks-steel-axes", unit: unitOf(50, 15, "automation-science-pack")})
			},
			want: "fkrecipes: the technology steelworks-steel-axes already exists in data.raw; this plan would overwrite it",
		},
		{
			name: "both anchors at once",
			build: func(l *Lib) {
				first := l.Technology("bronze-axes", TechSpec{CostOf: "steel-processing"})
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", AfterTech: first})
			},
			want: "fkrecipes: the technology steel-axes names both After and AfterTech; pick one anchor",
		},
		{
			name: "a splice around a technology this plan declares",
			build: func(l *Lib) {
				first := l.Technology("bronze-axes", TechSpec{CostOf: "steel-processing"})
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", AfterTech: first, Before: "logistics-3"})
			},
			want: "fkrecipes: the technology steel-axes names Before with AfterTech; InsertBetween splices around a technology that already exists",
		},
		{
			name: "a crafting time that is not a number",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTime: math.Inf(1)})
			},
			want: "fkrecipes: the recipe steel-axe declares a crafting time that is not a finite number",
		},
		{
			name: "a research time that is not a number",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   50,
					Seconds: math.NaN(),
					Packs:   []Pack{{Name: "automation-science-pack", Amount: 1}},
				}})
			},
			want: "fkrecipes: the technology steel-axes declares a research time that is not a finite number",
		},
		{
			name: "an item with an empty name",
			build: func(l *Lib) {
				l.Item("", ItemSpec{})
			},
			want: "fkrecipes: an item was declared with an empty name",
		},
		{
			name: "a technology with an empty name",
			build: func(l *Lib) {
				l.Technology("", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: a technology was declared with an empty name",
		},
		{
			// A holed or mixed Lua table crosses as a number-keyed map, which
			// converts to nil as a whole subtree rather than being half kept.
			// A unit that lost its ingredient list is a technology researchable
			// for free, so the copy is refused rather than emitted.
			name: "a CostOf source whose unit lost a subtree on the way in",
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withUnit("steel-processing", Obj(
					kv("count", Num(50)),
					kv("ingredients", Nil()),
					kv("time", Num(15)),
				))
			},
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: CostOf(steel-processing): the unit of steel-processing holds a table this library cannot copy faithfully",
		},
		{
			name: "a science pack with an empty name",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   50,
					Seconds: 15,
					Packs:   []Pack{{Name: "", Amount: 1}},
				}})
			},
			want: "fkrecipes: the technology steel-axes prices itself in a pack with an empty name",
		},
		{
			name: "both a fixed and a bound crafting time",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
				l.Recipe(axe, RecipeSpec{CraftTime: 2.5, CraftTimeFrom: from})
			},
			want: "fkrecipes: the recipe steel-axe names both CraftTime and CraftTimeFrom; pick one",
		},
		{
			name: "a crafting-time setting from another plan",
			build: func(l *Lib) {
				other := New()
				stray := other.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: stray})
			},
			want: "fkrecipes: the recipe steel-axe names a crafting-time setting that this plan never declared",
		},
		{
			// Measured: the engine refuses energy_required <= 0.001.
			name: "a declared crafting time below the engine floor",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTime: 0.001})
			},
			want: "fkrecipes: the recipe steel-axe declares a crafting time the engine refuses (energy_required can't be <= 0.001)",
		},
		{
			// THE AUTHOR'S HALF OF THE CRAFTING-TIME PAIR. A value the
			// player's setting answered falls back (see
			// TestBoundCraftingTimeFallsBack); the DECLARED default it falls
			// back onto is still held to the engine's rule, and this is the
			// only world that reaches the check. validateSettings refuses this
			// declaration at the settings stage, so PlanData sees it only when
			// a host test calls it on its own.
			//
			// THE SENTENCE NAMES THE DECLARED DEFAULT, not what the setting
			// answered: the stored NaN is gone by the time this check runs,
			// and the number it is about is the 0.001 the plan wrote. The
			// stored value that was set aside on the way here adds one FACT
			// and no advice, because the log ops never reach the host on a
			// refused load and there is no screen to send anybody to.
			name: "a bound crafting time whose declared default is below the engine floor",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 0.001, NumericSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: from})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withSetting("steelworks-axe-craft-time", Num(math.NaN()))
			},
			want: withFallbackFact("fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, whose declared default is at or below the engine floor (energy_required can't be <= 0.001)", "steelworks-axe-craft-time"),
		},
		{
			// The same pair for finiteness: a declared default of an infinity
			// is above the floor, so the floor arm would wave it through and
			// ship a recipe that never completes.
			name: "a bound crafting time whose declared default is an infinity",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", math.Inf(1), NumericSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: from})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withSetting("steelworks-axe-craft-time", Num(0.001))
			},
			want: withFallbackFact("fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, whose declared default is not a finite number", "steelworks-axe-craft-time"),
		},
		{
			// PRESENT and nil, which is what a unit whose table carried a
			// numeric key collapses to: a different answer from "no unit".
			name:  "a CostOf source whose unit arrives as nil",
			world: func(w *fixtureWorld) *fixtureWorld { return w.withNilUnit("steel-processing") },
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: CostOf(steel-processing): steel-processing has a unit that is not a dictionary",
		},
		{
			name: "a CostOf source whose max_level lost a subtree on the way in",
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withMaxLevel("steel-processing", Obj(kv("levels", Nil())))
			},
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: CostOf(steel-processing): the max_level of steel-processing holds a table this library cannot copy faithfully",
		},
		{
			// The World says the technology is there and is not a research
			// trigger, but hands back no unit: the arm the emit layer's
			// TechUnit read lands on.
			name:  "a CostOf source that carries no unit at all",
			world: func(w *fixtureWorld) *fixtureWorld { return w.withUnit("steel-processing", Nil()) },
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: CostOf(steel-processing): steel-processing carries no unit to copy",
		},
		{
			name: "a negative stack size",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{StackSize: -20})
			},
			want: "fkrecipes: the item steel-axe has a negative stack size, which the engine refuses",
		},
		{
			name: "a negative icon size on an item",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{IconSize: -64})
			},
			want: "fkrecipes: the item steel-axe has a negative icon size, which the engine refuses",
		},
		{
			name: "a negative icon size on a technology",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", IconSize: -128})
			},
			want: "fkrecipes: the technology steel-axes has a negative icon size, which the engine refuses",
		},
		{
			name: "an ingredient amount below one",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(0, "steel-plate")}})
			},
			want: "fkrecipes: the recipe steel-axe has an ingredient amount below 1, which the engine refuses",
		},
		{
			// A rung with no name is a ladder that can never answer, exactly
			// as it is for a science pack: ItemExists("") is a question no
			// World has a useful answer to, and the drop line the ingredient
			// would otherwise reach reads "none of  is present".
			name: "an ingredient ladder whose first rung has no name",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(4, "")}})
			},
			want: "fkrecipes: the recipe steel-axe names an ingredient with an empty name",
		},
		{
			// The FALLBACKS are held to the same rule, and they are the arm a
			// check written against the first name alone would miss.
			name: "an ingredient ladder whose fallback has no name",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(4, "steel-plate", "")}})
			},
			want: "fkrecipes: the recipe steel-axe names an ingredient with an empty name",
		},
		{
			// And inside a dropdown's plan, which is validated exactly like a
			// fixed list: a preset nobody selects today is a refusal the day
			// somebody does.
			name: "an empty ingredient name inside a dropdown's plan",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				temper := l.DropdownSettingNeedingLocale("axe-temper", "hard", []string{"hard", "soft"})
				l.Recipe(axe, RecipeSpec{
					IngredientsBy: &IngredientChoices{
						Setting: temper,
						Choices: []IngredientChoice{
							{Value: "hard", Ingredients: []Ingredient{IngredientNamed(4, "steel-plate")}},
							{Value: "soft", Ingredients: []Ingredient{IngredientNamed(4, "", "iron-plate")}},
						},
					},
				})
			},
			want: "fkrecipes: the recipe steel-axe names an ingredient with an empty name",
		},
		{
			name: "a negative result count",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{ResultCount: -2})
			},
			want: "fkrecipes: the recipe steel-axe has a negative result count, which the engine refuses",
		},
		{
			name: "a negative crafting time",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTime: -2.5})
			},
			want: "fkrecipes: the recipe steel-axe has a negative crafting time, which the engine refuses",
		},
		{
			name: "a science pack amount below one",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   50,
					Seconds: 15,
					Packs:   []Pack{{Name: "automation-science-pack", Amount: 0}},
				}})
			},
			want: "fkrecipes: the technology steel-axes has a science pack amount below 1, which the engine refuses",
		},
		{
			name: "a research time of zero",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   50,
					Seconds: 0,
					Packs:   []Pack{{Name: "automation-science-pack", Amount: 1}},
				}})
			},
			want: "fkrecipes: the technology steel-axes has a research time at or below zero, which the engine refuses",
		},
		{
			name: "a stack size past what a double holds",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{StackSize: 9007199254740993})
			},
			want: "fkrecipes: the item steel-axe declares a stack size a Lua double cannot hold exactly: 9007199254740993",
		},
		{
			name: "an ingredient amount past what a double holds",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(9007199254740993, "steel-plate")}})
			},
			want: "fkrecipes: the recipe steel-axe declares an ingredient amount a Lua double cannot hold exactly: 9007199254740993",
		},
		{
			name: "a unit count past what a double holds",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   9007199254740993,
					Seconds: 15,
					Packs:   []Pack{{Name: "automation-science-pack", Amount: 1}},
				}})
			},
			want: "fkrecipes: the technology steel-axes declares a unit count a Lua double cannot hold exactly: 9007199254740993",
		},
		{
			name: "a CostOf source whose unit is not a dictionary",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				// A Lua sequence IS a table, which is why the refusal says
				// dictionary: this shape has to be refused too.
				return w.withUnit("steel-processing", Arr(Num(50), Num(15)))
			},
			want: "fkrecipes: CostOf(steel-processing): steel-processing has a unit that is not a dictionary",
		},
		{
			// The result handle is checked before the duplicate-name scan, so
			// a recipe with no result is told what is actually wrong instead
			// of being reported as a name collision with the recipe it
			// accidentally shares a name with.
			name: "a recipe with no result item that also collides by name",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Name: "steel-axe-forging"})
				l.Recipe(ItemRef{}, RecipeSpec{Name: "steel-axe-forging"})
			},
			want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			w := baseWorld()
			if c.world != nil {
				w = c.world(w)
			}
			ops, err := lib.PlanData(w)
			if err == nil {
				t.Fatalf("plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}
}

// A handle is a fact about ONE plan. Another plan's handle is in range here
// and points at something else entirely, so it is refused rather than
// silently resolved into this plan's prototypes.
func TestHandlesFromAnotherPlanAreRefused(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			// The review's first shape: an in-range ItemRef that names a
			// different item in the other plan.
			name: "a result item handle from another plan",
			build: func(l *Lib) {
				other := New()
				bronze := other.Item("bronze-axe", ItemSpec{})
				l.Item("steel-axe", ItemSpec{})
				l.Recipe(bronze, RecipeSpec{Name: "steel-axe-forging"})
			},
			want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
		},
		{
			name: "an ingredient handle from another plan",
			build: func(l *Lib) {
				other := New()
				bronze := other.Item("bronze-axe", ItemSpec{})
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientOf(bronze, 1)}})
			},
			want: "fkrecipes: the recipe steel-axe names an ingredient item that this plan never declared",
		},
		{
			// The review's second shape: a BoolSettingRef that lands on this
			// plan's int setting, which would have read a bool out of it.
			name: "an EnabledBy handle from another plan lands on an int setting",
			build: func(l *Lib) {
				other := New()
				flag := other.BoolSetting("hardened-tools", true)
				l.IntSetting("axe-durability", 250, Between(50, 1000))
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", EnabledBy: flag})
			},
			want: "fkrecipes: the technology steel-axes names an EnabledBy setting that this plan never declared",
		},
		{
			name: "an unlock handle from another plan",
			build: func(l *Lib) {
				other := New()
				bronze := other.Item("bronze-axe", ItemSpec{})
				rec := other.Recipe(bronze, RecipeSpec{})
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{})
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", Unlocks: []RecipeRef{rec}})
			},
			want: "fkrecipes: the technology steel-axes unlocks a recipe that this plan never declared",
		},
		{
			name: "an AfterTech handle from another plan",
			build: func(l *Lib) {
				other := New()
				anchor := other.Technology("bronze-axes", TechSpec{CostOf: "steel-processing"})
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", AfterTech: anchor})
			},
			want: "fkrecipes: the technology steel-axes names an AfterTech technology that this plan never declared",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanData(baseWorld())
			if err == nil {
				t.Fatalf("plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}
}

// AfterTech is how a plan chains its own research: After probes the GAME, so
// an own-tech name would log a drop and leave the second technology detached.
func TestAfterTechChainsOwnTechnologies(t *testing.T) {
	lib := New()
	first := lib.Technology("bronze-axes", TechSpec{CostOf: "steel-processing", After: "steel-processing"})
	lib.Technology("steel-axes", TechSpec{CostOf: "logistics-2", AfterTech: first})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-bronze-axes", prerequisites=["steel-processing"], unit=` + steelProcessingUnit + `}`,
		`extend {type="technology", name="steelworks-steel-axes", prerequisites=["steelworks-bronze-axes"], unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

func TestPlanDataRefusesAnEmptyModName(t *testing.T) {
	lib := New()
	lib.Item("steel-axe", ItemSpec{})

	ops, err := lib.PlanData(baseWorld().withModName(""))
	if err == nil {
		t.Fatal("plan was accepted with no mod name")
	}
	want := "fkrecipes: the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// TinyGo's wasm int is 32 bits and the Rust mirror's i64 is not, so the
// widths are pinned rather than assumed. These are not realistic Factorio
// numbers: the point is that both halves carry the same one.
func TestWideAmountsSurviveTheEmit(t *testing.T) {
	lib := New()
	// 2^53 exactly: the largest integer a Lua double still holds without
	// rounding, so the planner accepts it and the transcript prints it as
	// the plain digits it is.
	axe := lib.Item("steel-axe", ItemSpec{StackSize: 9007199254740992})
	lib.Recipe(axe, RecipeSpec{
		// An INGREDIENT carries the engine's own ceiling of 65535 and so
		// cannot be a wide number at all, and neither can a SCIENCE PACK: a
		// unit ingredient is held in the same 16 bits (measured on 2.0.77,
		// where 65536 refuses the load with "The data type allows values from
		// 0 to 65535"). The stack size, the result count and the unit count
		// have no such ceiling and are what pin the width.
		Ingredients: []Ingredient{IngredientNamed(65535, "steel-plate")},
		ResultCount: 2500000000,
	})
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   5000000000,
		Seconds: 15,
		Packs:   []Pack{{Name: "automation-science-pack", Amount: 65535}},
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=9007199254740992}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steel-plate", amount=65535}], results=[{type="item", name="steelworks-steel-axe", amount=2500000000}]}`,
		`extend {type="technology", name="steelworks-steel-axes", unit={count=5000000000, time=15, ingredients=[["automation-science-pack", 65535]]}}`,
	})
}

// A plan is a snapshot. Reusing one buffer across several declarations is
// ordinary Go, and a consumer who does it must not find their first recipe
// rewritten by their second.
func TestDeclarationsDoNotAliasCallerSlices(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	head := lib.Item("axe-head", ItemSpec{})

	ingredients := []Ingredient{IngredientOf(head, 1), IngredientNamed(4, "steel-plate")}
	lib.Recipe(axe, RecipeSpec{Ingredients: ingredients})
	// The caller reuses the buffer for their next recipe.
	ingredients[0] = IngredientNamed(99, "copper-plate")
	ingredients[1] = IngredientNamed(99, "iron-plate")

	packs := []Pack{{Name: "automation-science-pack", Amount: 1}}
	unit := &UnitSpec{Count: 50, Seconds: 15, Packs: packs}
	lib.Technology("steel-axes", TechSpec{Unit: unit})
	// The caller keeps the unit and edits it afterwards.
	packs[0].Amount = 99
	unit.Count = 99
	unit.Seconds = 99

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="item", name="steelworks-axe-head", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steelworks-axe-head", amount=1}, {type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// Go-only: the Rust mirror takes &dyn World, which cannot be null.
func TestPlanningRefusesANilWorld(t *testing.T) {
	lib := New()
	lib.Item("steel-axe", ItemSpec{})

	if _, err := lib.PlanData(nil); err == nil || err.Error() != "fkrecipes: PlanData was given a nil World" {
		t.Errorf("PlanData(nil) gave %v", err)
	}
	if _, err := lib.PlanSettings(nil); err == nil || err.Error() != "fkrecipes: PlanSettings was given a nil World" {
		t.Errorf("PlanSettings(nil) gave %v", err)
	}
}

// A max_level the World reports as present but nil is a value this library
// could not carry across; writing it would put a nil into the prototype.
func TestANilMaxLevelIsNotEmitted(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})

	ops, err := lib.PlanData(baseWorld().withNilMaxLevel("steel-processing"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit=` + steelProcessingUnit + `}`,
	})
}

// A Set op's value reaches fkdata.Set, where a nil DELETES the key instead of
// writing one. Nothing plans a deletion, and this is what says so.
func TestNoPlannedSetOpCarriesANilValue(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", Before: "logistics-3"})
	lib.Technology("bronze-axes", TechSpec{CostOf: "steel-processing", After: "electronics", Before: "logistics-3"})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	sets := 0
	for _, op := range ops {
		if op.Kind != OpSet {
			continue
		}
		sets++
		if op.Val.Kind == KindNil {
			t.Errorf("a Set op at %s carries nil, which would delete the key", renderPath(op.Path))
		}
		for _, entry := range op.Val.Arr {
			if entry.Kind == KindNil {
				t.Errorf("a Set op at %s carries a nil entry", renderPath(op.Path))
			}
		}
	}
	if sets != 2 {
		t.Fatalf("expected two splices, got %d", sets)
	}
}

// The whole binding, end to end: the settings stage generates the setting
// with a minimum that clears the engine floor, and the data stage reads the
// player's answer back into energy_required.
func TestCraftTimeBindingRoundTrip(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	from := lib.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
	lib.Recipe(axe, RecipeSpec{
		CraftTimeFrom: from,
		Ingredients:   []Ingredient{IngredientNamed(4, "steel-plate")},
	})

	settingsOps, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	assertLines(t, transcript(settingsOps), []string{
		`extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=2.5000000000000000e0, order="aa", minimum_value=2.0000000000000000e-3}`,
	})

	// The player set it to four seconds.
	dataOps, err := lib.PlanData(baseWorld().withSetting("steelworks-axe-craft-time", Num(4)))
	assertNoError(t, err)
	assertLines(t, transcript(dataOps), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", energy_required=4, enabled=true, ingredients=[{type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// An unreadable setting degrades the same way an unreadable enablement does:
// one log line, and the declared default applies.
func TestCraftTimeBindingFallsBackToItsDefault(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	from := lib.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
	lib.Recipe(axe, RecipeSpec{CraftTimeFrom: from})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-axe-craft-time was not readable, so its default applies`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", energy_required=2.5000000000000000e0, enabled=true, ingredients=[], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// A CRAFTING TIME IS A FIELD THE PLAYER OWNS, so a value the engine would not
// take falls back to the setting's declared default with one ERROR line rather
// than stopping the load.
//
// The generated minimum clears the engine floor, so a player cannot type one of
// these through the settings screen; a second mod declaring the same setting
// name can hand one over, because setting names are a global namespace and the
// engine keeps the last declaration of a same-type name silently. That is not
// something to lock a player out of their save for, and the line names the
// setting so they can see which one collided.
func TestBoundCraftingTimeFallsBack(t *testing.T) {
	cases := []struct {
		name   string
		answer Value
		want   string
	}{
		{
			name:   "at or below the engine floor",
			answer: Num(0.001),
			want: "the recipe steel-axe reads its crafting time from steelworks-axe-craft-time," +
				" which answers at or below the engine floor (energy_required can't be <= 0.001)",
		},
		{
			// An infinity is ABOVE the floor, so a floor arm reached first
			// would wave it through and ship a recipe that never completes.
			name:   "an infinity",
			answer: Num(math.Inf(1)),
			want: "the recipe steel-axe reads its crafting time from steelworks-axe-craft-time," +
				" which answers a value that is not a finite number",
		},
		{
			// A NaN compares false against the floor, so a floor arm reached
			// first would report it as a value at or below one, which it is not.
			name:   "a NaN",
			answer: Num(math.NaN()),
			want: "the recipe steel-axe reads its crafting time from steelworks-axe-craft-time," +
				" which answers a value that is not a finite number",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			axe := lib.Item("steel-axe", ItemSpec{})
			from := lib.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
			lib.Recipe(axe, RecipeSpec{CraftTimeFrom: from})

			ops, err := lib.PlanData(baseWorld().withSetting("steelworks-axe-craft-time", c.answer))
			assertNoError(t, err)

			assertLines(t, transcript(ops), []string{
				`log fkrecipes: ERROR: ` + c.want +
					`. The mod loaded as though that number had been left alone; fix the number under Settings > Mod settings > Startup, then restart.`,
				`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
				`extend {type="recipe", name="steelworks-steel-axe", ` +
					noteIn("steelworks-axe-craft-time", false) +
					`energy_required=2.5000000000000000e0, enabled=true, ingredients=[], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
			})
		})
	}
}

// ONE BAD FIELD IS ONE LINE, however many declarations read it.
//
// A CRAFTING TIME IS THE ONE SETTING TWO DECLARATIONS MAY SHARE in this
// library (validateBindings refuses a shared cost number and a shared text, and
// says why). A player looking at the settings screen sees ONE field, so a bad
// value in it is ONE problem: a line per recipe would report the same typo
// twice and name a different recipe each time, and the count of ERROR lines a
// gate greps for would depend on how many recipes happened to bind it.
//
// THE LINE NAMES THE FIRST RECIPE IN DECLARATION ORDER, which is the same every
// run, and BOTH recipes still take the declared default: the dedupe is about
// the line, never about the value.
func TestOneBadCraftingTimeSettingTwoRecipesLogsOneLine(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	hammer := lib.Item("steel-hammer", ItemSpec{})
	from := lib.DoubleSetting("forging-time", 2.5, NumericSpec{})
	lib.Recipe(axe, RecipeSpec{Name: "steel-axe-forging", CraftTimeFrom: from,
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}})
	lib.Recipe(hammer, RecipeSpec{Name: "steel-hammer-forging", CraftTimeFrom: from,
		Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}})

	ops, err := lib.PlanData(baseWorld().withSetting("steelworks-forging-time", Num(0)))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: the recipe steel-axe-forging reads its crafting time from steelworks-forging-time,` +
			` which answers at or below the engine floor (energy_required can't be <= 0.001).` +
			` The mod loaded as though that number had been left alone; fix the number under Settings > Mod settings > Startup, then restart.`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="item", name="steelworks-steel-hammer", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe-forging", ` +
			noteIn("steelworks-forging-time", false) +
			`energy_required=2.5000000000000000e0, enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
		`extend {type="recipe", name="steelworks-steel-hammer-forging", ` +
			noteIn("steelworks-forging-time", false) +
			`energy_required=2.5000000000000000e0, enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}],` +
			` results=[{type="item", name="steelworks-steel-hammer", amount=1}]}`,
	})
}

// ---------------------------------------------------------------------------
// Two Libs, two data hooks: the split-emission pattern.
// ---------------------------------------------------------------------------

// THE ONE-DATA-HOOK RULE IS PER Lib, NOT PER MOD, and this is what says so.
//
// A single Lib emitted from two data-family hooks refuses, because the second
// pass finds the first pass's prototypes already in data.raw and reports the
// overwrite. That is a real rule and it is not the rule "a mod gets one data
// hook": a mod may carry TWO plans, one creating its own content at fk_data
// and one patching another mod's tree at fk_data_updates, each emitting its
// own settings at fk_settings. The stages share one data.raw, so what makes it
// work is that the two plans declare different names; nothing else is needed.
//
// The patching plan is planned against a world that carries the creating
// plan's prototypes, which is what data.raw actually looks like by the time
// fk_data_updates runs.
func TestTwoLibsSplitCreationFromPatching(t *testing.T) {
	// PLAN A, at fk_data: this mod's own content.
	creation := New()
	hardened := creation.BoolSetting("hardened-tools", true)
	plate := creation.Item("hardened-steel-plate", ItemSpec{})
	creation.Recipe(plate, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}})
	creation.Technology("hardened-steel", TechSpec{
		CostOf:    "logistics-2",
		After:     "steel-processing",
		EnabledBy: hardened,
	})

	// The setting is answered rather than left unreadable, so the transcript
	// is the split itself and not a degradation log.
	aOps, err := creation.PlanData(baseWorld().withSetting("steelworks-hardened-tools", Bool(true)))
	assertNoError(t, err)
	assertLines(t, transcript(aOps), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
		`extend {type="technology", name="steelworks-hardened-steel", prerequisites=["steel-processing"], unit=` + logistics2Unit + `, enabled=true}`,
	})

	// data.raw AS PLAN A LEFT IT. The patching plan runs a stage later, so
	// everything above is already there and is what it plans against.
	after := baseWorld().
		withSetting("steelworks-deep-tempering", Bool(true)).
		withItem("steelworks-hardened-steel-plate").
		withRecipe("steelworks-hardened-steel-plate").
		withTech(fixtureTech{
			name:    "steelworks-hardened-steel",
			prereqs: []string{"steel-processing"},
			unit:    logistics2UnitValue(),
		})

	// PLAN B, at fk_data_updates: a patch, spliced around plan A's OWN
	// technology by name. It cannot use AfterTech, because that takes a handle
	// and handles do not cross plans; a name is how one plan reaches another's
	// emitted prototype, which is the same way it reaches any other mod's.
	patch := New()
	deep := patch.BoolSetting("deep-tempering", true)
	patch.Technology("tempering", TechSpec{
		CostOf:    "logistics-3",
		After:     "steel-processing",
		Before:    "steelworks-hardened-steel",
		EnabledBy: deep,
	})

	bOps, err := patch.PlanData(after)
	if err != nil {
		t.Fatalf("the patching plan was refused: %v", err)
	}
	assertLines(t, transcript(bOps), []string{
		`extend {type="technology", name="steelworks-tempering", prerequisites=["steel-processing"], unit=` + logistics3Unit + `, enabled=true}`,
		`set technology.steelworks-hardened-steel.prerequisites = ["steelworks-tempering"]`,
	})

	// Each plan emits ITS OWN settings, and the two sets are disjoint: the
	// settings stage runs once, so both hooks route into it and a shared name
	// would be the silent last-writer-wins the settings validator refuses
	// within one plan and cannot see across two.
	aSettings, err := creation.PlanSettings(settingsWorld())
	assertNoError(t, err)
	bSettings, err := patch.PlanSettings(settingsWorld())
	assertNoError(t, err)
	assertLines(t, transcript(aSettings), []string{
		`extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="aa"}`,
	})
	assertLines(t, transcript(bSettings), []string{
		`extend {type="bool-setting", name="steelworks-deep-tempering", setting_type="startup", default_value=true, order="aa"}`,
	})
}

// The other half of the rule, so the pair cannot both pass on a library that
// never refuses an overwrite: two plans that DO share a name are refused, and
// the refusal comes from the world carrying the first plan's prototype rather
// than from anything the second plan knows about the first.
func TestASecondLibSharingANameIsStillRefused(t *testing.T) {
	patch := New()
	patch.Technology("hardened-steel", TechSpec{CostOf: "logistics-3"})

	after := baseWorld().withTech(fixtureTech{
		name:    "steelworks-hardened-steel",
		prereqs: []string{"steel-processing"},
		unit:    logistics2UnitValue(),
	})

	_, err := patch.PlanData(after)
	if err == nil {
		t.Fatal("the plan was accepted, want an overwrite refusal")
	}
	want := "fkrecipes: the technology steelworks-hardened-steel already exists in data.raw; this plan would overwrite it"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}
