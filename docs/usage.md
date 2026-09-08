# Using FkRecipes

A tour of every verb the library offers, with the Go and Rust forms side by side. The two halves have the same surface and produce the same prototypes; where the snippets differ it is because Go takes a spec struct with exported fields and Rust takes a struct literal completed by `..Default::default()`.

Start from the [README](../README.md) for what the library is and how to add it to a mod. This document assumes your guest is already building.

## The shape of a mod

Put your declarations in one function and call it from both stage hooks. The module is instantiated fresh for each stage it hooks, so nothing carries across a stage boundary: the settings stage needs the recipes in order to know which double setting backs a crafting time, and the data stage needs the settings in order to read them back.

```go
func plan() *fkrecipes.Lib {
	lib := fkrecipes.New()
	// declarations
	return lib
}

//go:wasmexport fk_settings
func onSettings() { plan().Emit() }

//go:wasmexport fk_data
func onData() { plan().Emit() }
```

```rust
fn plan() -> Lib {
    let mut lib = Lib::new();
    // declarations
    lib
}

#[no_mangle]
pub extern "C" fn fk_settings() { plan().emit(); }

#[no_mangle]
pub extern "C" fn fk_data() { plan().emit(); }
```

`New` and `Lib::new` are the only way to make a plan. Each one carries an identity, and a handle it hands out is valid only for that plan: passing an item or setting handle from one plan into another is refused by name rather than silently resolving to whatever sits at the same index.

## Settings

Four constructors, one per setting type. Each returns a handle you can bind to something later, and each takes the bare name: the mod prefix is added when the prototype is emitted.

```go
enabled := lib.BoolSetting("hardened-tools", true)
batch := lib.IntSetting("rivet-batch", 4, fkrecipes.Between(1, 20))
forging := lib.DoubleSetting("forging-time", 3, fkrecipes.NumericSpec{HasMax: true, Max: 120})
lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
```

```rust
let enabled = lib.bool_setting("hardened-tools", true);
let batch = lib.int_setting("rivet-batch", 4, NumericSpec::between(1.0, 20.0));
let forging = lib.double_setting("forging-time", 3.0, NumericSpec { min: None, max: Some(120.0) });
lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
```

If your mod already ships settings under names of its own, there are `Legacy` constructors that take a full name and an explicit order and emit both verbatim. See [Migrating a mod that already ships settings](migration.md).

Two more constructors declare a text setting the player edits: an ingredient list, or a list of science packs. The setting's default text is the word `default`, which means the list you give here, and the list is written out in the setting's description in the form documented in [The ingredient list](ingredient-list.md), so the player sees both the format and your list before typing.

```go
rivets := lib.IngredientsSetting("rivet-ingredients", []fkrecipes.Ingredient{
	fkrecipes.IngredientNamed(1, "steel-plate"),
	fkrecipes.IngredientNamed(2, "iron-stick", "iron-plate"),
})
packs := lib.PacksSetting("chain-packs", []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}})
```

```rust
let rivets = lib.ingredients_setting("rivet-ingredients", vec![
    Ingredient::named(1, "steel-plate", &[]),
    Ingredient::named(2, "iron-stick", &["iron-plate"]),
]);
let packs = lib.packs_setting("chain-packs", vec![Pack::new("automation-science-pack", 1)]);
```

A text setting must be bound to exactly one recipe or technology (below); one that is declared and bound nowhere is refused, because a setting the player can edit that nothing reads is a mistake. The two constructors are also the only place the ingredient language is linked from: a plan that never calls them ships no parser, no renderer and no custom-cost resolver, so a mod whose settings are bools, numbers and dropdowns pays nothing for the customizer it does not use (the README carries the measured difference). A default that names an item of this plan renders it under its emitted, prefixed name, and the player can name that item the same way: the resolver counts the plan's own items as present even though they are extended after the text is read. A pack list declared with no pack is refused for the reason a unit without one is: `fkrecipes: the packs setting chain-packs declares no science pack; research takes at least one`. Both have `Legacy` forms, `LegacyIngredientsSetting` and `LegacyPacksSetting`, for a text setting a mod already ships under a name of its own.

