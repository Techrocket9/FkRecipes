// Command notext is the size-measurement fixture: a plan shaped like
// BetterBeltBalancer's before its customizer round, and NOT a mirror arm.
//
// Two legacy dropdowns drive IngredientsBy and CostBy over legacy prototypes,
// and no text setting is declared anywhere. That is the shape that says
// whether a consumer who never calls IngredientsSetting or PacksSetting ships
// the ingredient language: package this guest with fklua and compare
// fk_data_module.lua's line and byte counts against the CURRENT figures, which
// are the four-fixture table under "The claim gets its adjective" in Fix round
// 1b of agents/implementation-notes.md, with a runnable recipe beside them. The
// follow-up subsection's older table is the seam's own before and after,
// measured at c7a806e against an fklua at a1fcd04, and neither half of it is a
// figure for this tree. The counts are the oracle; a grep for the
// language's function names is only a secondary signal, because TinyGo inlines
// a single-caller function and its header disappears from the module while its
// code stays.
//
// It is Go only because the question it answers is one whole-program
// elimination answers per toolchain, and the Go module is the one the pilot
// ships. Nothing in scripts/ builds or runs it; `go vet .` in this directory is
// the gate that keeps it compiling, and it plans without refusal on a fitted
// World. PlaceResult is left off the item on purpose: the pilot's item names
// an entity the pilot extends by hand before Emit, and nothing here does.
//
//	tinygo build -target=wasm-unknown -scheduler=none -gc=leaking -opt=2 \
//	    -o notext.wasm .
//	fklua mod --data-module notext.wasm --name better-belt-balancer ...
package main

import fkrecipes "github.com/Techrocket9/fkrecipes/go"

var recipeOptions = []string{"vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express"}

var techOptions = []string{"logistics", "logistics-2", "logistics-3"}

func plan() *fkrecipes.Lib {
	lib := fkrecipes.New()

	recipeCost := lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla", recipeOptions, "a")
	techCost := lib.LegacyDropdownSettingNeedingLocale("bbb-tech-cost", "logistics", techOptions, "b")

	part := lib.LegacyItem("bbb-balancer-part", fkrecipes.ItemSpec{
		Icon:      "__better-belt-balancer__/graphics/icons/balancer-part.png",
		IconSize:  64,
		StackSize: 50,
		Subgroup:  "belt",
		Order:     "c[splitter]-y[bbb-balancer]",
	})
	recipe := lib.LegacyRecipe(part, "bbb-balancer-part", fkrecipes.RecipeSpec{
		CraftTime: 1,
		Order:     "c[splitter]-y[bbb-balancer]",
		IngredientsBy: &fkrecipes.IngredientChoices{
			Setting: recipeCost,
			Choices: []fkrecipes.IngredientChoice{
				{Value: "vanilla", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(4, "iron-plate"),
					fkrecipes.IngredientNamed(2, "iron-gear-wheel"),
					fkrecipes.IngredientNamed(2, "transport-belt"),
				}},
				{Value: "cheap", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(2, "iron-plate"),
					fkrecipes.IngredientNamed(1, "transport-belt"),
				}},
				{Value: "belt-fast", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(2, "iron-plate"),
					fkrecipes.IngredientNamed(1, "fast-transport-belt"),
				}},
				{Value: "belt-express", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(2, "iron-plate"),
					fkrecipes.IngredientNamed(1, "express-transport-belt"),
				}},
				{Value: "splitter", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(3, "iron-plate"),
					fkrecipes.IngredientNamed(1, "splitter"),
				}},
				{Value: "splitter-express", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(3, "iron-plate"),
					fkrecipes.IngredientNamed(1, "express-splitter", "fast-splitter", "splitter"),
				}},
			},
		},
	})
	lib.LegacyTechnology("bbb-balancer", fkrecipes.TechSpec{
		Icon:     "__better-belt-balancer__/graphics/icons/balancer-part.png",
		IconSize: 64,
		Order:    "a-b-bbb",
		Unlocks:  []fkrecipes.RecipeRef{recipe},
		CostBy: &fkrecipes.CostChoices{
			Setting: techCost,
			Choices: []fkrecipes.CostChoice{
				{Value: "logistics", Sources: []string{"logistics"}},
				{Value: "logistics-2", Sources: []string{"logistics-2", "logistics"}},
				{Value: "logistics-3", Sources: []string{"logistics-3", "logistics-2", "logistics"}},
			},
			Fallback: fkrecipes.UnitSpec{
				Count:   20,
				Seconds: 15,
				Packs:   []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}},
			},
		},
	})
	return lib
}

//go:wasmexport fk_settings
func onSettings() { plan().Emit() }

//go:wasmexport fk_data
func onData() { plan().Emit() }

// TinyGo builds a reactor rather than a command: main never runs at a stage,
// and every //go:wasmexport traps until _initialize has.
func main() {}
