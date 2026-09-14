package fkrecipes

import (
	"math"
	"strings"
	"testing"
)

// FLUIDS AS AN AUTHOR DECLARES THEM. The language a player types is tested
// against the corpus; this is the other half: FluidIngredient, the ladder it
// walks, the prototype shape it emits, and the one rule the engine has about
// where a fluid may appear at all.

// A fluid ingredient emits {type="fluid", ...} and its amount rides as the
// double it was declared as. MEASURED: the engine dumped 0.5 and 1000000000
// unchanged, so an amount that is not a whole number is the fluid case working
// rather than a mistake.
func TestFluidIngredientEmitsTheFluidShape(t *testing.T) {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{Icon: "__steelworks__/graphics/icons/plate.png"})
	lib.Recipe(plate, RecipeSpec{
		Category: "crafting-with-fluid",
		Ingredients: []Ingredient{
			IngredientNamed(2, "steel-plate"),
			FluidIngredient(0.5, "water"),
			FluidIngredient(10, "steam"),
		},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", icon="__steelworks__/graphics/icons/plate.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", category="crafting-with-fluid", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}, {type="fluid", name="water", amount=5.0000000000000000e-1}, {type="fluid", name="steam", amount=10}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// The fluid ladder is the item ladder's twin and asks a DIFFERENT question:
// FluidExists, not ItemExists. A world where the first candidate is only an
// item is a world where the ladder must walk past it, because emitting
// {type="fluid", name="steel-plate"} is a load failure with the consumer's
// name on it.
func TestFluidLadderProbesFluidsAndDropsWhatIsAbsent(t *testing.T) {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{Icon: "__steelworks__/graphics/icons/plate.png"})
	lib.Recipe(plate, RecipeSpec{
		Category: "chemistry",
		Ingredients: []Ingredient{
			// steel-plate is an ITEM in this world and never a fluid, so a
			// fluid ladder naming it has to walk past it to steam.
			FluidIngredient(1.5, "steel-plate", "steam"),
			// Neither rung is a fluid here: heavy-oil is nothing at all and
			// iron-plate is an item, so the ingredient is dropped rather than
			// guessed at.
			FluidIngredient(4, "heavy-oil", "iron-plate"),
		},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: hardened-steel-plate: none of heavy-oil, iron-plate is present, so the ingredient is dropped`,
		`extend {type="item", name="steelworks-hardened-steel-plate", icon="__steelworks__/graphics/icons/plate.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", category="chemistry", enabled=true, ingredients=[{type="fluid", name="steam", amount=1.5000000000000000e0}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// A fluid the world removed is dropped even when an ITEM of that name is
// there: the two namespaces are separate questions and the ladder asks only
// its own.
func TestFluidLadderIgnoresAnItemOfTheSameName(t *testing.T) {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{Icon: "__steelworks__/graphics/icons/plate.png"})
	lib.Recipe(plate, RecipeSpec{
		Category:    "chemistry",
		Ingredients: []Ingredient{FluidIngredient(1, "water")},
	})

	// water is an item here and not a fluid, which is the wrong way round for
	// a fluid ingredient.
	ops, err := lib.PlanData(baseWorld().withoutFluid("water").withItem("water"))
	assertNoError(t, err)

	// AND THE EMPTIED RECIPE SAYS SO. The one entry this plan named was put to
	// the game and dropped, so what is emitted is a recipe with no ingredients
	// at all: a free craft nobody chose, which is why it carries a line and a
	// note of its own rather than riding on the ladder's own disclosure.
	assertLines(t, transcript(ops), []string{
		`log fkrecipes: hardened-steel-plate: none of water is present, so the ingredient is dropped`,
		`log fkrecipes: ERROR: hardened-steel-plate: this game has none of the ingredients this recipe names, so it is emitted with no ingredients and costs nothing to craft`,
		`extend {type="item", name="steelworks-hardened-steel-plate", icon="__steelworks__/graphics/icons/plate.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", ` +
			`localised_description=["", ` + descriptionRefIn("recipe", "steelworks-hardened-steel-plate") + `, ` +
			chunkedParams(ingredientlessNote()+` Changing a recipe empties an assembling machine's input slots of anything the new list does not use.`) + `], ` +
			`category="chemistry", enabled=true, ingredients=[], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// THE CATEGORY RULE, refused at plan time so the sentence names the recipe.
//
// MEASURED: a fluid ingredient in a recipe with no category refuses the load
// with "Recipe is in 'crafting' category but has a non-item ingredient 'water'
// (fluid)", and the same ingredient loads under advanced-crafting,
// basic-crafting, smelting, crafting-with-fluid and chemistry. An absent
// category IS crafting, which is why both spellings are refused.
func TestFluidIngredientRefusalsAndBounds(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			name: "a fluid in a recipe with no category",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{Ingredients: []Ingredient{FluidIngredient(1, "water")}})
			},
			want: "fkrecipes: the recipe hardened-steel-plate takes the fluid water, and a recipe in the crafting category takes items only",
		},
		{
			name: "a fluid in a recipe that spells the crafting category out",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					Category:    "crafting",
					Ingredients: []Ingredient{IngredientNamed(2, "steel-plate"), FluidIngredient(0.5, "water")},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate takes the fluid water, and a recipe in the crafting category takes items only",
		},
		{
			// The ladder's FIRST name is what the sentence points at: the
			// declaration is wrong whichever rung the game would have answered
			// with, so the message names what the consumer wrote first.
			name: "a fluid ladder in the crafting category",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{Ingredients: []Ingredient{FluidIngredient(1, "steam", "water")}})
			},
			want: "fkrecipes: the recipe hardened-steel-plate takes the fluid steam, and a recipe in the crafting category takes items only",
		},
		{
			// A dropdown's plans are validated exactly like a fixed list, and
			// against the recipe's own category: a preset nobody selects today
			// is a load failure the day somebody does.
			name: "a fluid inside a dropdown's plan",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				medium := l.DropdownSettingNeedingLocale("quench-medium", "dry", []string{"dry", "wet"})
				l.Recipe(plate, RecipeSpec{
					IngredientsBy: &IngredientChoices{
						Setting: medium,
						Choices: []IngredientChoice{
							{Value: "dry", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
							{Value: "wet", Ingredients: []Ingredient{FluidIngredient(0.5, "water")}},
						},
					},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate takes the fluid water, and a recipe in the crafting category takes items only",
		},
		{
			// MEASURED: a fluid ingredient with amount 0 refuses the load with
			// "amount must be larger than 0". The floor is exclusive.
			name: "a fluid amount of zero",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					Category:    "chemistry",
					Ingredients: []Ingredient{FluidIngredient(0, "water")},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate has a fluid amount at or below zero, which the engine refuses",
		},
		{
			name: "a negative fluid amount",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					Category:    "chemistry",
					Ingredients: []Ingredient{FluidIngredient(-0.5, "water")},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate has a fluid amount at or below zero, which the engine refuses",
		},
		{
			// MEASURED: 1e301 loads and dumps, and 1e302 ABORTS the engine in
			// FixedPointNumber.hpp with the crash handler. A crash is not a
			// refusal a player can read, so the ceiling is refused here by the
			// same sentence the typed path answers with.
			name: "a declared fluid amount above the ceiling",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					Category:    "chemistry",
					Ingredients: []Ingredient{FluidIngredient(1e302, "water")},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate takes the fluid water at an amount above 1e301, which the game cannot hold",
		},
		{
			// The ladder's first name again, for the same reason as the
			// category sentence: the amount is wrong whichever rung answers.
			name: "a declared fluid ladder above the ceiling",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					Category:    "chemistry",
					Ingredients: []Ingredient{FluidIngredient(1e302, "heavy-oil", "water")},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate takes the fluid heavy-oil at an amount above 1e301, which the game cannot hold",
		},
		{
			// An infinity in a prototype is a load failure a consumer cannot
			// read their way out of, and the two languages print one
			// differently, so it never reaches an op.
			name: "a fluid amount that is not finite",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					Category:    "chemistry",
					Ingredients: []Ingredient{FluidIngredient(math.Inf(1), "water")},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate declares a fluid amount that is not a finite number",
		},
		{
			// THE EMPTY NAME COMES BEFORE THE CATEGORY SENTENCE, and this is
			// what makes that sentence's first-candidate index safe: without
			// this rule the refusal below would read "takes the fluid , and a
			// recipe in the crafting category takes items only", a hole where
			// the name goes.
			name: "a fluid whose first rung has no name",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{Ingredients: []Ingredient{FluidIngredient(1, "")}})
			},
			want: "fkrecipes: the recipe hardened-steel-plate names an ingredient with an empty name",
		},
		{
			// A fallback rung, in a category that takes fluids: the rule is
			// about the ladder and not about where the ladder stands.
			name: "a fluid whose fallback has no name",
			build: func(l *Lib) {
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					Category:    "chemistry",
					Ingredients: []Ingredient{FluidIngredient(0.5, "water", "")},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate names an ingredient with an empty name",
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

// THE CATEGORY SENTENCE COMES FIRST, in both halves of the language.
//
// A fluid of 1e302 in a crafting recipe is wrong twice over, and the two halves
// have to agree about which sentence comes out or a mod would be told different
// things by the Go build and the Rust build of the same declaration. The
// category is the one the author can act on: the amount does not matter in a
// recipe that takes no fluid at all.
func TestTheCategorySentenceComesBeforeTheFluidCeiling(t *testing.T) {
	want := "the recipe hardened-steel-plate takes the fluid water, and a recipe in the crafting category takes items only"

	t.Run("the declared path", func(t *testing.T) {
		lib := New()
		plate := lib.Item("hardened-steel-plate", ItemSpec{})
		lib.Recipe(plate, RecipeSpec{
			Category:    "crafting",
			Ingredients: []Ingredient{FluidIngredient(1e302, "water")},
		})

		_, err := lib.PlanData(baseWorld())
		if err == nil {
			t.Fatal("a fluid in a crafting recipe was accepted")
		}
		if err.Error() != "fkrecipes: "+want {
			t.Errorf("\n got: %s\nwant: fkrecipes: %s", err.Error(), want)
		}
	})

	// The typed path writes the same number the only way the language takes
	// one: 303 digits, because an exponent form is not an amount here.
	t.Run("the typed path", func(t *testing.T) {
		text := "1" + strings.Repeat("0", 302) + " water"
		_, problem := parseIngredientList(text, listRecipe, "crafting", "mymod-parts", baseWorld())
		want := `fkrecipes: mymod-parts, entry 1 ("` + text + `"): water is a fluid, and a recipe in the crafting category takes items only`
		if problem != want {
			t.Errorf("\n got: %s\nwant: %s", problem, want)
		}
	})
}

// The categories the engine accepts a fluid in are accepted here too, which is
// the other half of the rule: a check that refused everything would pass the
// table above and ship a library that cannot make a chemistry recipe.
func TestFluidIsAcceptedInEveryCategoryTheEngineAllows(t *testing.T) {
	for _, category := range []string{"advanced-crafting", "basic-crafting", "smelting", "crafting-with-fluid", "chemistry"} {
		t.Run(category, func(t *testing.T) {
			lib := New()
			plate := lib.Item("hardened-steel-plate", ItemSpec{})
			lib.Recipe(plate, RecipeSpec{
				Category:    category,
				Ingredients: []Ingredient{FluidIngredient(0.5, "water")},
			})
			if _, err := lib.PlanData(baseWorld()); err != nil {
				t.Errorf("a fluid in the %s category was refused: %s", category, err)
			}
		})
	}
}