`NumericSpec` bounds an int or double setting and both bounds are optional: a zero value in Go, `None` in Rust, means unbounded rather than pinned to zero. `Between` and `NumericSpec::between` are shorthand for the common case of setting both.

Settings are emitted in declaration order and given `order` strings from that order, so the settings screen shows them the way you wrote them.

The dropdown constructor is named the way it is because it is the one shape with a locale obligation the library cannot meet for you. A bool, int or double setting renders from its own name, but a string setting renders each of its **values** from a separate `[string-mod-setting]` entry, keyed `<prefixed-setting>-<value>`, and there is no fallback: a value with no entry shows the player a raw key. Prefer another setting type when the choice fits one, and ship the locale entries when it does not. `CheckLocale` below is how you keep them honest.

## Items

```go
plate := lib.Item("hardened-steel-plate", fkrecipes.ItemSpec{
	Icon:        "__steelworks__/graphics/icons/hardened-steel-plate.png",
	IconSize:    64,
	StackSize:   100,
	Subgroup:    "intermediate-product",
	DisplayName: "Hardened steel plate",
	Description: "Quenched and tempered, for tools that keep an edge.",
})
```

```rust
let plate = lib.item(
    "hardened-steel-plate",
    ItemSpec {
        icon: "__steelworks__/graphics/icons/hardened-steel-plate.png".into(),
        icon_size: 64,
        stack_size: 100,
        subgroup: "intermediate-product".into(),
        display_name: "Hardened steel plate".into(),
        description: "Quenched and tempered, for tools that keep an edge.".into(),
    },
);
```

`Order` is the sort key inside the subgroup; empty omits it. `PlaceResult` names the entity the item builds and is presence probed like every other name this library emits, because the engine's answer to a dangling one is an `assignID` abort. The probe runs inside `Emit`, so a hand-rolled entity is extended before that call, not after it. `Extra` passes raw prototype fields through verbatim, after the ones this library writes.

If your mod already ships this item, use `LegacyItem` (`legacy_item`) and the name is emitted verbatim. `LegacyRecipe` and `LegacyTechnology` do the same for the other two. See [Migrating a mod that already ships settings](migration.md).

A zero `StackSize` means 50. A zero `IconSize` omits the field and lets the engine apply its own default. `DisplayName` and `Description` are emitted as inline localised strings, so an item with both needs no locale entries at all; leaving them empty omits the fields.

## Recipes

A recipe produces an item your plan declares. Its name defaults to that item's name, or you can give it one.

```go
quenching := lib.Recipe(plate, fkrecipes.RecipeSpec{
	Name:          "hardened-steel-plate-quenching",
	CraftTimeFrom: forging,
	Category:      "smelting",
	ResultCount:   1,
	Ingredients: []fkrecipes.Ingredient{
		fkrecipes.IngredientNamed(2, "tungsten-plate", "steel-plate"),
		fkrecipes.IngredientOf(rivet, 4),
	},
	DisplayName: "Hardened steel plate",
})
```

```rust
let quenching = lib.recipe(
    plate,
    RecipeSpec {
        name: "hardened-steel-plate-quenching".into(),
        craft_time_from: forging,
        category: "smelting".into(),
        result_count: 1,
        ingredients: vec![
            Ingredient::named(2, "tungsten-plate", &["steel-plate"]),
            Ingredient::of(rivet, 4),
        ],
        display_name: "Hardened steel plate".into(),
        ..Default::default()
    },
);
```

### A result this plan does not declare

`ResultNamed` produces an item that already exists: give a zero `ItemRef` and the name. The item is presence probed and refused if absent, and the recipe then needs an explicit `Name`, because there is no declared item to take one from. It is how a recipe migrates onto the library before its item does, and how a recipe produces somebody else's item outright.

It is not for an item **this plan** declares. The probe asks the game as it stands before your plan runs, so your own item is not there yet; naming it is refused with a sentence saying to use the handle instead. `Extra` has a similar edge worth knowing: the values you put in it are snapshotted when you declare, so a slice you build up and reuse across declarations will not rewrite one you already made.

### Extra, and what it does not do

`Extra` on `ItemSpec`, `RecipeSpec` and `TechSpec` is a list of raw fields, emitted verbatim after the ones this library writes, in the order you give them.

