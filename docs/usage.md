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

### Ingredients, and the resolve-or-drop contract

There are two ways to name an ingredient and neither takes a bare string you have not checked.

`IngredientOf` (`Ingredient::of`) names an item your own plan declares. It always resolves, because the same plan emits the prototype.

`IngredientNamed` (`Ingredient::named`) is a presence ladder over names the game may or may not have. The candidates are tried in order and the first one actually present is used. If none is present the ingredient is **dropped** and a line goes to the log:

```
fkrecipes: hardened-steel-plate-quenching: none of tungsten-carbide, titanium-plate is present, so the ingredient is dropped
```

Dropping rather than guessing is deliberate. An ingredient the game does not have is a hard load failure that names your mod, inside whatever overhaul pack the player has installed, and a substituted guess is a recipe you did not design. Use the ladder for anything optional, and put a staple last when the ingredient is required.

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
fkrecipes: at the data stage, CostOf(steam-power): steam-power is a research_trigger technology with no unit to copy; name a unit-carrying technology instead
```

`Unit` is the escape hatch when no existing technology has the price you want. It takes a count, a time in seconds and a list of science packs, each of which must exist.

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
    packs: vec![Pack { name: "automation-science-pack".into(), amount: 1 }],
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
fkrecipes: at the data stage, a prerequisite cycle: logistics-2 -> steel-processing -> steelworks-steel-axes -> logistics-3 -> logistics-2
```

### Unlocks and enablement

`Unlocks` takes handles to recipes your plan declared and emits them as `unlock-recipe` effects, in the order given.

`EnabledBy` takes a bool setting handle. When the setting is on, the technology is emitted enabled. When it is off, the technology is still emitted, with `enabled = false` and `hidden = true`. It is hidden rather than absent on purpose: a technology researched in an existing save whose prototype disappears is dropped from that save, and flipping a startup setting is exactly the mid-save event a configurable mod invites. A technology with no `EnabledBy` says nothing about enablement and takes the engine's default.

## Emit

`Emit` (`emit`) is the only call that touches `fkdata`. It reads the stage it is running in, plans, and then writes: settings prototypes at the settings stage, everything else at a data stage.

**Route `fk_settings` and exactly one data-family hook into it.** The three data stages (`fk_data`, `fk_data_updates`, `fk_data_final_fixes`) share one Lua state and one `data.raw`, so a second call would find the first pass's prototypes already there and refuse as an overwrite. Which one you pick is yours: `fk_data` for content of your own, `fk_data_updates` to sit after other mods.

When the plan is refused, the whole sentence is written to `factorio-current.log` and the load then stops. The player sees a failed data stage naming your mod; the sentence saying which declaration to fix is one line above it in the log. This is why the message goes to the log rather than the error dialog: a guest trap carries a code and no text, so the dialog cannot show it.

`PlanSettings` and `PlanData` are the seams `Emit` stands on. They are public so your own host tests can inspect a plan without a wasm toolchain, and they take the same World the emit layer implements over `fkdata`. Consumers call `Emit`.

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
- a description is optional and is never reported missing, but a description matching nothing is still an orphan.

Sample output over a file missing one name and one dropdown value, and carrying two leftovers:

```
the dropdown setting steelworks-quench-medium has no [string-mod-setting] entry for its value oil
the setting steelworks-bonus-research has no [mod-setting-name] entry
the [mod-setting-name] entry steelworks-scrap-recovery matches no setting this plan declares
the [string-mod-setting] entry steelworks-quench-medium-brine matches no dropdown value this plan declares
```

**Pass the name your mod is packaged under.** Every other prefix in this library is derived from the packaged mod at emit time, where it cannot disagree; a host test has no `fkdata` to ask, so this one is a parameter. A wrong name is a wrong prefix for every key at once, which shows up as every setting reported missing and every entry reported orphaned rather than as a subtle miss.

Two limits worth knowing. In `[string-mod-setting]` only keys under one of your own dropdown settings are considered, so another mod's string setting in the same file is left alone. In the name and description sections the only available rule is the mod prefix, so a setting you wrote by hand rather than declaring through this library is reported as an orphan.

## Gates in this repository

Two scripts check the library end to end, and both need a local FkLua checkout.

[`scripts/run-mirror.sh`](../scripts/run-mirror.sh) builds both example guests, packages each with `fklua mod`, runs their settings and data stages under FkLua's Lua interpreter against a strict engine stand-in, and compares the two transcripts against each other and against a committed golden. The stand-in validates prototypes the way the engine does, so a prototype that would fail in game fails here.

[`scripts/run-ingame.sh`](../scripts/run-ingame.sh) runs both packaged mods in a real Factorio with `--dump-data` and hashes the normalised data and settings dumps against a golden keyed by engine version. It also asserts, with queries over the dump, the things a hash cannot name: that the prerequisite splice landed, that the bound crafting time reached the recipe, and that the copied research cost matches its source prototype in the same dump.

Maintainer design notes, including the measured engine behaviour these rules come from, live in the repository's `agents/` directory.
