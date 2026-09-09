# Migrating a mod that already ships settings

FkRecipes generates setting names from the mod it is packaged as, and it generates `order` strings from declaration order. That is right for a new mod and wrong for one that already has players: Factorio persists startup values in `mod-settings.dat` keyed by name, with no rename mechanism, so a setting that changes name is a setting that silently reverts to its default in every existing save and server config.

The same is true of prototype names. A save references an item by name in every inventory and on every belt, a technology by name in its researched list, a recipe by name in every assembler, and your own hand-rolled entity names its item through `place_result`. The engine's answer to a name that moved is not a warning:

```
Error in assignID: item with name 'bbb-balancer-part' does not exist.
```

This document is about the surfaces that let an existing mod adopt the library while keeping every name a player or a save can observe: settings and prototypes it keeps verbatim, ingredients a dropdown chooses, and a research cost a dropdown chooses. Names are what is preserved. Adoption is not free in other respects, and the [README](../README.md) carries the measured download and parse cost. It assumes you have read [Using FkRecipes](usage.md).

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

A mod that ships a dropdown of preset recipes can let the player write their own without losing anybody's stored choice. The dropdown stays, under its legacy name, and gains one value; a text setting arrives beside it; the text applies only while the dropdown says `custom`. See [The ingredient list](ingredient-list.md) for what the player types.

```go
cost := lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
	[]string{"vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express", "custom"}, "a")
custom := lib.IngredientsSetting("balancer-part-ingredients", vanillaIngredients)
lib.LegacyRecipe(part, "bbb-balancer-part", fkrecipes.RecipeSpec{
	IngredientsBy: &fkrecipes.IngredientChoices{Setting: cost, Choices: presets, Custom: custom},
	CraftTime:     1,
})
```

```rust
let cost = lib.legacy_dropdown_setting_needing_locale("bbb-recipe-cost", "vanilla",
    &["vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express", "custom"], "a");
let custom = lib.ingredients_setting("balancer-part-ingredients", vanilla_ingredients);
lib.legacy_recipe(part, "bbb-balancer-part", RecipeSpec {
    ingredients_by: Some(IngredientChoices { setting: cost, choices: presets, custom: Some(custom), ..Default::default() }),
    craft_time: 1.0,
    ..Default::default()
});
```

Every value a player has stored is still one of the dropdown's values, so every preference survives the update untouched; the only players who see anything new are the ones who open the settings screen. The text setting is a new name, so it carries the generated prefix and the word `default` as its text. Where it lands in the settings screen is a separate decision: a generated setting's order string comes from its declaration index, and next to legacy orders such as `a` and `b` that puts it between the two whatever you meant. `OrderAfter("a")` (`order_after("a")`) before the declaration places it behind the dropdown it belongs to, under the order `aab`; see [Using FkRecipes](usage.md) for the rule.

What the engine does not allow is filling that text from the player's old choice. The settings stage, where defaults are declared, cannot see any stored value (measured on Factorio 2.0.77: `data.raw` is empty there), and nothing at the data stage or at runtime can write a startup setting. A default is therefore one text for every player. What the library does instead is compose the dropdown's description: your own `[mod-setting-description]` entry, then one line per preset rendered as an ingredient list, so a player on `cheap` who picks `custom` can see what `cheap` was and start from it.

Two locale entries are new: `[string-mod-setting]` for the `custom` value under the dropdown's name, and `[mod-setting-name]` and `[mod-setting-description]` for the text setting, where the description is the place to tell the player the format. `CheckLocaleWith` reports all three when they are missing. If a preset of yours is already named `custom`, give the arm another value with `CustomValue` rather than renaming the preset.

One thing to check in the labels you already ship: a dropdown label such as `Default: 4 iron plates, 2 gears, 2 transport belts` speaks display names, and a player who copies it into the text field gets your own list back with an error line in the log, because the language takes internal names. The composed description under the dropdown shows each preset in the language, so the two vocabularies sit side by side in one setting; either rewrite the labels in internal names, or keep them as they are and rely on the description, but decide it rather than discover it from a player.

```
[string-mod-setting]
bbb-recipe-cost-custom=Custom (edit the ingredients below)

[mod-setting-name]
better-belt-balancer-balancer-part-ingredients=Balancer part ingredients
[mod-setting-description]
better-belt-balancer-balancer-part-ingredients=Used when the recipe above is set to Custom. Write the amount, then the item name, and separate ingredients with commas: 2 iron-plate, 3 copper-cable
```

This is the step whose settings hash is expected to move, because the dropdown's `allowed_values` grows and its description becomes a composed value, while the data hash stays where it was for every preset: the presets are the same plans as before, and the custom path is only taken by a player who chose it.

One rule that reaches a migrated `CostBy` whether or not it takes a `Custom` arm: a `Fallback` unit must name at least one science pack, because a unit declared with none is refused at plan time even when no ladder ever reaches it. The pack is a ladder, so a fallback that names `automation-science-pack` with a rung behind it survives a modpack that renames the pack. A `CostBy` dropdown takes a `Custom` arm the same way, with a `PacksSetting`, an int setting for the count, a double setting for the seconds and a `Position` ladder for the prerequisite. Those three settings want to sit under their dropdown, and with a legacy order `b` on the dropdown they would not: their generated orders would sort between `a` and `b`, above it. `OrderAfter("b")` before the three declarations gives them `bad`, `bae` and `baf`, under `b` and before `c`.

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