**The values are yours and are not touched.** Nothing inside an `Extra` value is prefixed, and no name inside one is presence probed: this library cannot know which strings in an arbitrary field are prototype names, so guessing would be worse than the passthrough. If a field holds a name the game may not have, that check is yours.

A key this library emits itself is refused rather than merged, because two writers of one field is a silent last-writer:

```
fkrecipes: the recipe balancer-part sets ingredients through Extra, which this library emits
```

One key is shared on purpose. A recipe some technology of this plan unlocks is emitted `enabled = false` by the library, and `enabled` in `Extra` is refused for it with a sentence naming the technology. A recipe nothing in this plan unlocks may carry `enabled` in `Extra`, and the library then writes nothing for that field itself: the value lands where the library's own `enabled` would have gone, so a mod migrating its recipe before its technology can keep the recipe locked by hand until the technology follows.

### Ingredients, and the resolve-or-drop contract

There are two ways to name an ingredient and neither takes a bare string you have not checked.

`IngredientOf` (`Ingredient::of`) names an item your own plan declares. It always resolves, because the same plan emits the prototype.

`IngredientNamed` (`Ingredient::named`) is a presence ladder over names the game may or may not have. The candidates are tried in order and the first one actually present is used. If none is present the ingredient is **dropped** and a line goes to the log:

```
fkrecipes: hardened-steel-plate-quenching: none of tungsten-carbide, titanium-plate is present, so the ingredient is dropped
```

Dropping rather than guessing is deliberate. An ingredient the game does not have is a hard load failure that names your mod, inside whatever overhaul pack the player has installed, and a substituted guess is a recipe you did not design. Use the ladder for anything optional, and put a staple last when the ingredient is required.

`FluidIngredient` (`Ingredient::fluid`) is the same ladder over fluid names, with an amount that may be fractional: `fkrecipes.FluidIngredient(0.5, "water")`. A fluid is accepted only in a recipe whose category allows one. The default category, `crafting`, is the hand-crafting category and the engine refuses a fluid there (measured on Factorio 2.0.77), so a declared fluid in a recipe with no category or with `crafting` is refused at plan time with a sentence naming the recipe, the fluid and the category. Set `Category` to `crafting-with-fluid`, `chemistry` or whichever category your recipe belongs in.

### Ingredients a dropdown chooses

`IngredientsBy` binds the whole ingredient list to a dropdown setting: one plan per value, resolved by the ordinary ladder rules. It is mutually exclusive with `Ingredients`, and the values it offers must equal the setting's allowed values in the same order. A chosen plan that resolves to nothing falls back to the default option's plan, with a line saying so. See [Migrating a mod that already ships settings](migration.md) for the full shape and the refusals.

### Ingredients the player writes

`IngredientsFrom` binds the ingredient list to a text setting declared with `IngredientsSetting`. The player edits the text in the settings screen, in the form documented in [The ingredient list](ingredient-list.md), and the recipe is made of what they wrote. It is mutually exclusive with `Ingredients` and `IngredientsBy`.

```go
lib.Recipe(rivet, fkrecipes.RecipeSpec{IngredientsFrom: rivets, CraftTime: 1})
```

```rust
lib.recipe(rivet, RecipeSpec { ingredients_from: Some(rivets), craft_time: 1.0, ..Default::default() });
```

The text starts out as the word `default`, which means the list you declared with its ladders, exactly as if you had written `Ingredients`, and it keeps meaning that when you change the list in a later release. An edited text is taken as written: every name must exist in the game as loaded, nothing is substituted, and a mistake refuses the load with a sentence naming the setting, the entry and the problem, from the table on that page. One text setting serves one recipe; a mod with several customizable recipes declares one setting per recipe. When the text applies, one line records what was read:

```
fkrecipes: steelworks-steel-rivet takes its ingredients from steelworks-rivet-ingredients: 3 steel-plate, 2 iron-stick
```

A dropdown of presets can offer the same thing as one more value. Give the dropdown a value named `custom`, leave it out of `Choices`, and put the text setting in `Custom`:

