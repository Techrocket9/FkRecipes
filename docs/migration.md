# Migrating a mod that already ships settings

FkRecipes generates setting names from the mod it is packaged as, and it generates `order` strings from declaration order. That is right for a new mod and wrong for one that already has players: Factorio persists startup values in `mod-settings.dat` keyed by name, with no rename mechanism, so a setting that changes name is a setting that silently reverts to its default in every existing save and server config.

The same is true of prototype names. A save references an item by name in every inventory and on every belt, a technology by name in its researched list, a recipe by name in every assembler, and your own hand-rolled entity names its item through `place_result`. The engine's answer to a name that moved is not a warning:

```
Error in assignID: item with name 'bbb-balancer-part' does not exist.
```

This document is about the surfaces that let an existing mod adopt the library while keeping every name a player or a save can observe: settings and prototypes it keeps verbatim, ingredients a dropdown chooses, and a research cost a dropdown chooses. Names are what is preserved. Adoption is not free in other respects, and the [README](../README.md) carries the measured download and parse cost. The last section is for a mod that has already adopted the library and is taking a newer version of it, where what changes is what your own tests assert rather than what your declarations say. It assumes you have read [Using FkRecipes](usage.md).

## Keeping the names you already ship

Four constructors take a **full** name and an explicit order, and emit both verbatim. No prefix is added.

```go
recipeCost := lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
	[]string{"vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express"}, "a")
techCost := lib.LegacyDropdownSettingNeedingLocale("bbb-tech-cost", "logistics",
	[]string{"logistics", "logistics-2", "logistics-3"}, "b")
lib.LegacyBoolSetting("bbb-multi-edge-parts", false, "a")
lib.LegacyIntSetting("bbb-batch", 4, fkrecipes.Between(1, 20), "c")
lib.LegacyDoubleSetting("bbb-speed", 2.5, fkrecipes.NumericSpec{}, "d")
```

```rust
let recipe_cost = lib.legacy_dropdown_setting_needing_locale(
    "bbb-recipe-cost",
    "vanilla",
    &["vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express"],
    "a",
);
let tech_cost = lib.legacy_dropdown_setting_needing_locale(
    "bbb-tech-cost",
    "logistics",
    &["logistics", "logistics-2", "logistics-3"],
    "b",
);
lib.legacy_bool_setting("bbb-multi-edge-parts", false, "a");
lib.legacy_int_setting("bbb-batch", 4, NumericSpec::between(1.0, 20.0), "c");
lib.legacy_double_setting("bbb-speed", 2.5, NumericSpec::default(), "d");
```

The handles they return are ordinary typed handles. `EnabledBy`, `CraftTimeFrom`, `IngredientsBy` and `CostBy` take them exactly as they take a generated setting's, and everything the library does with a bound setting still applies: a legacy double bound as a crafting time is still given the 0.002 minimum, because the engine's floor does not care where the name came from.

The order is a parameter rather than derived because a mod that already shipped chose one, and because a generated order would reshuffle the settings screen for existing players. An empty order string is refused: it is not a choice, it is an omission.

Two more refusals are worth knowing. An empty name is refused as it is for a generated setting. Duplicates are checked on the **emitted** names, which is the namespace the engine actually keeps, so a legacy name and a generated one that arrive at the same string are caught even though the declarations differ:

```
fkrecipes: two settings share the name steelworks-hardened-tools; the engine keeps the last one silently
```

`CheckLocale` reads legacy settings under the names they actually carry, and keys their dropdown values `<legacy-name>-<value>` rather than `<prefix><legacy-name>-<value>`, so the locale file you already ship keeps validating. Once your names stop carrying the mod prefix, though, its orphan scan has nothing to work with: use `CheckLocaleWith` instead, as the worked example below does.

## Keeping the prototype names you already ship

The same three constructors exist for prototypes, and the argument is stronger: a setting reverts to its default when renamed, but a prototype reference DANGLES.

```go
part := lib.LegacyItem("bbb-balancer-part", fkrecipes.ItemSpec{
	Icon:        "__better-belt-balancer__/graphics/icons/balancer-part.png",
	IconSize:    64,
	StackSize:   50,
	Order:       "z[balancer]-a[part]",
	PlaceResult: "bbb-balancer-1-to-2",
})
lib.LegacyRecipe(part, "bbb-balancer-part", fkrecipes.RecipeSpec{
	Ingredients: []fkrecipes.Ingredient{
		fkrecipes.IngredientNamed(2, "transport-belt"),
		fkrecipes.IngredientNamed(1, "splitter"),
	},
})
lib.LegacyTechnology("bbb-balancer", fkrecipes.TechSpec{
	CostOf:  "logistics-2",
	After:   "logistics-2",
	Unlocks: []fkrecipes.RecipeRef{recipe},
	Order:   "z-b-a",
})
```

```rust
let part = lib.legacy_item(
    "bbb-balancer-part",
    ItemSpec {
        icon: "__better-belt-balancer__/graphics/icons/balancer-part.png".into(),
        icon_size: 64,
        stack_size: 50,
        order: "z[balancer]-a[part]".into(),
        place_result: "bbb-balancer-1-to-2".into(),
        ..Default::default()
    },
);
```

The names are emitted verbatim, with no prefix. The handles are ORDINARY handles: `IngredientOf`, `Unlocks`, `AfterTech` and the splices take them exactly as they take a generated declaration's, so a plan can be part legacy and part generated and neither half needs to know.

`PlaceResult` is the field that makes this concrete for BetterBeltBalancer. Its item is built by a hand-rolled `simple-entity-with-force` that names the item back, so a renamed item is the `assignID` abort quoted at the top of this page. The entity stays hand-rolled and is extended before the `Emit` call runs, because the probe runs inside that call; the item names it through `PlaceResult`, which is presence probed: an entity that is not there is refused at plan time with the item and the entity in the message rather than aborting the load.

Duplicate scans and the overwrite refusals run on the EMITTED names, which is the namespace the engine keeps. A legacy name that collides with a generated one is caught even though the declarations differ:

```
fkrecipes: two items share the name better-belt-balancer-balancer-part; the second would overwrite the first
```

## Ingredients a dropdown chooses

A hand-rolled mod that varies a recipe by setting usually ends up with a branch per option around one `data:extend`. `IngredientsBy` is that branch, declared once.

