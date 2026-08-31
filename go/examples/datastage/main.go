// Command datastage is the Go example guest: a small steelworks expansion that
// exercises every verb this library has, and the Go arm of the mirror harness.
//
// IT IS THE RUST MIRROR'S TWIN. rust/examples/datastage is the same
// declarations in the same order, and scripts/run-mirror.sh packages both,
// runs each mod's settings and data stages under lua52f against one strict
// stand-in, and compares the two transcripts byte for byte. A difference
// between the two files may only be the language.
//
//	tinygo build -target=wasm-unknown -scheduler=none -gc=leaking -opt=2 \
//	    -o datastage.wasm .
//	fklua mod --data-module datastage.wasm --name fkrecipes-example ...
package main

import fkrecipes "github.com/Techrocket9/fkrecipes/go"

// plan declares the whole mod. Both stages call it, because the module is
// instantiated fresh per stage and nothing carries across: the settings stage
// needs the recipes to know which double setting backs a crafting time, and
// the data stage needs the settings to read them back.
func plan() *fkrecipes.Lib {
	lib := fkrecipes.New()

	hardened := lib.BoolSetting("hardened-tools", true)
	lib.IntSetting("rivet-batch", 4, fkrecipes.Between(1, 20))
	// A maximum and no minimum: the library generates the floor-safe minimum
	// beside the declared ceiling, so both bounds are in the golden and the
	// single-bound spec is exercised.
	forging := lib.DoubleSetting("forging-time", 3, fkrecipes.NumericSpec{HasMax: true, Max: 120})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
	bonuses := lib.BoolSetting("bonus-research", true)
	// Declared LAST on purpose: the generated order is derived from the
	// declaration index, so a new setting at the end leaves every existing
	// order alone.
	tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "projectile",
		[]string{"projectile", "military"})

	plate := lib.Item("hardened-steel-plate", fkrecipes.ItemSpec{
		Icon:        "__fkrecipes-example__/graphics/icons/hardened-steel-plate.png",
		IconSize:    64,
		StackSize:   100,
		DisplayName: "Hardened steel plate",
		Description: "Quenched and tempered, for tools that keep an edge.",
	})
	rivet := lib.Item("steel-rivet", fkrecipes.ItemSpec{
		Icon:        "__fkrecipes-example__/graphics/icons/steel-rivet.png",
		IconSize:    64,
		StackSize:   200,
		Subgroup:    "intermediate-product",
		DisplayName: "Steel rivet",
	})

	rivets := lib.Recipe(rivet, fkrecipes.RecipeSpec{
		CraftTime:   0.5,
		ResultCount: 4,
		Ingredients: []fkrecipes.Ingredient{
			fkrecipes.IngredientNamed(1, "iron-plate"),
		},
		DisplayName: "Steel rivets",
	})
	plates := lib.Recipe(plate, fkrecipes.RecipeSpec{
		CraftTimeFrom: forging,
		// The player picks what the plate is quenched in, and each medium is
		// a whole ingredient plan rather than one substituted line.
		IngredientsBy: &fkrecipes.IngredientChoices{
			Setting: medium,
			Choices: []fkrecipes.IngredientChoice{
				{Value: "water", Ingredients: []fkrecipes.Ingredient{
					// The ladder: tungsten is another mod's plate and is
					// absent from the stand-in, so the second rung answers
					// and the drop of the first is visible in the transcript.
					fkrecipes.IngredientNamed(2, "tungsten-plate", "steel-plate"),
					fkrecipes.IngredientOf(rivet, 4),
					// An optional hardener only an overhaul pack provides.
					// Neither candidate is in the stand-in, so the whole
					// ingredient is DROPPED with a log line rather than
					// guessed at, which is the other half of the ladder.
					fkrecipes.IngredientNamed(1, "tungsten-carbide", "titanium-plate"),
				}},
				// The organic bath: cheaper in rivets, and it wants an oil
				// this stand-in does not have, so the drop line fires on this
				// branch too. Enough survives that the water plan is not
				// reached for.
				{Value: "oil", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(2, "steel-plate"),
					fkrecipes.IngredientOf(rivet, 2),
					fkrecipes.IngredientNamed(1, "light-oil-barrel", "crude-oil-barrel"),
				}},
			},
		},
		Name:        "hardened-steel-plate-quenching",
		Category:    "smelting",
		DisplayName: "Hardened steel plate",
		Description: "Quench the plate, then temper it back to workable.",
	})

	// Reclaimed from worn plate, and known from the start: nothing unlocks it,
	// so it is enabled without research, and it names no crafting time, so the
	// engine's own default applies.
	lib.Recipe(rivet, fkrecipes.RecipeSpec{
		Name:        "salvaged-steel-rivet",
		ResultCount: 3,
		Ingredients: []fkrecipes.Ingredient{
			fkrecipes.IngredientOf(plate, 1),
		},
		DisplayName: "Salvaged steel rivets",
	})

	hardenedSteel := lib.Technology("hardened-steel", fkrecipes.TechSpec{
		Icon:        "__fkrecipes-example__/graphics/technology/hardened-steel.png",
		IconSize:    128,
		CostOf:      "logistics-2",
		After:       "steel-processing",
		Before:      "logistics-2",
		Unlocks:     []fkrecipes.RecipeRef{rivets, plates},
		EnabledBy:   hardened,
		DisplayName: "Hardened steel",
		Description: "Quenching steel plate to make it hold an edge.",
	})
	lib.Technology("steel-riveting", fkrecipes.TechSpec{
		Icon:     "__fkrecipes-example__/graphics/technology/steel-riveting.png",
		IconSize: 128,
		Unit: &fkrecipes.UnitSpec{
			Count:   45,
			Seconds: 20,
			Packs:   []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}},
		},
		AfterTech:   hardenedSteel,
		DisplayName: "Steel riveting",
	})
	// A bonus line the player prices for themselves. Each ladder is walked to
	// the first technology that is actually there and carries a cost, and THE
	// PREREQUISITE MOVES WITH THE UNIT: whichever source pays for this one
	// also becomes the thing it hangs off, so cost and tree position never
	// disagree.
	lib.Technology("hardened-tips", fkrecipes.TechSpec{
		Icon:     "__fkrecipes-example__/graphics/technology/hardened-tips.png",
		IconSize: 128,
		CostBy: &fkrecipes.CostChoices{
			Setting: tier,
			Choices: []fkrecipes.CostChoice{
				// The first rung is an overhaul pack's technology and is in
				// neither the stand-in nor the game, so the ladder steps past
				// it to the multi-level one, whose count_formula and level cap
				// come across with the unit.
				{Value: "projectile", Sources: []string{
					"tungsten-hardening", "physical-projectile-damage-7",
				}},
				{Value: "military", Sources: []string{"military-4"}},
			},
			// What applies when a ladder finds nothing at all: the technology
			// is still researchable, and still says so in the log.
			Fallback: fkrecipes.UnitSpec{
				Count:   200,
				Seconds: 30,
				Packs:   []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}},
			},
		},
		EnabledBy:   bonuses,
		DisplayName: "Hardened tool tips",
		Description: "Every level puts a harder edge on the same tools.",
	})

	return lib
}

//go:wasmexport fk_settings
func onSettings() { plan().Emit() }

//go:wasmexport fk_data
func onData() { plan().Emit() }

// TinyGo builds a reactor rather than a command: main never runs at a stage,
// and every //go:wasmexport traps until _initialize has. The stage files call
// it, and this stays empty.
func main() {}