```go
IngredientsBy: &fkrecipes.IngredientChoices{
	Setting: medium, // its values are water, oil, custom
	Choices: []fkrecipes.IngredientChoice{
		{Value: "water", Ingredients: []fkrecipes.Ingredient{fkrecipes.IngredientNamed(2, "steel-plate"), fkrecipes.FluidIngredient(10, "water")}},
		{Value: "oil", Ingredients: []fkrecipes.Ingredient{fkrecipes.IngredientNamed(2, "steel-plate"), fkrecipes.FluidIngredient(5, "lubricant")}},
	},
	Custom: quench,
},
```

```rust
ingredients_by: Some(IngredientChoices {
    setting: medium,
    choices: vec![
        IngredientChoice { value: "water".into(), ingredients: vec![Ingredient::named(2, "steel-plate", &[]), Ingredient::fluid(10.0, "water", &[])] },
        IngredientChoice { value: "oil".into(), ingredients: vec![Ingredient::named(2, "steel-plate", &[]), Ingredient::fluid(5.0, "lubricant", &[])] },
    ],
    custom: Some(quench),
    ..Default::default()
}),
```

The text applies only while the dropdown says `custom`; on any preset the preset applies and the text is ignored. The dropdown's description is composed for you: your own `[mod-setting-description]` entry, then one line per preset, its localised label followed by its ingredients written out in the language, so the player switching to `custom` can start from the preset they were on. A dropdown that lists `custom` with no preset behind it and no `Custom` arm is refused, as is a `Custom` arm on a dropdown that does not list it, and so is a dropdown that takes a `Custom` arm from two recipes, because its composed description can only describe one. A dropdown whose presets already include one named `custom` is an ordinary dropdown until you give it an arm. If your dropdown already uses the value `custom` for a preset of its own, name the arm's value with `CustomValue` (`custom_value`) instead of renaming the preset, which would reset every player who had chosen it. A text edited while the dropdown still says a preset does nothing, and the log says so once:

```
fkrecipes: steelworks-quench-ingredients is edited, but steelworks-quench-medium is not on custom, so the text is ignored
```

A stored dropdown value that is none of the values you offer cannot come through the settings screen, which resets it, but a file edited by hand can carry one; it is refused by name rather than read as any preset.

### Crafting time

`CraftTime` is a fixed number of seconds. Zero means "say nothing", and the engine applies its own default.

`CraftTimeFrom` binds the recipe to a double setting you declared, and the player chooses the seconds. The two are mutually exclusive and naming both is refused.

The engine will not load a recipe whose `energy_required` is at or below 0.001 (measured on Factorio 2.0.77, build 84539). The library holds that floor in three places: a declared `CraftTime` at or below it is refused, a setting that answers at or below it is refused with the setting named, and a double setting bound as a crafting time is given a `minimum_value` of 0.002 unless you declared a minimum of your own, so the settings screen cannot produce a value that kills the load. A declared minimum at or below the floor is refused too.

If the bound setting cannot be read, the declared default applies and a line goes to the log:

```
fkrecipes: the setting steelworks-forging-time was not readable, so its default applies
```

A recipe some technology unlocks is emitted with `enabled = false`, because the research is what turns it on. A recipe nothing unlocks is emitted enabled.

## Technologies

A technology needs exactly one source of cost and at most one anchor in the tree.

### Cost

`CostOf` names an existing technology and copies its whole `unit` unchanged. This is the intended way to price research: cost and tree position then come from one named point, and a multi-level source brings its `count_formula` and its `max_level` across without this library needing to evaluate either.

```go
lib.Technology("hardened-tips", fkrecipes.TechSpec{
	CostOf:      "physical-projectile-damage-7",
	EnabledBy:   bonuses,
	DisplayName: "Hardened tool tips",
})
```

```rust
lib.technology(
    "hardened-tips",
    TechSpec {
        cost_of: "physical-projectile-damage-7".into(),
        enabled_by: bonuses,
        display_name: "Hardened tool tips".into(),
        ..Default::default()
    },
);
```

Some base technologies are `research_trigger` technologies and carry no unit at all, so there is nothing to copy. Naming one is refused with the reason:

```
fkrecipes: CostOf(steam-power): steam-power is a research_trigger technology with no unit to copy; name a unit-carrying technology instead
```

`CostBy` is `CostOf` with a ladder per dropdown value: the player picks a tier, and the ladder is walked to the first technology that exists and carries a unit. That unit is copied verbatim with its `max_level`, and the source becomes the technology's sole prerequisite, so it does not combine with any of the placement fields. A `Fallback` unit applies when no rung works out. See [Migrating a mod that already ships settings](migration.md).