```go
lib.Recipe(plate, fkrecipes.RecipeSpec{
	IngredientsBy: &fkrecipes.IngredientChoices{
		Setting: medium,
		Choices: []fkrecipes.IngredientChoice{
			{Value: "water", Ingredients: []fkrecipes.Ingredient{
				fkrecipes.IngredientNamed(2, "tungsten-plate", "steel-plate"),
				fkrecipes.IngredientOf(rivet, 4),
			}},
			{Value: "oil", Ingredients: []fkrecipes.Ingredient{
				fkrecipes.IngredientNamed(2, "steel-plate"),
				fkrecipes.IngredientOf(rivet, 2),
			}},
		},
	},
})
```

```rust
lib.recipe(
    plate,
    RecipeSpec {
        ingredients_by: Some(IngredientChoices {
            setting: medium,
            choices: vec![
                IngredientChoice {
                    value: "water".into(),
                    ingredients: vec![
                        Ingredient::named(2, "tungsten-plate", &["steel-plate"]),
                        Ingredient::of(rivet, 4),
                    ],
                },
                IngredientChoice {
                    value: "oil".into(),
                    ingredients: vec![
                        Ingredient::named(2, "steel-plate", &[]),
                        Ingredient::of(rivet, 2),
                    ],
                },
            ],
        }),
        ..Default::default()
    },
);
```

Each choice is a whole ingredient plan, not one substituted line, and every plan is resolved by the ordinary ladder rules: `IngredientOf` always resolves, and `IngredientNamed` walks its candidates and drops the ingredient with a log line if none is present.

The values must equal the setting's allowed values, in the same order. A missing value, an extra one, or one out of place is refused by name:

```
fkrecipes: the recipe hardened-steel-plate offers nothing for the value oil that the setting steelworks-quench-medium allows
```

That rule exists because the alternative is a recipe with no ingredients the first time somebody adds a dropdown value and forgets the recipe. `IngredientsBy` and `Ingredients` are mutually exclusive; naming both is refused.

If the chosen plan named ingredients and none of them resolved, the recipe would be made of nothing, so the **default** option's plan applies instead, with a line saying so. The default is the answer the mod ships as its own:

```
fkrecipes: hardened-steel-plate: the oil ingredients name nothing this game has, so the water ingredients apply
```

If the default resolves to nothing either, the recipe is emitted with an empty ingredient list and every drop is on the record. The load completes and says what happened rather than failing.

## A research cost a dropdown chooses

`CostBy` is `CostOf` with a ladder per dropdown value. Each ladder is walked in order to the first technology that exists, is not a `research_trigger` technology, and carries a dictionary unit. That unit is copied verbatim along with the source's `max_level`, **and the source becomes the technology's sole prerequisite**.

```go
lib.Technology("hardened-tips", fkrecipes.TechSpec{
	CostBy: &fkrecipes.CostChoices{
		Setting: tier,
		Choices: []fkrecipes.CostChoice{
			{Value: "projectile", Sources: []string{"tungsten-hardening", "physical-projectile-damage-7"}},
			{Value: "military", Sources: []string{"military-4"}},
		},
		Fallback: fkrecipes.UnitSpec{
			Count:   200,
			Seconds: 30,
			Packs:   []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}},
		},
	},
})
```

```rust
lib.technology(
    "hardened-tips",
    TechSpec {
        cost_by: Some(CostChoices {
            setting: tier,
            choices: vec![
                CostChoice {
                    value: "projectile".into(),
                    sources: vec!["tungsten-hardening".into(), "physical-projectile-damage-7".into()],
                },
                CostChoice { value: "military".into(), sources: vec!["military-4".into()] },
            ],
            fallback: UnitSpec {
                count: 200,
                seconds: 30.0,
                packs: vec![Pack::new("automation-science-pack", 1)],
            },
        }),
        ..Default::default()
    },
);
```

Cost and tree position come from one named point, which is why `CostBy` does not combine with `After`, `Before` or `AfterTech`. Naming a placement beside it is refused:

```
fkrecipes: the technology hardened-tips names CostBy with a placement; the prerequisite moves with the unit, so CostBy places the technology itself
```

