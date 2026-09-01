# Migrating a mod that already ships settings

FkRecipes generates setting names from the mod it is packaged as, and it generates `order` strings from declaration order. That is right for a new mod and wrong for one that already has players: Factorio persists startup values in `mod-settings.dat` keyed by name, with no rename mechanism, so a setting that changes name is a setting that silently reverts to its default in every existing save and server config.

This document is about the surfaces that let an existing mod adopt the library without changing anything a player can observe: setting names it keeps verbatim, ingredients a dropdown chooses, and a research cost a dropdown chooses. It assumes you have read [Using FkRecipes](usage.md).

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
                packs: vec![Pack { name: "automation-science-pack".into(), amount: 1 }],
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

## Migrating incrementally

Nothing forces an all-at-once move. A plan may mix legacy and generated settings, and a mod may keep declaring some settings by hand. The order that has caused the least churn is:

1. Declare the existing settings with the legacy constructors, keeping every name, default, allowed-value list and order exactly as shipped. Check the settings prototypes hash the way they did before.
2. Move the recipes and technologies across, using `IngredientsBy` and `CostBy` where a setting was driving a hand-rolled branch.
3. Run `CheckLocaleWith` from your own test suite against the `.cfg` you already ship, passing the settings you still declare by hand. It should be clean, because the names have not moved, and anything it does report is a leftover the migration created.
4. Add new settings with the generated constructors. Those get the prefix and a derived order, and they cost nothing to rename later because no save has ever seen them.