`Unit` is the escape hatch when no existing technology has the price you want. It takes a count, a time in seconds and a list of science packs. A pack is a presence ladder like an ingredient: `Pack{Name: "automation-science-pack", Amount: 1}` is a one-rung ladder, `Fallbacks` adds rungs, and a pack whose rungs are all absent is dropped with a log line. A unit whose packs all drop is refused, because a research with no packs is not something this library emits on your behalf, and a unit declared with no pack at all is refused for the same reason.

```go
Unit: &fkrecipes.UnitSpec{
	Count:   45,
	Seconds: 20,
	Packs:   []fkrecipes.Pack{{Name: "automation-science-pack", Amount: 1}},
},
```

```rust
unit: Some(UnitSpec {
    count: 45,
    seconds: 20.0,
    packs: vec![Pack::new("automation-science-pack", 1)],
}),
```

### Placement

`After` puts the technology behind one the game already has. If that technology is absent, because another mod removed it, the prerequisite is dropped and logged rather than guessed:

```
fkrecipes: steel-axes: quarry-drills is absent, so the prerequisite is dropped
```

`After` together with `Before` splices the technology between two existing ones: yours takes `After` as its prerequisite, and `Before` has `After` replaced by yours in its own prerequisite list. Two degradations are possible and both are logged rather than silently applied. If `Before` does not actually require `After`, yours is appended to its prerequisites instead of replacing anything. If `Before` is absent entirely, the splice degrades to a plain `After`:

```
fkrecipes: steel-axes: logistics-3 does not require electronics, so the new technology is appended to its prerequisites
fkrecipes: steel-axes: logistics-3 is absent, so InsertBetween degrades to After(logistics-2)
```

`Before` without `After` is refused: a splice needs both ends.

`AfterTech` anchors on a technology **your own plan** declares, which is how you chain your own research. It takes a handle rather than a name, so it cannot degrade and needs no presence check. It does not combine with `Before`, because a splice rearranges technologies that already exist.

```go
first := lib.Technology("hardened-steel", fkrecipes.TechSpec{CostOf: "logistics-2", After: "steel-processing"})
lib.Technology("steel-riveting", fkrecipes.TechSpec{CostOf: "logistics-2", AfterTech: first})
```

```rust
let first = lib.technology("hardened-steel", TechSpec { cost_of: "logistics-2".into(), after: "steel-processing".into(), ..Default::default() });
lib.technology("steel-riveting", TechSpec { cost_of: "logistics-2".into(), after_tech: first, ..Default::default() });
```

Before anything is emitted, the library walks the tree your plan is about to produce: every existing technology's prerequisites, with your splices applied and your own technologies added. A cycle is refused with the whole ring named in order.

```
fkrecipes: a prerequisite cycle: logistics-2 -> steel-processing -> steelworks-steel-axes -> logistics-3 -> logistics-2
```

### A research cost the player writes

`CostFrom` is `Unit` with its three numbers in the player's hands: a `PacksSetting` for the science packs, written as an ingredient list, an int setting for the count and a double setting for the seconds. The int setting must declare a minimum of at least 1 and the double a minimum above 0, because the engine refuses a unit with a count of 0 or a time of 0 (measured on Factorio 2.0.77); a `CustomCost` whose settings do not is refused at plan time, and the engine's own rule that an out-of-range stored value resets to the default keeps every value the library reads legal. Placement is the ordinary placement fields, as for `Unit`.

```go
count := lib.IntSetting("chain-count", 20, fkrecipes.Between(1, 100000))
seconds := lib.DoubleSetting("chain-seconds", 10, fkrecipes.Between(0.5, 600))
lib.Technology("chain-forging", fkrecipes.TechSpec{
	CostFrom: &fkrecipes.CustomCost{Packs: packs, Count: count, Seconds: seconds},
	After:    "steel-processing",
	Unlocks:  []fkrecipes.RecipeRef{chain},
})
```