The ladder is how you write "price this like the tier the player asked for, whichever of those technologies this particular game happens to have". A rung that is absent, that is a `research_trigger` technology, or that carries a unit this library cannot copy faithfully is stepped past rather than refused: the player's install is not something your mod can validate at declaration time. A rung the ladder DOES settle on can still turn out unusable later, because its science packs are filtered and its pack list has to be readable; that is the `Fallback`'s second job and the paragraph under [Upgrading a mod that has already adopted the library](#upgrading-a-mod-that-has-already-adopted-the-library) says when each one is asked about.

If no rung in the chosen ladder works out, the `Fallback` unit is emitted and the technology has no prerequisite, with a line saying so:

```
fkrecipes: hardened-tips: no source for the projectile cost carries a unit, so the fallback cost applies and the technology has no prerequisite
```

The fallback is held to exactly the same rules as a hand-rolled `Unit`, because it is the cost that applies when nothing else does. The rules about its own NUMBERS are checked before anything is emitted and whether or not any ladder ever lands on it: a count below 1, a research time at or below zero, a declared amount above a ceiling, an empty rung name, a repeated pack and a pack list with nothing in it are all refused at plan time. Its SCIENCE PACKS are a different rule and are not checked then, because that is a question about the player's mod set rather than about your declaration: each pack is a ladder, a ladder whose rungs are all absent is dropped with a log line, and only a unit left with no pack at all refuses, which happens while the plan resolves rather than while it is validated. Which is also to say that a pack the game does not have costs you nothing until the fallback is the cost that applies; the paragraph below the upgrade list says when that is.

The values must equal the setting's allowed values in order, the same as `IngredientsBy`, and `CostBy` is mutually exclusive with both `CostOf` and `Unit`.

One behaviour to expect when a hand-rolled cost copied three fields by hand: `CostOf` and `CostBy` copy the source's whole `unit`, and a source that carries `max_level` brings it across, so the migrated technology becomes multi-level exactly as its source is. That is the charter's rule (cost and tree position from one named point, the level cap included), and a mod that wants a single-level technology priced like a multi-level one names a single-level source or writes a `Unit`.

## Adding a customizer to a dropdown you already ship

**Adopting the customizer on a dropdown you already ship is an identity.** The dropdown keeps its exact option list, so no stored choice is reset and none changes meaning; the text setting is purely additive, so a player who never opens the settings screen sees the load they always had; a rollback to an older release loses nothing, because the release that does not declare the text setting simply leaves that setting alone (the engine keeps every setting a release does not declare, measured on Factorio 2.0.77); and a return to the new release restores everything, because the value the rollback did not touch is still in `mod-settings.dat`.

That property rests on one rule: **the text is the switch.** While the text setting says the word `default` the dropdown decides exactly as it always did, and anything else is what applies instead. The library adds no value to the dropdown and reserves none. See [The ingredient list](ingredient-list.md) for what the player types.

```go
cost := lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
	[]string{"vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express"}, "a")
parts := lib.IngredientsSetting("balancer-part-ingredients", vanillaIngredients)
lib.LegacyRecipe(part, "bbb-balancer-part", fkrecipes.RecipeSpec{
	IngredientsBy:   &fkrecipes.IngredientChoices{Setting: cost, Choices: presets},
	IngredientsFrom: parts,
	CraftTime:       1,
})
```

```rust
let cost = lib.legacy_dropdown_setting_needing_locale("bbb-recipe-cost", "vanilla",
    &["vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express"], "a");
let parts = lib.ingredients_setting("balancer-part-ingredients", vanilla_ingredients);
lib.legacy_recipe(part, "bbb-balancer-part", RecipeSpec {
    ingredients_by: Some(IngredientChoices { setting: cost, choices: presets }),
    ingredients_from: Some(parts),
    craft_time: 1.0,
    ..Default::default()
});
```

The text setting is a new name, so it carries the generated prefix and the word `default` as its text. Where it lands in the settings screen is a separate decision: a generated setting's order string comes from its declaration index, and next to legacy orders such as `a` and `b` that puts it between the two whatever you meant. `OrderAfter("a")` (`order_after("a")`) before the declaration places it behind the dropdown it belongs to, under the order `aab`; see [Using FkRecipes](usage.md) for the rule.

What the engine does not allow is filling that text from the player's old choice. The settings stage, where defaults are declared, cannot see any stored value (measured on Factorio 2.0.77: `data.raw` is empty there), and nothing at the data stage or at runtime can write a startup setting. A default is therefore one text for every player. What the library does instead is compose the dropdown's description: your own `[mod-setting-description]` entry, then two lines per preset, its localised label and then an indented line opening with `type:` carrying the preset rendered as an ingredient list, so a player on `cheap` who starts typing can see what `cheap` was and copy it. A last line says that the text setting above or below it applies while it does not say `default`, and the text setting's own description carries the mirror of that sentence. The settings screen has no conditional visibility at all (measured on 2.0.77), so those two lines are the only place the pairing can be stated.

Two locale entries are new: `[mod-setting-name]` and `[mod-setting-description]` for the text setting, where your description says what the setting is for. The format, the 2000-character limit, the switch line and what happens to a text the library cannot use are composed under it for you, so the entry does not have to teach the syntax. `CheckLocaleWith` reports both when they are missing, and the dropdown's own description becomes required at the same time, since the preset list is composed onto it.

One thing to know about the labels you already ship: a dropdown label such as `Default: 4 iron plates, 2 gears, 2 transport belts` speaks display names, while the text field beside it takes internal names, so a player who copies the label into the field gets your own list back with an error line in the log. The composition keeps the two apart. Each preset is two lines in the tooltip, your label and then an indented line opening with `to type:` holding that preset in internal names, and that second line is the only one in the tooltip a player is invited to copy. You can leave your labels in display names.

```
[mod-setting-name]
better-belt-balancer-balancer-part-ingredients=Balancer part ingredients
[mod-setting-description]
better-belt-balancer-balancer-part-ingredients=What a balancer part is made of while this is not on default. Write the amount, then the item name, and separate ingredients with commas: 2 iron-plate, 3 copper-cable
```

This is the step whose settings hash is expected to move, because the dropdown's description becomes a composed value and three new setting prototypes arrive, while the data hash stays where it was for every preset: the presets are the same plans as before, and a typed list is only read for a player who typed one.

**Why the option list may not grow.** Factorio resets a stored dropdown value that is not in the running release's `allowed_values` to that release's default, before any stage runs, with no line in the log, and writes the reset back to `mod-settings.dat`; every setting a release does not declare comes through untouched. Both measured on Factorio 2.0.77. So a release that answered "the text is in force" with a dropdown value would lose that answer the first time a player ran an older release, on any rollback, in a modpack pinned to an older version, or on a second machine still on the last public release, and would lose it silently: the typed text survives, because it belongs to a setting the old release does not declare, so the field still shows the player's own recipe while the dropdown beside it has gone back to a preset and the screen looks like nothing was lost. Keeping the state in the TEXT is what makes the whole step an identity instead.

**And why withdrawing a value costs the same.** The engine's rule is about the running release's `allowed_values` and nothing else, so removing a value a release already shipped is the same defect run backwards: a player who stored that value and updates has it reset to the default, silently and permanently, exactly as a player who stored a new value and rolls back does. Nothing in the library can tell the two directions apart, because a declaration says which values exist now and never says which ones existed before. If you have already shipped a dropdown value and want it gone, the withdrawal belongs in your release notes, naming the value and what a player who was on it will find instead.

## A research cost the player can reprice

A `CostBy` dropdown takes the same treatment, with `CostFrom` beside it: the dropdown chooses a tier and the three settings overwrite that tier's numbers one field at a time, so a field left at its declared default comes from the tier and a player who touches nothing gets the tier byte for byte. Beside a dropdown, 0 is what a number says instead of the reserved word, so the count and the seconds settings each declare a default of 0, a minimum of 0 and a maximum. **The tier still places the technology**, so nothing about the tree moves when a player reprices the research: the source whose cost the tier names is the prerequisite whatever the settings say.

One rule reaches a migrated `CostBy` either way: a `Fallback` unit must name at least one science pack, because a unit declared with none is refused at plan time even when no ladder ever reaches it. The pack is a ladder, so a fallback that names `automation-science-pack` with a rung behind it survives a modpack that renames the pack. The three settings want to sit under their dropdown, and with a legacy order `b` on the dropdown they would not: their generated orders would sort between `a` and `b`, above it. `OrderAfter("b")` before the three declarations gives them `bad`, `bae` and `baf`, under `b` and before `c`.

## A worked example: BetterBeltBalancer

[BetterBeltBalancer](https://github.com/Techrocket9/BetterBeltBalancer) is the pilot for this path, and it is a good example because its names cannot be regenerated.

The mod is packaged as `better-belt-balancer`, so the prefix this library would derive is `better-belt-balancer-`. All three of its settings use the historical `bbb-` prefix instead:

| Setting | Type | Scope | Default | Allowed values | Order |
| --- | --- | --- | --- | --- | --- |
| `bbb-recipe-cost` | string dropdown | startup | `vanilla` | `vanilla`, `cheap`, `belt-fast`, `belt-express`, `splitter`, `splitter-express` | `a` |
| `bbb-tech-cost` | string dropdown | startup | `logistics` | `logistics`, `logistics-2`, `logistics-3` | `b` |
| `bbb-multi-edge-parts` | bool | runtime-global | `false` | | `a` |

Every one of those names would change under the generated scheme, and Factorio persists startup values in `mod-settings.dat` by name with no rename mechanism. Renaming them resets `bbb-recipe-cost` and `bbb-tech-cost` to their defaults in every existing save and every server config, silently. The four legacy constructors exist so that does not happen, and the acceptance criterion for the migration is BetterBeltBalancer's own `mod_settings_sha256` golden: the settings prototypes have to hash to what they hashed to before.

Two facts about that mod shape the migration beyond the names:

`bbb-multi-edge-parts` is **runtime-global**, not startup, and is declared only on Factorio 2.0 engines. This library generates startup settings only, and reads only startup settings, so that one stays hand-rolled. A mod can declare some settings through FkRecipes and others itself, and the library validates what it was told about.

Use `CheckLocaleWith` rather than `CheckLocale` in that arrangement, and pass the hand-rolled setting:

```go
findings := plan().CheckLocaleWith("better-belt-balancer", cfg,
	[]string{"bbb-multi-edge-parts"})
```

That is the whole difference, and it is what makes the check complete for a migrated mod. Plain `CheckLocale` polices two namespaces: the mod-prefix namespace, and the names your plan declared. For BBB those barely overlap, because every name carries the historical `bbb-` prefix and none carries `better-belt-balancer-`. The missing direction still works (a legacy setting with no `[mod-setting-name]` entry is reported under the name it actually carries) and so does the value direction (a stale `[string-mod-setting]` key under `bbb-recipe-cost` is reported), but the name and description **orphan** scan sees nothing at all: no entry in the file carries the mod prefix, so a leftover from a setting you renamed during the migration goes unreported, and that is exactly the mistake a migration makes.

Given the hand-rolled list, the checker knows every setting name the mod has and the orphan scan becomes complete: an entry matching no declared, legacy or hand-rolled name is reported whatever it is called. `bbb-multi-edge-parts` is recognised rather than flagged, and a stale `bbb-renamed-away` is caught. The list suppresses orphans without creating obligations, so naming the runtime-global setting there does not make this library start demanding locale entries for it.

`bbb-multi-edge-parts` is also written by the mod itself at runtime. Nothing in this library reads or writes settings at runtime, so that behaviour is untouched by the migration either way.

The two startup dropdowns are what `IngredientsBy` and `CostBy` are for. `bbb-recipe-cost` selects the ingredients of the balancer recipes, which is an ingredient plan per value. `bbb-tech-cost` selects which vanilla logistics technology the balancer research is priced from, which is a one-rung ladder per value, and under `CostBy` it also makes that technology the prerequisite, so the research lands where its price says it should.

The customizer arrived in the mod's third round on this library, and its four settings are where the ordering rule bites. `bbb-recipe-cost` gained a text setting beside it, its own option list untouched; `bbb-tech-cost` gained a pack text, a count and a seconds setting beside it, the two numbers defaulting to 0 so that an untouched pair leaves the chosen tier deciding. The mod shipped those four as `Legacy` settings under hand-written `bbb-` names and orders (`aa`, `ba`, `bb`, `bc`), because at the time a generated setting's order came only from its declaration index and no ordering of the declarations could put the three research fields under their dropdown. With `OrderAfter` the same layout is reached with generated names:

```go
recipeCost := lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
	[]string{"vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express"}, "a")
lib.OrderAfter("a")
recipeIngredients := lib.IngredientsSetting("recipe-ingredients", vanillaIngredients) // aab
techCost := lib.LegacyDropdownSettingNeedingLocale("bbb-tech-cost", "logistics",
	[]string{"logistics", "logistics-2", "logistics-3"}, "b")
lib.OrderAfter("b")
techPacks := lib.PacksSetting("tech-packs", []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}}) // bad
techCount := lib.IntSetting("tech-count", 0, fkrecipes.Between(0, 1000000))                                   // bae
techSeconds := lib.IntSetting("tech-seconds", 0, fkrecipes.Between(0, 3600))                                  // baf
```

```rust
let recipe_cost = lib.legacy_dropdown_setting_needing_locale("bbb-recipe-cost", "vanilla",
    &["vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express"], "a");
lib.order_after("a");
let recipe_ingredients = lib.ingredients_setting("recipe-ingredients", vanilla_ingredients); // aab
let tech_cost = lib.legacy_dropdown_setting_needing_locale("bbb-tech-cost", "logistics",
    &["logistics", "logistics-2", "logistics-3"], "b");
lib.order_after("b");
let tech_packs = lib.packs_setting("tech-packs", vec![Pack::new("automation-science-pack", 1)]); // bad
let tech_count = lib.int_setting("tech-count", 0, NumericSpec::between(0.0, 1000000.0));         // bae
let tech_seconds = lib.int_setting("tech-seconds", 0, NumericSpec::between(0.0, 3600.0));       // baf
```

The screen then reads `a`, `aab`, `b`, `bad`, `bae`, `baf`: the recipe dropdown, its text, the research dropdown, its three fields. The names are `better-belt-balancer-recipe-ingredients` and so on, prefixed from the packaged mod name, and the locale entries follow those names. The `Legacy` text and numeric constructors remain the way to keep both a name and an order a mod already ships; for a setting no player has stored yet, the generated name with `OrderAfter` costs nothing to rename later. One rule reaches a mixed plan whether or not it calls `OrderAfter`: a generated order equal to a legacy one is refused by name, so a mod whose legacy orders happen to be two letters (`aa` beside a first declaration, or `ba` beside a twenty-seventh) meets that refusal on updating and gives one side an order of its own.

## Migrating incrementally

Nothing forces an all-at-once move. A plan may mix legacy and generated settings, and a mod may keep declaring some settings by hand. The order that has caused the least churn is:

1. Declare the existing settings with the `Legacy` setting constructors, keeping every name, default, allowed-value list and order exactly as shipped. Check the settings prototypes hash the way they did before.
2. Move the prototypes across with `LegacyItem`, `LegacyRecipe` and `LegacyTechnology`, keeping every name and every field. `Order`, `PlaceResult` and `Extra` are how the fields this library has no slot of its own for still reach the prototype. Check the data dump hash the way you checked the settings one: for a faithful migration it should not move either.
3. Use `IngredientsBy` and `CostBy` where a setting was driving a hand-rolled branch. This is the step that changes something, so it is the step whose hash is expected to move.
4. Run `CheckLocaleWith` from your own test suite against the `.cfg` you already ship, passing the settings you still declare by hand. It should be clean, because the names have not moved, and anything it does report is a leftover the migration created. `CheckLocaleAdvisories` is a separate call and is deliberately not part of that report: it returns notes about the game's own `technology-name` keys a cost dropdown composes, which you must NOT define, so failing a test on them would fail it on something there is nothing to do about. Log those, do not assert on them. `LocaleEntries` reads the same file for assertions of your own, such as the entity name entry this library knows nothing about.
5. Add new settings and prototypes with the generated constructors. Those get the prefix and a derived order, and they cost nothing to rename later because no save has ever seen them. Call `OrderAfter` first when the new settings must sit under a legacy one, because a derived order sorts among legacy orders by the alphabet, not by the declaration.
6. Put a text setting beside a dropdown of presets, as described above, when the mod is ready to let the player write the recipe. Stored preferences survive untouched, because the dropdown keeps its name and its exact option list.

Steps 1 and 2 are meant to be hash-neutral, which is what makes them safe to ship on their own. Steps 3 and 6 are where behaviour changes, and separating them is what lets a bisect say which one did it.

## Changing a setting's type under a name you already ship

A name is what Factorio preserves, and it preserves the name whatever type the declaration gives it. So redeclaring an existing setting as a different type is not a rename and does not reset anybody: the stored value is read back through the new type's rules, and what happens next is silent in every path.

Measured on Factorio 2.0.77, with an `int-setting` declared under a name whose stored value is a double, minimum 0, maximum 600 and a declared default of 100:

| Stored value | What the engine logged | Exit | What the mod read | The file after the load |
|---|---|---|---|---|
| `45.0` | nothing | 0 | `45` | rewritten, now encoded as a signed int |
| `7.5` | nothing | 0 | `7` | rewritten as `7` |
| `-0.5`, below the new minimum | nothing | 0 | `0` | rewritten as `0` |
| `1200.5`, above the new maximum | nothing | 0 | `100`, the declared default | rewritten as `100` |

The rule behind those four rows: the double is truncated toward zero (`-5.5` becomes `-5`, `599.5` becomes `599`), then range checked; in range it is kept and out of range the declared default applies. Nothing is logged in any path and the load always succeeds, so the first successful load rewrites the file and the fractional part is gone for good.

The library cannot warn about this and neither can you. By the time the data stage runs it is handed `7`, with nothing marking it as having been `7.5`, and the engine's own type-mismatch check fires only on a genuine kind mismatch (a string stored under a number), not on a double stored under an int. What an author can do is say so: a whole number survives the change, a fractional one is truncated silently and permanently, and that belongs in the release note for the version that makes the change.

The reverse direction is safe by comparison: an `int-setting` redeclared as a `double-setting` reads every stored value back unchanged.

## Upgrading a mod that has already adopted the library

### Taking v0.1.2

v0.1.2 changes only text, and only text this library composes onto its own settings. No plan surface moves, no prototype this library builds out of your declaration moves, and no behaviour changes: what moves is what a player reads on the Mod Settings screen, which was about 1050 characters per text field and is now about 450 to 650.

- **A text setting's composed block is four or five lines instead of six.** The line saying that a list too long for one line continues on the next is gone from every composition, on a text setting and on an ingredient dropdown alike. What it disclosed is real, that the engine's own break leaves the continuation at the left margin so a player who copies what looks like a whole line loses the end of the list, and it is now documented for you, in [Give a dropdown option a short name](usage.md#a-few-things-worth-knowing-about-the-settings-screen), rather than said on every field of every consumer.
- **The ladder line moved to the field that renders the lists it is about.** It is composed onto a text setting only where you declared NO dropdown beside it, whose own default line is then the list that applies. With a dropdown beside it, the dropdown's own description already carries the same disclosure over the presets a player is choosing between, and the text field no longer repeats it. A cost dropdown still carries none, so a science-pack setting bound beside a `CostBy` tier is the one shape where the ladder reaches no settings-screen field at all; what a copied cost's packs do on a mod set that lacks them is disclosed on the technology's own tooltip, where it happens.
- **The format, switch and fallback sentences are shorter.** They read `Internal names, as on the default line, up to 2000 characters.`, `Leave this as default and the option chosen above decides; anything else applies instead.` (or `...and this mod's own list applies; anything else applies instead.` with no dropdown beside it) and `Text this mod cannot use is set aside as though it said default; the reason is in the log or the load error.` The claims are the ones they always made, the fallback line's two places included.
- **An ingredient preset's second line opens with `to type:` rather than `type:`.** The line is an instruction and `type:` alone read as the noun. A gate of yours that matches that prefix moves.
- **A research count or seconds setting beside a research dropdown leads with the sentinel.** It reads `Leave this at 0 and the option chosen above supplies the number; otherwise a whole number up to 100000.` rather than opening with the range, because 0 there is how the field says "not customised" and not the bottom of a range anyone would pick. With no dropdown beside it the sentence is unchanged: `A whole number from 1 to 600.` This one line is LONGER than what it replaces, which is the one place this round spends rather than saves.
- **Every settings hash you keep moves, on every engine you keep a row for.** A hash over `data.raw` does not, and this round is a good test of that: if one moves, something other than a composed description moved with it.

Nothing in your locale file changes, no entry becomes required or stops being required, and `CheckLocale`'s findings are the ones they were.

### Taking v0.1.1 for Factorio 2.1

v0.1.1 changes two things the library hands the engine, both because Factorio 2.1 moved under it and both measured on 2.1.17 (build 87315), and one thing you may have declared:

- **A recipe's crafting category goes out as `categories`, a list, rather than as `category`.** On 2.1 a recipe carrying `category` stops the load outright with ``Error while loading recipe prototype "<name>" (recipe): In RecipePrototype, `category` and `additional_categories` got merged into `categories` table. Please use that instead.`` The spelling is chosen at emit time from the running engine's own version, so on 2.0 nothing moves. Your `RecipeSpec.Category` field is unchanged and so is the `Op` stream a host test of yours asserts on: the plan still says `category`, and only what reaches the engine differs. What does move is a test that hashes or transcribes a real 2.1 dump.
- **A science pack is recognised by the engine's own current rule.** On 2.0 that is the prototype type `tool`; on 2.1 base and its bundled expansions declare no `tool` prototype at all and base's packs are items whose subgroup is `science-pack`. Under v0.1.0 on a 2.1 engine every pack answered no, so every research this library priced came out with an empty unit, one `fkrecipes: ERROR: ` line each and a free-research note in its tooltip. A suite that pinned those lines on 2.1 goes green-to-red the right way round: the lines should now be absent and the packs present.
- **`Extra` may no longer carry `categories` on a recipe.** The library owns both spellings of the field now, so `the recipe <name> sets categories through Extra, which this library emits` is a new plan-time refusal. On 2.0 that key meant nothing to the engine, so a plan that carried it was legal and inert; it is refused on both engines rather than refused on one and silently overwritten on the other. Take it out; `RecipeSpec.Category` is the surface.

**One thing to put in your own `changelog.txt`.** A player who ran an earlier release on a 2.1 engine has technologies your mod priced that completed for free, and some of them are researched in live saves. v0.1.1 prices future research correctly and does not and cannot take back a completed one. That is a balance change your players saw and a second one they are about to see, and the changelog is one of the three places they look.

Your `World` and your fixtures are untouched. `ToolExists` (`tool_exists`) keeps its name and its signature; the name is historical now, and what it answers is "is this a science pack the running engine's research units accept". A fixture that answers it out of a list of names keeps working exactly as it did.

Your mod's own `info.json` is a separate matter and the library has no say in it: `fklua mod --factorio-version` has to name the series the player runs, because a 2.1 engine refuses a mod declaring `2.0` at game start with `Incompatible Factorio version (current: 2.1, required: 2.0)`, before a line of it runs.

One thing v0.1.1 does not do: an item whose subgroup is `science-pack` is not the same question the 2.1 engine actually asks. Measured, the engine refuses a research unit naming anything no lab's `inputs` lists, with `Technology <name>: there is no lab that will accept all of the science packs this technology requires.` That is not a question the data stage can answer, because on a stock 2.1 game the expansion's own packs and its second lab do not exist yet when the data stage runs. The subgroup is what the data stage can see, and it selects base's own packs exactly.

### What moved in the customizer round

What compiles is not what passes, and with that version, what compiles is not everything either.

**Six declarations no longer compile**, and the break is intended: the state "the player's own list is in force" moved out of a dropdown value and into the text setting itself, so the arm that used to carry it is gone. In Go: `IngredientChoices.Custom`, `IngredientChoices.CustomValue`, `CostChoices.Custom`, `CostChoices.CustomValue` and `CustomCost.Position` are removed, and `CustomCost.Seconds` takes an `IntSettingRef` where it took a `DoubleSettingRef`. In Rust the same six, under `custom`, `custom_value`, `position` and `seconds`; the three struct literals fail with `E0560: struct has no field named 'custom'`. [Adding a customizer to a dropdown you already ship](#adding-a-customizer-to-a-dropdown-you-already-ship) has the shape that replaces them, and the dropdown keeps its exact option list through the change.

**One locale entry becomes an orphan, and the prose around it goes quietly false.** With no `custom` value in the dropdown, `CheckLocaleWith` no longer requires `string-mod-setting.<dropdown>-custom`, and an entry you already ship for it is reported as matching no dropdown value this plan declares. Delete the line rather than debugging the report. Then read the rest of your `.cfg` with the same question, because nothing in any gate can: every `[mod-setting-name]` and `[mod-setting-description]` entry that names the withdrawn option is now instructing the player to pick a value the dropdown does not offer, and it is a string the checker never reads. A dropdown description offering the option, a text or number field described as applying "while the setting above is set to Custom", and a field NAMED for the option are the three shapes to look for. Each says the same thing after the change: the text field applies whenever it does not say `default`, and the dropdown supplies the rest.

Past those, the declaration surface is stable across a library upgrade, so the rest of your declarations compile unchanged and a green build tells you nothing about what moved; your own test suite is where an upgrade lands, because a suite pins the messages the library writes and the prototypes it emits rather than the API it offers. A suite written against an older version can go red in a number, in a whole assertion, and in the name of the test itself.

Seventeen behaviours are the ones a suite is most likely to have pinned. Each is stated as the failure it produces, so a red test can be matched against it:

- **A stored text the library cannot use falls back instead of refusing the load.** The whole typed list is set aside, the field decides as though it had been left alone (the preset the dropdown beside it is currently on where you declared one, your declared list where you did not), and one `fkrecipes: ERROR: ` line names the setting and the reason, so a test named for a refusal is asserting the opposite of what happens. [Ingredients the player writes](usage.md#ingredients-the-player-writes) has the rule and the three things a mod set can still stop the load over.
- **A ladder that lands twice on one name merges rather than emitting a list the engine refuses.** The amounts are added into the first occurrence, which keeps its place, and a line records it: an item line names both amounts, a fluid line names the ladder that landed on the name. A test expecting a duplicate entry, a dropped second landing or a failed load therefore sees a shorter list and one more log line. [Ingredients, and the resolve-or-drop contract](usage.md#ingredients-and-the-resolve-or-drop-contract) has both lines.
- **Nothing is deleted from a text.** Spaces, tabs, line breaks and the four spaces a word processor produces separate words; every other character with no visible shape of its own is refused by its code point wherever it sits, and every message that quotes a piece of a player's text writes each invisible character in it as `U+XXXX`, so a test asserting that a text was silently stripped, or expecting a quoted blank, moves. [The ingredient list](ingredient-list.md) has the whole set and the message shape.
- **The reserved words `default` and `none` are matched without regard to ASCII case.** `Default` and `DEFAULT` are the word rather than an item name, so a test that read a capitalised spelling as a name moves. [The ingredient list](ingredient-list.md) also says how to name an item that really is called one of them.
- **A recipe or technology whose stored setting value could not be used carries a `localised_description` it did not carry before.** One trailing line names the setting and points at the log, joined onto your own description, so the player hovering the recipe or the technology in the game reads why it is not what they typed. Where you declared `Description` in the plan, the line is joined onto that literal. Where you did not, the composition opens with a reference to the prototype's own `[recipe-description]` or `[technology-description]` locale entry followed by a newline, behind an empty alternative: your locale entry is rendered above the line where you wrote one, and the line stands alone with no blank line in front of it where you did not. So a description declared in a locale file survives a fallback, and you owe nothing for that: the entry stays optional and `CheckLocale` never asks for it. A test that transcribes such a prototype moves, and so does a hash over `data.raw` taken in a state where something falls back; a prototype nothing fell back on is byte for byte what it was. A fallback on a recipe's ingredient text carries one sentence more, because changing a recipe empties an assembling machine's input slots of anything the new list does not use and that is worth saying before a player acts on it; a crafting time, a pack text and a research number carry the first sentence alone, because none of them changes what the recipe is made of. A prototype's whole `localised_name` or `localised_description` is also SEVERAL ELEMENTS now where a composed line is long: the engine allows 200 bytes per string element and refuses the load over it, so every literal this library writes into either field is filled to at most 180 bytes and broken at a space, and the engine concatenates the pieces back into one sentence for the player. A test comparing a whole `localised_description` value sees the extra elements; a test comparing the rendered sentence does not.
- **A merged amount above an amount ceiling is capped rather than refused.** Two ladder rungs collapsing onto one name above 65535 items, or above 1e301 of a fluid, now emit that ceiling with a line ending `so it is capped there`, and the recipe or technology carries a trailing description line saying so. A test that expects a refused plan there sees an accepted one with two more log lines and a moved prototype. An amount YOU declared above a ceiling is still refused, because that check reads only your declaration.
- **A copied research cost drops a science pack the game does not have.** `CostOf`, and a `CostBy` tier's chosen source, filter the unit they copy through the same probe a declared pack ladder uses, so a mod set that removed a pack or demoted it out of whatever the running engine treats as a science pack gets a shorter unit and a line per dropped pack instead of a load that stops with `Invalid research unit (...)`. A source that carries a unit no longer settles the pack question on its own, which is what can put a tier's copied packs and your `Fallback` on one path; the paragraph below this list says which fallback is asked about and when. A test that transcribes such a technology moves, and so does a hash over `data.raw` taken against a mod set that changed a pack.
- **A copied cost that keeps no science pack falls back to the one you declared.** A `CostBy` tier whose chosen source has no usable pack left is priced by your `Fallback` unit, with one `fkrecipes: ERROR: ` line and a trailing description line; the prerequisite and the level cap stay where the tier put them. So a fixture whose science packs are all absent reaches your `Fallback`'s science packs rather than leaving them unasked. `CostOf` has no declared cost behind it, so that one is emitted with an empty unit instead, as the next bullet describes.
- **A research left with no usable science pack anywhere is emitted with none, and the refusal that used to stop the load is gone.** The sentence `fkrecipes: the technology X has no science pack the game has; research takes at least one, and none of <names> is a science pack here` no longer exists, and a test asserting it, or asserting a refused plan there, sees an accepted plan instead: the technology comes out with an empty `unit.ingredients`, one `fkrecipes: ERROR: ` line per technology ending `so the research is emitted with no science pack and completes for free`, and a trailing description line saying the same thing in the tooltip. The line and the note are PER TECHNOLOGY, where the old refusal named only the first in declaration order, so a plan where two technologies lose their packs now produces two of each. The names are still every rung the walk asked about, each once and in the order they were asked: where a tier's copied unit lost every pack and your `Fallback` then lost every pack too, the list holds both sets, the copied unit's first. The word `free` is literal and measured: an empty research unit loads, `add_research` accepts it, and it completes after `count * time` ticks in a lab holding nothing (measured in play on Factorio 2.0.77, build 84539). That is a balance change nobody chose, which is why it is disclosed in the tooltip, and it is still better than the load stopping: the engine's error dialog cannot reach the Mod Settings screen, so the refusal was a lock-out. A unit you DECLARE with no pack at all is still refused, because that check reads only your declaration.
- **A copied pack list in a form this library cannot read degrades instead of refusing.** `fkrecipes: <tech>: the unit of <source> holds a table this library cannot copy faithfully` is no longer a refusal. Behind a `CostBy` tier, where you declared a `Fallback`, the line ends `, so this mod's own declared cost applies instead` and the prerequisite and the level cap stay where the tier put them. Behind a bare `CostOf`, where there is nothing declared to fall back to, the line ends `, so the research is emitted with no science pack and completes for free` and the unit is kept with its `ingredients` replaced by an empty list, so the count, the time, a `count_formula` and every field this library has never heard of survive untouched. Each has its own trailing description line, and the two notes differ deliberately: on this path the list was never decoded, so the note says it could not be read rather than that the game has none of those packs. A test asserting a refused plan on either shape sees an accepted one.
- **A prerequisite cycle your plan closed drops an edge instead of stopping the load.** The walk now resolves: it finds a ring, drops the first edge in the ring's own order that your plan made, and walks again. An edge of yours is one of two things and each has its own `fkrecipes: ERROR: ` line and its own tooltip sentence: a prerequisite of one of your own technologies (`requiring <name> would loop this game's technology tree (<ring>), so the prerequisite is dropped`) and a splice your plan inserted into another technology's list (`making it a prerequisite of <before> would loop this game's technology tree (<ring>), so the splice is dropped`). A dropped splice gives back the prerequisite it replaced, to every later splice built on top of it, so undoing yours never deletes another mod's own edge. `fkrecipes: a prerequisite cycle: <ring>` survives for exactly one case, a ring holding no edge of yours, and a test that built a cycle through its own plan to pin that sentence now sees an accepted plan, two more log lines and a moved prototype.
- **A refusal reached after a stored value fell back states the fact and points nowhere.** It still carries one more sentence, and that sentence is now `. The stored value of <setting> could not be used and was set aside, so what applied is what that field gives when it is left alone.` and stops there. What it used to add was a route, saying that correcting the setting under Settings then Mod settings then Startup was what a player could change; the `Error loading mods` dialog cannot reach that screen (measured on Factorio 2.0.77), so the advice was unusable while the fact behind it was true. A test comparing a whole refusal message in that state moves.
- **A composed description carries more than the entry you wrote.** A text setting's description is a default line holding your declared list, and four or five library sentences under it. The ladder sentence, which a field with no dropdown beside it carries, says that where a list the mod chose names something the player's mods do not have, the next name that entry offers is used instead, that an entry it offers nothing for is left out, and that two entries landing on one name have their amounts added, so what they craft can be a shorter list than the one shown; on a science-pack setting it is the same rule in the packs vocabulary. The format sentence says the field takes internal names, as on the default line, up to 2000 characters. The switch sentence says which field decides while this one holds the word `default`. The fallback sentence says what text the library cannot use costs. On an INGREDIENT setting the format sentence also names the word `none`, which empties the list and makes the recipe free to craft; on a science-pack setting it does not, because a pack list turns that word down. An ingredient dropdown's preset is two lines, your localised label and then an indented line opening with `to type:`, and the ladder sentence sits after the last preset and before the line about the text setting beside it; an ingredient dropdown with NO text setting beside it carries the ladder sentence alone, under your own entry, and nothing else; a cost dropdown carries no ladder sentence, because its presets render no list to shorten. A test that transcribes a setting prototype moves with all of that, and so does any hash you keep over the engine's settings dump; a hash over `data.raw` does not move, because no preset's ingredient plan changed.
- **An ingredient dropdown with no text setting beside it now needs a `[mod-setting-description]` entry.** The library composes the ladder sentence onto that dropdown, so the entry stopped being optional there and `CheckLocale` reports `the dropdown setting <name> has no [mod-setting-description] entry, and the library composes onto that entry the line saying what a name this game does not have costs the list`. The remedy is one line: add a `[mod-setting-description]` entry for that setting saying what the dropdown chooses between. A suite that asserts the report is empty goes red until you do.
- **A recipe whose ladders all ran out carries a description line and an `ERROR:` line of its own.** Where every entry a recipe declared was put to the game and every ladder missed, the recipe is emitted with no ingredients at all, `fkrecipes: ERROR: <recipe>: this game has none of the ingredients this recipe names, so it is emitted with no ingredients and costs nothing to craft` joins the log, and the recipe's own tooltip carries `This game has none of the ingredients this recipe names, so it costs nothing to craft. The reason is in the log.` followed by the sentence about emptied input slots. The emitted prototype is unchanged, so a test comparing `ingredients` alone stays green; a test comparing a transcript line by line gains a line, and one comparing a whole prototype sees a `localised_description` where there was none. A recipe somebody emptied on purpose, by declaring no ingredients or by typing `none`, gets neither: those are choices and are disclosed where the choice was made.
- **Every locale key inside a composed SETTING description is a three-element table now, not a one-element one.** Where the library used to write `{"mod-setting-description.<setting>"}` it writes `{"?", {"mod-setting-description.<setting>"}, "<setting>"}`, and the same for a dropdown value's `[string-mod-setting]` key and for the `technology-name` key a cost preset names; the raw fallback is last, and the engine renders it when it has no entry for the key. Three things move with that. A test that counts the parameters of a composed description or of one of its preset lines still sees the same count, because a wrapper occupies the one slot the bare key table occupied. A test that pins the table SHAPE, comparing a whole `localised_description` value or walking into a key table by index, sees a table one level deeper and a `"?"` where the key used to be: [BetterBeltBalancer](https://github.com/Techrocket9/BetterBeltBalancer) pins one such shape in its tuning suite. And any hash you keep over the engine's settings dump moves again, on every engine you keep a row for, while a hash over `data.raw` does not. A PROTOTYPE description carries a key in exactly one shape, and it is a different one: a recipe or a technology that carries a trailing line and declares no `Description` opens with `{"?", {"", {"<kind>-description.<emitted name>"}, "\n"}, ""}`, which is the bullet above. A hash over `data.raw` taken in a state where something falls back moves with it.

One more line joins the log stream and belongs on the same list: a recipe whose resolved ingredient list names its own product is emitted as it resolved and logs one line saying that nothing can craft the first one, so a suite asserting a transcript line by line gains a line. The wording and the three shapes that produce it are under [Ingredients, and the resolve-or-drop contract](usage.md#ingredients-and-the-resolve-or-drop-contract).

**Which fallback is asked about, and when.** Every ladder in this library stops at its first answer and nothing past that answer is put to the game: an ingredient ladder asks about its second rung only when the first is absent, a declared pack ladder does the same, and a `CostBy` tier's source ladder asks about a second source only when no earlier one carries a unit it can copy. `CostChoices.Fallback` is read in two steps that follow different rules. The numbers it is VALIDATED against come from your declaration alone, with no question put to the game, so a count below 1, a time at or below zero, a duplicate pack, an empty rung name and a declared amount above a ceiling are refused whether or not any ladder ever lands on it. Its science packs are put to the game in exactly two places: when no source in the chosen ladder carries a unit, and when a source did carry one and the science-pack probe then left it with no science pack at all. The second place is the one to plan for, because a source carrying a unit does not end the pack question: the copied unit's own packs go through the same probe a declared ladder uses, so a mod set that removed a pack, or demoted it to a plain item, can empty the tier's cost and reach your `Fallback` after all, with the tier still supplying the prerequisite and the level cap. If your `Fallback`'s own ladders then resolve to nothing either, the technology is emitted with an empty `unit.ingredients` and an `fkrecipes: ERROR: ` line naming every rung both sets tried, unless the technology also declares `CostFrom` and the player has typed a pack list into it, in which case the cost that applies is theirs and the line and the note are taken back with it. That outcome is a research that completes for free, which is why it is disclosed in the technology's own tooltip, and it is the case the remedy below exists to prevent rather than merely to diagnose. The remedy is the one a declared ladder already has, and it is worth more since the refusal became a degradation: give the `Fallback`'s packs rungs of their own, so a modpack that renames or removes one leaves something behind it. Without a rung the research does not stop the load any more, it goes free, and a free research is a balance change your players did not choose and will not report as a bug. And a fixture meaning to prove that a `Fallback` pack is never asked about needs a source that carries a copyable unit AND keeps at least one pack, which a game with every science pack taken out of it does not have.

Three things follow for the upgrade itself. Treat the suite rather than the compiler as the gate, and run it before concluding that an upgrade was free. Read the log stream a plan produces and not only the prototypes it emits: the fallback and the self-product line are visible only as a line, because what each of them emits is what your declaration already produced, so a test that inspects prototypes alone stays green while what a player reads has moved. And re-take any hash you keep over a settings dump, on every engine you keep a row for; a hash over `data.raw` should not move for an upgrade that changes no plan of yours, and one that moves anyway is an ingredient list resolving differently, which is worth finding before it ships.
