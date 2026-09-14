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

The ladder is how you write "price this like the tier the player asked for, whichever of those technologies this particular game happens to have". A rung that is absent, that is a `research_trigger` technology, or that carries a unit this library cannot copy faithfully is stepped past rather than refused: the player's install is not something your mod can validate at declaration time.

If no rung in the chosen ladder works out, the `Fallback` unit is emitted and the technology has no prerequisite, with a line saying so:

```
fkrecipes: hardened-tips: no source for the projectile cost carries a unit, so the fallback cost applies and the technology has no prerequisite
```

The fallback is held to exactly the same rules as a hand-rolled `Unit`, because it is the cost that applies when nothing else does: a count below 1, a research time at or below zero, or a science pack that does not exist is refused before anything is emitted.

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

One thing to know about the labels you already ship: a dropdown label such as `Default: 4 iron plates, 2 gears, 2 transport belts` speaks display names, while the text field beside it takes internal names, so a player who copies the label into the field gets your own list back with an error line in the log. The composition keeps the two apart. Each preset is two lines in the tooltip, your label and then an indented line opening with `type:` holding that preset in internal names, and that second line is the only one in the tooltip a player is invited to copy. You can leave your labels in display names.

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
4. Run `CheckLocaleWith` from your own test suite against the `.cfg` you already ship, passing the settings you still declare by hand. It should be clean, because the names have not moved, and anything it does report is a leftover the migration created. `LocaleEntries` reads the same file for assertions of your own, such as the entity name entry this library knows nothing about.
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

What compiles is not what passes, and with this version, what compiles is not everything either.

**Six declarations no longer compile**, and the break is intended: the state "the player's own list is in force" moved out of a dropdown value and into the text setting itself, so the arm that used to carry it is gone. In Go: `IngredientChoices.Custom`, `IngredientChoices.CustomValue`, `CostChoices.Custom`, `CostChoices.CustomValue` and `CustomCost.Position` are removed, and `CustomCost.Seconds` takes an `IntSettingRef` where it took a `DoubleSettingRef`. In Rust the same six, under `custom`, `custom_value`, `position` and `seconds`; the three struct literals fail with `E0560: struct has no field named 'custom'`. [Adding a customizer to a dropdown you already ship](#adding-a-customizer-to-a-dropdown-you-already-ship) has the shape that replaces them, and the dropdown keeps its exact option list through the change.

**One locale entry becomes an orphan.** With no `custom` value in the dropdown, `CheckLocaleWith` no longer requires `string-mod-setting.<dropdown>-custom`, and an entry you already ship for it is reported as matching no dropdown value this plan declares. Delete the line rather than debugging the report.

Past those, the declaration surface is stable across a library upgrade, so the rest of your declarations compile unchanged and a green build tells you nothing about what moved; your own test suite is where an upgrade lands, because a suite pins the messages the library writes and the prototypes it emits rather than the API it offers. A suite written against an older version can go red in a number, in a whole assertion, and in the name of the test itself.

Six behaviours are the ones a suite is most likely to have pinned. Each is stated as the failure it produces, so a red test can be matched against it:

- **A stored text the library cannot use falls back instead of refusing the load.** The whole typed list is set aside, your declared list applies, and one `fkrecipes: ERROR: ` line names the setting and the reason, so a test named for a refusal is asserting the opposite of what happens. [Ingredients the player writes](usage.md#ingredients-the-player-writes) has the rule and the one case that still stops the load.
- **A ladder that lands twice on one name merges rather than emitting a list the engine refuses.** The amounts are added into the first occurrence, which keeps its place, and a line records it: an item line names both amounts, a fluid line names the ladder that landed on the name. A test expecting a duplicate entry, a dropped second landing or a failed load therefore sees a shorter list and one more log line. [Ingredients, and the resolve-or-drop contract](usage.md#ingredients-and-the-resolve-or-drop-contract) has both lines and the added-amount ceilings that do still refuse.
- **Nothing is deleted from a text.** Spaces, tabs, line breaks and the four spaces a word processor produces separate words; every other character with no visible shape of its own is refused by its code point wherever it sits, and every message that quotes a piece of a player's text writes each invisible character in it as `U+XXXX`, so a test asserting that a text was silently stripped, or expecting a quoted blank, moves. [The ingredient list](ingredient-list.md) has the whole set and the message shape.
- **The reserved words `default` and `none` are matched without regard to ASCII case.** `Default` and `DEFAULT` are the word rather than an item name, so a test that read a capitalised spelling as a name moves. [The ingredient list](ingredient-list.md) also says how to name an item that really is called one of them.
- **A recipe or technology whose stored setting value could not be used carries a `localised_description` it did not carry before.** One trailing line names the setting and points at the log, joined onto your own description when you declared one and standing alone when you did not, so the player hovering the recipe or the technology in the game reads why it is not what they typed. A test that transcribes such a prototype moves, and so does a hash over `data.raw` taken in a state where something falls back; a prototype nothing fell back on is byte for byte what it was. A fallback on a recipe's ingredient text carries one sentence more, because changing a recipe empties an assembling machine's input slots of anything the new list does not use and that is worth saying before a player acts on it; a crafting time, a pack text and a research number carry the first sentence alone, because none of them changes what the recipe is made of.
- **A composed description carries more than the entry you wrote.** A text setting's description holds three library sentences under the default line, and an ingredient dropdown's preset is two lines, your localised label and then an indented line opening with `type:`. A test that transcribes a setting prototype moves with them, and so does any hash you keep over the engine's settings dump; a hash over `data.raw` does not move, because no preset's ingredient plan changed.

One more line joins the log stream and belongs on the same list: a recipe whose resolved ingredient list names its own product is emitted as it resolved and logs one line saying that nothing can craft the first one, so a suite asserting a transcript line by line gains a line. The wording and the three shapes that produce it are under [Ingredients, and the resolve-or-drop contract](usage.md#ingredients-and-the-resolve-or-drop-contract).

Three things follow for the upgrade itself. Treat the suite rather than the compiler as the gate, and run it before concluding that an upgrade was free. Read the log stream a plan produces and not only the prototypes it emits: the fallback and the self-product line are visible only as a line, because what each of them emits is what your declaration already produced, so a test that inspects prototypes alone stays green while what a player reads has moved. And re-take any hash you keep over a settings dump, on every engine you keep a row for; a hash over `data.raw` should not move for an upgrade that changes no plan of yours, and one that moves anyway is an ingredient list resolving differently, which is worth finding before it ships.