```rust
let count = lib.int_setting("chain-count", 20, NumericSpec::between(1.0, 100000.0));
let seconds = lib.double_setting("chain-seconds", 10.0, NumericSpec::between(0.5, 600.0));
lib.technology("chain-forging", TechSpec {
    cost_from: Some(CustomCost { packs, count, seconds, position: vec![] }),
    after: "steel-processing".into(),
    unlocks: vec![chain],
    ..Default::default()
});
```

Only items the game treats as science packs (prototype type `tool`) are accepted in the pack list, and the word `default` means the packs you declared, with their fallbacks. The unit is emitted in the engine's short tuple form, and one line records what was read:

```
fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 25, time 12.5, packs 1 automation-science-pack, 1 logistic-science-pack
```

A `CostBy` dropdown takes the same thing as its `Custom` arm, with one addition: a `Position` ladder, walked to the first technology the game has, which becomes the sole prerequisite exactly as a chosen tier's source would, or no prerequisite with a log line when no rung exists. `Position` is required in a `Custom` arm and refused under `CostFrom`, where the placement fields already say where the technology goes.

```go
CostBy: &fkrecipes.CostChoices{
	Setting:  tier, // its values end with custom
	Choices:  tiers,
	Fallback: fallback,
	Custom:   &fkrecipes.CustomCost{Packs: tipsPacks, Count: tipsCount, Seconds: tipsSeconds, Position: []string{"military-2", "military"}},
},
```

```rust
cost_by: Some(CostChoices {
    setting: tier,
    choices: tiers,
    fallback,
    custom: Some(CustomCost { packs: tips_packs, count: tips_count, seconds: tips_seconds, position: vec!["military-2".into(), "military".into()] }),
    ..Default::default()
}),
```

### Unlocks and enablement

`Unlocks` takes handles to recipes your plan declared and emits them as `unlock-recipe` effects, in the order given.

`EnabledBy` takes a bool setting handle. When the setting is on, the technology is emitted enabled. When it is off, the technology is still emitted, with `enabled = false` and `hidden = true`. It is hidden rather than absent on purpose: a technology researched in an existing save whose prototype disappears is dropped from that save, and flipping a startup setting is exactly the mid-save event a configurable mod invites. A technology with no `EnabledBy` says nothing about enablement and takes the engine's default.

## Emit

`Emit` (`emit`) is the only call that touches `fkdata`. It reads the stage it is running in, plans, and then writes: settings prototypes at the settings stage, everything else at a data stage. A stage it does not plan for is refused by name rather than guessed at, so routing `Emit` somewhere it does not belong stops the load with a sentence saying so instead of running the wrong plan against a `data.raw` that is not there.

**Route each plan's `Emit` into `fk_settings` and exactly one data-family hook.** The three data stages (`fk_data`, `fk_data_updates`, `fk_data_final_fixes`) share one Lua state and one `data.raw`, so emitting **the same plan** twice would find the first pass's prototypes already there and refuse as an overwrite. Which stage you pick is yours: `fk_data` for content of your own, `fk_data_updates` to sit after other mods.

**A settings-only guest names `EmitSettings`.** `Emit` reaches both planners, so a guest that only generates settings still links the data planner: the pilot measured that as a 21,047-line Lua function that never runs, carried in every player's download. `EmitSettings` and `EmitData` plan one stage family each and let the linker drop the other. Each refuses if called at the wrong family, so a mis-routed hook is a sentence naming the hook rather than a confusing probe failure later.

```go
//go:wasmexport fk_settings
func onSettings() { plan().EmitSettings() }
```

**The rule is one data hook per plan, not one per mod.** A mod that wants both can carry two plans: one creating its own content at `fk_data`, one patching another mod's tree at `fk_data_updates`, each routed into `fk_settings` as well. Nothing special is needed to make that work, because the two plans are independent and the later one sees the earlier one's prototypes exactly as it sees any other mod's.

```go
var creation, patch = buildCreation(), buildPatch()

//go:wasmexport fk_settings
func onSettings() { creation.Emit(); patch.Emit() }

//go:wasmexport fk_data
func onData() { creation.Emit() }

//go:wasmexport fk_data_updates
func onDataUpdates() { patch.Emit() }
```