The customizer arrived in the mod's third round on this library, and its four settings are where the ordering rule bites. `bbb-recipe-cost` gained a seventh value, `custom`, and a text setting beside it; `bbb-tech-cost` gained a fourth, with a pack text, a count and a seconds setting beside it, a `Position` ladder of `logistics-3`, `logistics-2`, `logistics`, and the three fields defaulting to the fallback unit so an untouched `custom` is the base game's Logistics cost. The mod shipped those four as `Legacy` settings under hand-written `bbb-` names and orders (`aa`, `ba`, `bb`, `bc`), because at the time a generated setting's order came only from its declaration index and no ordering of the declarations could put the three research fields under their dropdown. With `OrderAfter` the same layout is reached with generated names:

```go
recipeCost := lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
	[]string{"vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express", "custom"}, "a")
lib.OrderAfter("a")
recipeIngredients := lib.IngredientsSetting("recipe-ingredients", vanillaIngredients) // aab
techCost := lib.LegacyDropdownSettingNeedingLocale("bbb-tech-cost", "logistics",
	[]string{"logistics", "logistics-2", "logistics-3", "custom"}, "b")
lib.OrderAfter("b")
techPacks := lib.PacksSetting("tech-packs", []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}}) // bad
techCount := lib.IntSetting("tech-count", 20, fkrecipes.Between(1, 1000000))                                  // bae
techSeconds := lib.DoubleSetting("tech-seconds", 15, fkrecipes.Between(1, 3600))                              // baf
```

```rust
let recipe_cost = lib.legacy_dropdown_setting_needing_locale("bbb-recipe-cost", "vanilla",
    &["vanilla", "cheap", "belt-fast", "belt-express", "splitter", "splitter-express", "custom"], "a");
lib.order_after("a");
let recipe_ingredients = lib.ingredients_setting("recipe-ingredients", vanilla_ingredients); // aab
let tech_cost = lib.legacy_dropdown_setting_needing_locale("bbb-tech-cost", "logistics",
    &["logistics", "logistics-2", "logistics-3", "custom"], "b");
lib.order_after("b");
let tech_packs = lib.packs_setting("tech-packs", vec![Pack::new("automation-science-pack", 1)]); // bad
let tech_count = lib.int_setting("tech-count", 20, NumericSpec::between(1.0, 1000000.0));        // bae
let tech_seconds = lib.double_setting("tech-seconds", 15.0, NumericSpec::between(1.0, 3600.0));  // baf
```

The screen then reads `a`, `aab`, `b`, `bad`, `bae`, `baf`: the recipe dropdown, its text, the research dropdown, its three fields. The names are `better-belt-balancer-recipe-ingredients` and so on, prefixed from the packaged mod name, and the locale entries follow those names. The `Legacy` text and numeric constructors remain the way to keep both a name and an order a mod already ships; for a setting no player has stored yet, the generated name with `OrderAfter` costs nothing to rename later. One rule reaches a mixed plan whether or not it calls `OrderAfter`: a generated order equal to a legacy one is refused by name, so a mod whose legacy orders happen to be two letters (`aa` beside a first declaration, or `ba` beside a twenty-seventh) meets that refusal on updating and gives one side an order of its own.

## Migrating incrementally

Nothing forces an all-at-once move. A plan may mix legacy and generated settings, and a mod may keep declaring some settings by hand. The order that has caused the least churn is:

1. Declare the existing settings with the `Legacy` setting constructors, keeping every name, default, allowed-value list and order exactly as shipped. Check the settings prototypes hash the way they did before.
2. Move the prototypes across with `LegacyItem`, `LegacyRecipe` and `LegacyTechnology`, keeping every name and every field. `Order`, `PlaceResult` and `Extra` are how the fields this library has no slot of its own for still reach the prototype. Check the data dump hash the way you checked the settings one: for a faithful migration it should not move either.
3. Use `IngredientsBy` and `CostBy` where a setting was driving a hand-rolled branch. This is the step that changes something, so it is the step whose hash is expected to move.
4. Run `CheckLocaleWith` from your own test suite against the `.cfg` you already ship, passing the settings you still declare by hand. It should be clean, because the names have not moved, and anything it does report is a leftover the migration created. `LocaleEntries` reads the same file for assertions of your own, such as the entity name entry this library knows nothing about.
5. Add new settings and prototypes with the generated constructors. Those get the prefix and a derived order, and they cost nothing to rename later because no save has ever seen them. Call `OrderAfter` first when the new settings must sit under a legacy one, because a derived order sorts among legacy orders by the alphabet, not by the declaration.
6. Give a dropdown of presets a `custom` arm, as described above, when the mod is ready to let the player write the recipe. Stored preferences survive because the dropdown keeps its name and every value it had.

Steps 1 and 2 are meant to be hash-neutral, which is what makes them safe to ship on their own. Steps 3 and 6 are where behaviour changes, and separating them is what lets a bisect say which one did it.
