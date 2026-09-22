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
//
// THE ITEMS COME FIRST because the text settings name them. An ingredient list
// a player edits is declared with the list the mod would have used, and that
// list reaches this plan's own items through IngredientOf, which needs their
// handles. Declaration order inside each kind is what the emitted order
// strings derive from, and nothing here reorders either kind.
func plan() *fkrecipes.Lib {
	lib := fkrecipes.New()

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
		// A sort key, so the rivets sit beside the plate they fasten rather
		// than wherever the engine's name ordering puts them.
		Order: "b[steelworks]-a[rivet]",
	})
	chain := lib.Item("steel-chain", fkrecipes.ItemSpec{
		Icon:      "__fkrecipes-example__/graphics/icons/steel-chain.png",
		IconSize:  64,
		StackSize: 100,
		Subgroup:  "intermediate-product",
		Order:     "b[steelworks]-b[chain]",
	})

	hardened := lib.BoolSetting("hardened-tools", true)
	lib.IntSetting("rivet-batch", 4, fkrecipes.Between(1, 20))
	// A maximum and no minimum: the library generates the floor-safe minimum
	// beside the declared ceiling, so both bounds are in the golden and the
	// single-bound spec is exercised.
	forging := lib.DoubleSetting("forging-time", 3, fkrecipes.NumericSpec{HasMax: true, Max: 120})
	// The quenching medium. Its option list is exactly the two the mod
	// declares: the text setting beside it is what hands the whole ingredient
	// list to the player, and it adds no value here.
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water",
		[]string{"water", "oil"})
	bonuses := lib.BoolSetting("bonus-research", true)
	// Declared LAST on purpose: the generated order is derived from the
	// declaration index, so a new setting at the end leaves every existing
	// order alone.
	tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "projectile",
		[]string{"projectile", "military"})
	// A FLOOR AND NO CEILING, the one NumericSpec arm the goldens did not
	// carry. Organic here: a longer hold keeps tempering, so there is nothing
	// to cap, but below half a second the plate never reaches temperature.
	// Bound as a crafting time too, which is what shows that a DECLARED
	// minimum stands rather than being replaced by the generated floor-safe
	// one: the generated minimum fills in only where the consumer named none.
	tempering := lib.DoubleSetting("tempering-hold", 1.5,
		fkrecipes.NumericSpec{HasMin: true, Min: 0.5})

	// The customizer's settings, all appended after the ones above so no
	// existing order string moves. Each text setting's field starts out as the
	// word default and its description carries the list written out.

	// The whole ingredient list of one recipe, with no dropdown in front of
	// it: the simplest binding there is.
	rivetIngredients := lib.IngredientsSetting("rivet-ingredients", []fkrecipes.Ingredient{
		fkrecipes.IngredientNamed(1, "iron-plate"),
	})
	// The text setting that sits beside a dropdown, declared as the preset it
	// starts from so a player who types has somewhere to start from.
	quenchIngredients := lib.IngredientsSetting("quench-ingredients", []fkrecipes.Ingredient{
		fkrecipes.IngredientNamed(2, "tungsten-plate", "steel-plate"),
		fkrecipes.IngredientOf(rivet, 4),
		fkrecipes.IngredientNamed(1, "tungsten-carbide", "titanium-plate"),
	})
	chainLinks := lib.DropdownSettingNeedingLocale("chain-links", "short",
		[]string{"short", "long"})
	chainIngredients := lib.IngredientsSetting("chain-ingredients", []fkrecipes.Ingredient{
		fkrecipes.IngredientOf(rivet, 4),
	})
	// A research cost the player prices: the packs as text, the count and the
	// seconds as numbers, each with the minimum the engine's own floors need.
	tipsPacks := lib.PacksSetting("tips-packs", []fkrecipes.Pack{
		{Name: "automation-science-pack", Amount: 1},
		{Name: "military-science-pack", Amount: 1},
	})
	// BOTH DEFAULT TO 0 AND BOTH FLOOR AT 0, because a research dropdown sits
	// beside them: 0 is a number's way of saying the reserved word, and it
	// means the tier the dropdown chose supplies that field.
	tipsCount := lib.IntSetting("tips-count", 0, fkrecipes.Between(0, 100000))
	tipsSeconds := lib.IntSetting("tips-seconds", 0, fkrecipes.Between(0, 600))
	chainPacks := lib.PacksSetting("chain-packs", []fkrecipes.Pack{
		{Name: "automation-science-pack", Amount: 1},
	})
	// NO DROPDOWN BESIDE THESE TWO, so there is nothing to defer to and each
	// declares a minimum of at least 1: the engine refuses a unit count of 0
	// and a unit time of 0.
	chainCount := lib.IntSetting("chain-count", 20, fkrecipes.Between(1, 100000))
	chainSeconds := lib.IntSetting("chain-seconds", 10, fkrecipes.Between(1, 600))

	// THE SCAFFOLDING LINE, AND IT IS THE SHAPE A MIGRATING MOD HAS: one
	// dropdown that moves a whole tier at once, named by TWO recipes and one
	// technology, under names and orders this mod shipped before it adopted
	// the library. A dropdown composes ONE declaration's presets, so the
	// first recipe carries Describes and the tooltip a player reads is the
	// bill they can paste into the text field beside it.
	scaffoldTier := lib.LegacyDropdownSettingNeedingLocale("steelworks-scaffold-tier",
		"light", []string{"light", "heavy"}, "za")
	// ITS ORDER PUTS IT ABOVE THE DROPDOWN AND THE PACK TEXT BELOW IT, which
	// is what lets the dropdown's own switch line say WHICH of the two texts
	// it defers to: the word is above or below, so two texts on one side of it
	// would be one sentence about either.
	scaffoldParts := lib.LegacyIngredientsSetting("steelworks-scaffold-parts",
		[]fkrecipes.Ingredient{fkrecipes.IngredientNamed(2, "iron-plate")}, "ya")
	scaffoldPacks := lib.LegacyPacksSetting("steelworks-scaffold-packs",
		[]fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}}, "zc")
	// BOTH FLOOR AT 0 AND DEFAULT TO 0, because the tier dropdown decides
	// while they do: the same rule the tips numbers follow.
	scaffoldCount := lib.LegacyIntSetting("steelworks-scaffold-count", 0,
		fkrecipes.Between(0, 100000), "zd")
	scaffoldSeconds := lib.LegacyIntSetting("steelworks-scaffold-seconds", 0,
		fkrecipes.Between(0, 600), "ze")

	rivets := lib.Recipe(rivet, fkrecipes.RecipeSpec{
		CraftTime:   0.5,
		ResultCount: 4,
		// The list the player owns. The declared default is what the word
		// default in the field means, and it is the fixed list this recipe
		// used to carry.
		IngredientsFrom: rivetIngredients,
		DisplayName:     "Steel rivets",
		// A REAL 2.0 RECIPE FIELD this library has no slot for, passed
		// through verbatim. That is what Extra is: the library emits what it
		// knows and gets out of the way for the rest, rather than growing a
		// field per prototype property the engine has.
		Extra: []fkrecipes.KV{fkrecipes.Pair("allow_productivity", fkrecipes.Bool(true))},
	})
	plates := lib.Recipe(plate, fkrecipes.RecipeSpec{
		CraftTimeFrom: forging,
		// The player picks what the plate is quenched in, and each medium is
		// a whole ingredient plan rather than one substituted line. The text
		// setting below takes the list over entirely whenever it is not on the
		// reserved word.
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
		IngredientsFrom: quenchIngredients,
		Name:            "hardened-steel-plate-quenching",
		// A CATEGORY THAT TAKES FLUIDS, because the player may now type one.
		// The engine refuses a fluid in the crafting category (measured) and
		// this library refuses it first, so a customizable recipe that wants
		// to allow water has to say where it is crafted.
		Category:    "crafting-with-fluid",
		Order:       "b[steelworks]-b[quenching]",
		DisplayName: "Hardened steel plate",
		Description: "Quench the plate, then temper it back to workable.",
	})

	// Reclaimed from worn plate, and known from the start: nothing unlocks it,
	// so it is enabled without research.
	lib.Recipe(rivet, fkrecipes.RecipeSpec{
		Name:          "salvaged-steel-rivet",
		CraftTimeFrom: tempering,
		ResultCount:   3,
		Ingredients: []fkrecipes.Ingredient{
			fkrecipes.IngredientOf(plate, 1),
		},
		DisplayName: "Salvaged steel rivets",
	})

	chains := lib.Recipe(chain, fkrecipes.RecipeSpec{
		CraftTime: 2,
		Category:  "crafting-with-fluid",
		IngredientsBy: &fkrecipes.IngredientChoices{
			Setting: chainLinks,
			Choices: []fkrecipes.IngredientChoice{
				{Value: "short", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientOf(rivet, 4),
				}},
				{Value: "long", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientOf(rivet, 8),
					fkrecipes.IngredientNamed(1, "steel-plate"),
				}},
			},
		},
		IngredientsFrom: chainIngredients,
		DisplayName:     "Steel chain",
		Description:     "Links of rivets",
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
	// A bonus line the player can price for themselves. Each ladder is walked
	// to the first technology that is actually there and carries a cost, and
	// THE PREREQUISITE MOVES WITH THE UNIT: whichever source pays for this one
	// also becomes the thing it hangs off, so cost and tree position never
	// disagree. The three settings beside it overwrite the chosen tier's
	// numbers one field at a time, and the tier still places the technology.
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
				// WHAT THIS TIER COSTS, IN THIS MOD'S OWN WORDS. The
				// settings stage sees mods and never data.raw, so without
				// this the line would name the ladder's FIRST rung,
				// tungsten-hardening, which is an overhaul pack's technology
				// and is in no game most players run. Display is where an
				// author says the true thing per mod set.
				{Value: "projectile", Display: "as much as the seventh projectile damage level, or the overhaul pack's own hardening", Sources: []string{
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
		CostFrom: &fkrecipes.CustomCost{
			Packs:   tipsPacks,
			Count:   tipsCount,
			Seconds: tipsSeconds,
		},
		EnabledBy:   bonuses,
		DisplayName: "Hardened tool tips",
		Description: "Every level puts a harder edge on the same tools.",
	})
	// The other half of the custom cost: no dropdown at all, so the three
	// settings are the whole price and the ordinary placement fields say where
	// the technology goes.
	lib.Technology("chain-forging", fkrecipes.TechSpec{
		Icon:     "__fkrecipes-example__/graphics/technology/chain-forging.png",
		IconSize: 128,
		CostFrom: &fkrecipes.CustomCost{
			Packs:   chainPacks,
			Count:   chainCount,
			Seconds: chainSeconds,
		},
		After:   "steel-processing",
		Unlocks: []fkrecipes.RecipeRef{chains},
	})

	// THE TWO RECIPES THE TIER MOVES. Both produce an item this plan already
	// declares, under explicit names of their own, and only the FIRST carries
	// a text setting: a dropdown composes one declaration's presets and
	// Describes says which.
	bracket := lib.Recipe(rivet, fkrecipes.RecipeSpec{
		Name:        "scaffold-bracket",
		CraftTime:   0.5,
		ResultCount: 2,
		IngredientsBy: &fkrecipes.IngredientChoices{
			Setting: scaffoldTier,
			Choices: []fkrecipes.IngredientChoice{
				{Value: "light", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(2, "iron-plate"),
				}},
				{Value: "heavy", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientNamed(4, "steel-plate"),
				}},
			},
			// THIS IS THE DECLARATION THE DROPDOWN SHOWS. Without it the
			// technology below would describe it, because that walk runs
			// second, and the tooltip would carry research costs over a field
			// a player fills in with an ingredient list.
			Describes: true,
		},
		IngredientsFrom: scaffoldParts,
		DisplayName:     "Scaffold bracket",
		// A SECOND RECIPE FOR AN ITEM THAT ALREADY HAS ONE, and the field
		// that keeps the RECYCLER off it. Measured in quality's own
		// prototypes/recycling.lua: it builds one recycling recipe per item
		// out of the LAST recipe that produces it, and the only opt-out is
		// auto_recycle = false on the recipe. Without it an alternative
		// recipe added beside the primary one silently moves that generated
		// prototype. Nothing in this library owns the field; it is passed
		// through, which is what Extra is for.
		Extra: []fkrecipes.KV{fkrecipes.Pair("auto_recycle", fkrecipes.Bool(false))},
	})
	tie := lib.Recipe(chain, fkrecipes.RecipeSpec{
		Name:      "scaffold-tie",
		CraftTime: 1,
		IngredientsBy: &fkrecipes.IngredientChoices{
			Setting: scaffoldTier,
			Choices: []fkrecipes.IngredientChoice{
				{Value: "light", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientOf(rivet, 2),
				}},
				{Value: "heavy", Ingredients: []fkrecipes.Ingredient{
					fkrecipes.IngredientOf(rivet, 4),
					fkrecipes.IngredientNamed(1, "steel-plate"),
				}},
			},
		},
		DisplayName: "Scaffold tie",
		Extra:       []fkrecipes.KV{fkrecipes.Pair("auto_recycle", fkrecipes.Bool(false))},
	})
	// THE THIRD DECLARATION ON THE SAME DROPDOWN, and the one that makes the
	// pair worth having in the fixture: its pack text sits beside a dropdown
	// that composes the INGREDIENT ladder sentence, so it keeps its own packs
	// sentence. Two vocabularies, one dropdown.
	lib.Technology("scaffold-raising", fkrecipes.TechSpec{
		Icon:     "__fkrecipes-example__/graphics/technology/scaffold-raising.png",
		IconSize: 128,
		CostBy: &fkrecipes.CostChoices{
			Setting: scaffoldTier,
			Choices: []fkrecipes.CostChoice{
				{Value: "light", Sources: []string{"logistics-2"}},
				{Value: "heavy", Sources: []string{"logistics-3"}},
			},
			Fallback: fkrecipes.UnitSpec{
				Count:   100,
				Seconds: 15,
				Packs:   []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}},
			},
		},
		CostFrom: &fkrecipes.CustomCost{
			Packs:   scaffoldPacks,
			Count:   scaffoldCount,
			Seconds: scaffoldSeconds,
		},
		Unlocks:     []fkrecipes.RecipeRef{bracket, tie},
		DisplayName: "Scaffold raising",
	})

	// TWO DESCRIPTIONS THE PLAN WRITES rather than the locale file. A .cfg
	// entry is one string for every mod set; a plan that branches on what is
	// installed can say the true thing for the game actually running, and
	// this is where that goes. The bool has nothing composed onto it, so the
	// literal is its whole tooltip; the ingredient text has the library's own
	// lines under it exactly as it would under a locale key.
	lib.DescribeSetting(hardened, "Adds the hardened steel line, its scaffolding and the research that unlocks them.")
	lib.DescribeSetting(scaffoldParts, "What one scaffold bracket is made of while this is not on default.")

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