Two things follow from the plans being independent. **Their names must differ**, in settings and in prototypes: the overwrite refusal is what catches a collision between a plan and `data.raw`, and the duplicate-name refusal only ever sees inside one plan, so two plans that both declare `hardened-steel` are caught at the second one's data stage rather than at declaration. And the patching plan reaches the creating plan's technologies **by name**, not by handle: `AfterTech` takes a handle, and a handle is a fact about one plan.

When the plan is refused, the load stops with the sentence itself. `Emit` hands the message to `fkdata.Raise`, which is the same exit FkLua's own data-stage failures take: the host prefixes the stage and reports it, so the player reads one line naming the stage, this library and the declaration to fix.

```
fklua: at the data stage, fkrecipes: two technologies share the name hardened-tips; the second would overwrite the first
```

Every refusal in this document is shown the way the library builds it, without that host prefix. The stage is the host's to add, so nothing built here carries one; a refusal you read from `PlanSettings` or `PlanData` in your own host test is the bare `fkrecipes: ...` sentence.

**The build tag, and why you do not need it.** The emit layer is behind `//go:build tinygo.wasm` in Go and `cfg(target_family = "wasm")` in Rust, because the fkdata imports it uses are rejected off-target. A host build gets stub methods with the same names that panic:

```
fkrecipes: Emit runs only inside a wasm guest; this build is for the host
```

That is what lets `go vet ./...`, `go build ./...` and `cargo check` pass on your guest with no tag and no target: the tag belongs on the build that produces the wasm, not on every check you run. Your own host tests do not go through the stubs; they call `PlanSettings` and `PlanData`, below.

`PlanSettings` and `PlanData` are the seams `Emit` stands on. They are public so your own host tests can inspect a plan without a wasm toolchain, and they take the same World the emit layer implements over `fkdata`. Consumers call `Emit`.

A fixture World of your own should embed `fkrecipes.UnimplementedWorld` in Go; in Rust the trait's newer questions are default methods. Either way a question this library adds in a later version panics with its own name when your plan first asks it, instead of breaking your build the day you update:

```
fkrecipes: World.FluidExists is not implemented by this fixture
```

## Checking your locale file

`CheckLocale` reads your `.cfg` and returns one sentence per problem, empty for a clean file. It is pure and host-testable: run it from your own test suite, because nothing headless opens a settings menu and a data dump does not read locale.

```go
findings := plan().CheckLocale("steelworks", string(cfgBytes))
```

```rust
let findings = plan().check_locale("steelworks", &cfg);
```

It checks both directions across `[mod-setting-name]`, `[mod-setting-description]` and `[string-mod-setting]`:

- every generated setting needs a `[mod-setting-name]` entry, and an entry that is present but blank counts as missing because it renders as nothing;
- every dropdown value needs its own `[string-mod-setting]` entry, keyed `<prefixed-setting>-<value>`;
- an entry matching no setting or value of yours is reported as an orphan, because that is what a rename that was only half applied looks like;
- two dropdown values that would produce one locale key are reported, since `<setting>-<value>` is a flat namespace and the engine keeps whichever came last;
- a description is optional for a bool, int, double or plain dropdown setting and is never reported missing there, but a description matching nothing is still an orphan;
- a text setting (an ingredient list or a pack list) needs both a `[mod-setting-name]` entry and a `[mod-setting-description]` entry, because the description is where the player learns the format, and a dropdown with a `custom` arm needs a description too, since the library composes the preset texts onto it.

Sample output over a file missing one name and one dropdown value, and carrying two leftovers:

```
the dropdown setting steelworks-quench-medium has no [string-mod-setting] entry for its value oil
the setting steelworks-bonus-research has no [mod-setting-name] entry
the [mod-setting-name] entry steelworks-scrap-recovery matches no setting this plan declares
the [string-mod-setting] entry steelworks-quench-medium-brine matches no dropdown value this plan declares
```

**Pass the name your mod is packaged under.** Every other prefix in this library is derived from the packaged mod at emit time, where it cannot disagree; a host test has no `fkdata` to ask, so this one is a parameter.

A wrong name is a wrong prefix for every key at once, which for a plan carrying generated names shows up loudly: every setting reported missing and every entry reported orphaned, rather than as a subtle miss. **That loudness depends on the plan having at least one prefixed name.** A fully legacy plan derives no key from the mod name, so a wrong one changes nothing and the checker reports exactly what it would have reported anyway. If every setting you declare is a `Legacy` one, the mod-name argument is not load-bearing and no findings will tell you it was wrong.

Two limits worth knowing. In `[string-mod-setting]` only keys under one of your own dropdown settings are considered, so another mod's string setting in the same file is left alone. In the name and description sections the orphan rule is the mod prefix, so an entry matching no setting you declared is reported only when it carries that prefix: one under a name of its own, such as a setting you wrote by hand or a legacy name you have since renamed, is not reported at all. That second limit is a guess the checker refuses to make rather than one it cannot make, and `CheckLocaleWith` removes the need for it.

### Telling the checker what you declare elsewhere

`CheckLocaleWith` (`check_locale_with`) takes a third argument: the complete set of settings your mod declares **outside** this library, under whatever names they carry.

```go
findings := plan().CheckLocaleWith("better-belt-balancer", cfg,
	[]string{"bbb-multi-edge-parts"})
```

```rust
let findings = plan().check_locale_with("better-belt-balancer", &cfg, &["bbb-multi-edge-parts"]);
```

With that list the checker knows every setting name the mod has, so the `[mod-setting-name]` and `[mod-setting-description]` orphan scan stops being prefix-shaped and becomes complete: an entry matching no declared, legacy or hand-rolled name is an orphan whatever it is called. That closes the gap in both directions. A leftover entry from a setting you renamed is now reported even though it carries no mod prefix, and a hand-rolled setting that happens to carry the mod prefix is no longer reported for existing.

The list suppresses orphans; it does not create obligations. A name in it is never reported missing, because the library knows the name and nothing else: whether that setting is a dropdown needing per-value entries is your business, not something a name reveals. The value direction is unchanged for the same reason.

A name in the list that this plan also declares is a contradiction rather than a fact about your file, and is reported first:

```
the hand-rolled name bbb-recipe-cost is also a setting this plan declares; the list names only settings declared outside this library
```

**An empty list is not the same as the plain call, and it is the mistake worth naming.** Passing `nil` (`&[]`) is not a way to opt out; it is the assertion that your mod declares nothing outside this library, so every `[mod-setting-name]` and `[mod-setting-description]` entry in the file must match a declared or legacy setting. Anything else is reported, including another mod's entry that happens to share the file. That is the strictest reading available and it is the right one if you really do declare everything here.

Plain `CheckLocale` keeps the prefix rule, which is the right call when you have not enumerated the rest: it never invents an orphan, it just cannot see every one.

## Two things about `go mod tidy`

Both surprised the pilot, and neither is a defect in anything.

`go mod tidy` DELETES a require nothing imports. While your guest is a placeholder that does not yet import `fkdata`, tidy removes the require and both `go.sum` lines; the require comes back when the import does. Do not run tidy in that window and conclude the dependency was wrong.

The `fklua` require moves by MVS under a `replace`. If you point `github.com/Techrocket9/fkrecipes/go` at a checkout, your module still resolves `github.com/Techrocket9/fklua/guest/go` through minimal version selection, and it will be raised to whatever that checkout requires. Seeing `v0.0.0` become `v0.2.0` in your own go.mod is that, working correctly.

## Gates in this repository

Two scripts check the library end to end, and both need a local FkLua checkout.

[`scripts/run-mirror.sh`](../scripts/run-mirror.sh) builds both example guests, packages each with `fklua mod`, runs their settings and data stages under FkLua's Lua interpreter against a strict engine stand-in, and compares the two transcripts against each other and against a committed golden. The stand-in validates prototypes the way the engine does, so a prototype that would fail in game fails here.

[`scripts/run-ingame.sh`](../scripts/run-ingame.sh) runs both packaged mods in a real Factorio with `--dump-data` and hashes the normalised data and settings dumps against a golden keyed by engine version. It also asserts, with queries over the dump, the things a hash cannot name: that the prerequisite splice landed, that the bound crafting time reached the recipe, and that the copied research cost matches its source prototype in the same dump.

[Migrating a mod that already ships settings](migration.md) covers the surfaces an existing mod needs: settings whose names it keeps verbatim, and ingredients and research costs a dropdown chooses.

Maintainer design notes, including the measured engine behaviour these rules come from, live in the repository's `agents/` directory.
