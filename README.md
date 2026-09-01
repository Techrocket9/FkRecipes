# FkRecipes

FkRecipes ("Factorio: konfigurierbare Recipes", joining the [FkLua](https://github.com/Techrocket9/FkLua) naming pattern) is a guest library for Factorio mods built with FkLua. Your mod declares items, crafting recipes and technologies in Go or Rust; the library turns them into prototypes at the data stage and generates the startup settings that let a player configure them from the mod settings screen.

Two kinds of configuration are wired for you. A technology's enablement binds to a generated bool setting: switch it off and the technology is emitted hidden rather than dropped, so a save that already researched it does not lose it. A recipe's crafting time binds to a generated double setting: the player picks the seconds, and the library gives that setting a minimum the engine will accept.

Everything is validated before a prototype reaches the game, because the engine's own diagnostics for these failures are thin. Factorio reports a prerequisite cycle without naming any of the technologies in it; this library refuses the plan and names the whole ring, in order. It also checks that every name you reference is actually present, that a research cost you copy has a shape it can copy, and that a crafting time clears the engine's floor. The Go and Rust halves are written to produce identical output, and two gates in this repository check that against a stand-in and against a real Factorio.

## Status

Both halves implement the v1 surface and are mirrored: the same verbs, the same refusal text byte for byte, and the same emitted prototypes. The library has host tests in both languages, a harness that packages both example mods and compares their behaviour byte for byte, and a gate that runs both in a real Factorio and hashes the result against a committed golden.

There is no published version tag yet, so both halves are consumed from a checkout rather than from a release, and the surface may still change. The quickstart below shows both forms.

## Requirements

- Go 1.24 with TinyGo, or Rust 2021 with the `wasm32-unknown-unknown` target, matching the half you consume.
- An FkLua checkout for `fklua mod`, which packages your guest into a mod. See [FkLua's data-stage documentation](https://github.com/Techrocket9/FkLua/blob/master/docs/data-stage.md).
- Factorio 2.0 to run the result.

## Quickstart

Your mod owns the stage exports; this library never declares one. Route `fk_settings` and one data-family hook into `Emit`, which plans and writes. That is one data hook per plan rather than per mod: a mod may carry a second plan on another data stage, which [the usage guide](docs/usage.md) shows.

### Go

The Go half is the module `github.com/Techrocket9/fkrecipes/go`, rooted in this repository's `go/` directory.

```
require (
	github.com/Techrocket9/fklua/guest/go v0.2.0
	github.com/Techrocket9/fkrecipes/go v0.1.0
)

// Until a version is tagged, point the require at a checkout:
replace github.com/Techrocket9/fkrecipes/go => ../FkRecipes/go
```

```go
package main

import fkrecipes "github.com/Techrocket9/fkrecipes/go"

// Both stages call this: the module is instantiated fresh per stage, so the
// declarations run again each time and the plan is rebuilt from them.
func plan() *fkrecipes.Lib {
	lib := fkrecipes.New()
	hardened := lib.BoolSetting("hardened-tools", true)
	forging := lib.DoubleSetting("forging-time", 3, fkrecipes.NumericSpec{HasMax: true, Max: 120})

	plate := lib.Item("hardened-steel-plate", fkrecipes.ItemSpec{
		Icon:        "__steelworks__/graphics/icons/hardened-steel-plate.png",
		IconSize:    64,
		StackSize:   100,
		DisplayName: "Hardened steel plate",
	})
	quenching := lib.Recipe(plate, fkrecipes.RecipeSpec{
		Name:          "hardened-steel-plate-quenching",
		CraftTimeFrom: forging,
		Ingredients: []fkrecipes.Ingredient{
			fkrecipes.IngredientNamed(2, "tungsten-plate", "steel-plate"),
		},
	})
	lib.Technology("hardened-steel", fkrecipes.TechSpec{
		CostOf:    "logistics-2",
		After:     "steel-processing",
		Before:    "logistics-2",
		Unlocks:   []fkrecipes.RecipeRef{quenching},
		EnabledBy: hardened,
	})
	return lib
}

//go:wasmexport fk_settings
func onSettings() { plan().Emit() }

//go:wasmexport fk_data
func onData() { plan().Emit() }

func main() {}
```

Build it as any FkLua data guest: `tinygo build -target=wasm-unknown -scheduler=none -gc=leaking -opt=2 -o datastage.wasm .`

### Rust

The Rust half is the crate `fkrecipes`, rooted in this repository's `rust/` directory. Declare it wasm-gated, so a host `cargo test` in your own project never needs a wasm target.

```toml
[target.'cfg(target_family = "wasm")'.dependencies]
fkrecipes = { path = "../FkRecipes/rust" }
fkdata = { git = "https://github.com/Techrocket9/fklua" }
fk = { git = "https://github.com/Techrocket9/fklua" }
```

This crate pulls `fkdata` and `fk` from git. If your project consumes FkLua from a vendored checkout instead, patch that git source onto your paths as well, or two copies of `fk` land in one build and its allocator is defined twice:

```toml
[patch."https://github.com/Techrocket9/fklua"]
fkdata = { path = "../FkLua/guest/rust/fkdata" }
fk     = { path = "../FkLua/guest/rust/fk" }
```

```rust
use fkrecipes::{Ingredient, ItemSpec, Lib, NumericSpec, RecipeSpec, TechSpec};

fn plan() -> Lib {
    let mut lib = Lib::new();
    let hardened = lib.bool_setting("hardened-tools", true);
    let forging = lib.double_setting("forging-time", 3.0, NumericSpec { min: None, max: Some(120.0) });

    let plate = lib.item(
        "hardened-steel-plate",
        ItemSpec {
            icon: "__steelworks__/graphics/icons/hardened-steel-plate.png".into(),
            icon_size: 64,
            stack_size: 100,
            display_name: "Hardened steel plate".into(),
            ..Default::default()
        },
    );
    let quenching = lib.recipe(
        plate,
        RecipeSpec {
            name: "hardened-steel-plate-quenching".into(),
            craft_time_from: forging,
            ingredients: vec![Ingredient::named(2, "tungsten-plate", &["steel-plate"])],
            ..Default::default()
        },
    );
    lib.technology(
        "hardened-steel",
        TechSpec {
            cost_of: "logistics-2".into(),
            after: "steel-processing".into(),
            before: "logistics-2".into(),
            unlocks: vec![quenching],
            enabled_by: hardened,
            ..Default::default()
        },
    );
    lib
}

#[no_mangle]
pub extern "C" fn fk_settings() { plan().emit(); }

#[no_mangle]
pub extern "C" fn fk_data() { plan().emit(); }
```

Build it with `cargo build --release --target wasm32-unknown-unknown`.

Both guests above declare the same mod. Package either with `fklua mod --data-module datastage.wasm --name your-mod --factorio-version 2.0`.

## What it does

- **Generates the settings.** Bool, int, double and dropdown startup settings, named after your mod, ordered as you declared them. A double setting bound as a crafting time is given a minimum of 0.002 unless you set one yourself.
- **Prefixes everything.** The prefix comes from the mod name FkLua packaged, read at emit time. There is no prefix parameter, so a generated name cannot drift from the mod it ships in.
- **Resolves ingredient names, or drops them.** `IngredientNamed` takes a list of candidates and uses the first one the game actually has. If none is present the ingredient is dropped and a line is written to the log, because a name the game does not have is a hard load failure that names your mod, and a guess is worse than an omission.
- **Copies a research cost from a technology you name.** `CostOf` takes the source's whole `unit` unchanged, so a `count_formula` and a multi-level technology's `max_level` come across without this library needing to understand either.
- **Lets a setting choose the ingredients or the cost.** `IngredientsBy` binds a recipe's whole ingredient list to a dropdown, one plan per value. `CostBy` binds a technology's research cost to a dropdown, walking a ladder of source technologies per value to the first one the game actually has; the source it settles on also becomes the technology's prerequisite.
- **Keeps the names an existing mod already ships.** Four `Legacy` constructors take a full setting name and an explicit order and emit both verbatim, because Factorio persists startup values by name with no rename mechanism and a regenerated name resets every existing save to its default.
- **Places a technology in the tree.** Behind an existing one, between two of them, or behind another technology from your own plan. If an endpoint is missing because another mod removed it, the placement degrades and logs the degradation rather than guessing a substitute.
- **Hides rather than deletes.** A technology whose bool setting is off is emitted with `enabled = false` and `hidden = true`. A prototype that vanishes is dropped from any save that had researched it, and flipping a startup setting is exactly the mid-save event this library invites.
- **Refuses before the game does.** Prerequisite cycles (with the full ring named), duplicate names, a name your plan would overwrite, a `CostOf` source that is a `research_trigger` technology and therefore has no unit to copy, a science pack that does not exist, a crafting time the engine will not take, and handles from a different plan.
- **Checks your locale file.** `CheckLocale` is a host-side function for your own tests: it reads your `.cfg` and reports both settings with no entry and entries matching no setting, including the per-value entries a dropdown needs and has no fallback for.

## What it does not do

- No control stage. It touches `fkdata` and nothing else, in both languages, and never `fkapi` under any feature. That is what makes it pin-transparent: no FkLua API pin move can break it.
- No deleting or hiding another mod's content. The one thing it writes outside its own prototypes is a prerequisite splice into a technology you name.
- No arithmetic on a `count_formula`. A formula is copied as the string it is; scaling one would need an expression evaluator.
- No enumeration helpers or cached indexes. Reads go through the World interface one question at a time.
- No unprefixed names, and no prefix parameter to get wrong.

## Measured behaviour

Numbers here carry the environment that produced them.

**The crafting-time floor.** Factorio 2.0.77 (build 84539, mac-arm64, steam) refuses to load a recipe whose `energy_required` is at or below 0.001, with `energy_required can't be <= 0.001`. Values of 0.0011 and 0.002 load and survive to a data dump unchanged; an omitted `energy_required` stays absent and the engine applies its own default. The library refuses a declared crafting time at or below the floor, refuses a setting value that answers below it, and gives a generated craft-time setting a minimum of 0.002 so the settings screen cannot produce one.

**What a refusal looks like in game.** The load stops with the sentence itself, through `fkdata.Raise`. FkLua's data-stage runtime prefixes the stage and reports it the way it reports its own failures, so the player reads one line naming the stage, this library and the declaration to fix:

```
fklua: at the data stage, fkrecipes: two technologies share the name hardened-tips; the second would overwrite the first
```

The stage comes from the host, which is why nothing this library builds carries a stage of its own.

**Flash cost.** A minimal guest that declares one item and one recipe through this library is 370,859 bytes of wasm; the same guest built against `fkdata` alone is 42,285 bytes. The library therefore adds about 320 KiB. Measured 2026-08-31 with TinyGo 0.41.1, target `wasm-unknown`, `-scheduler=none -gc=leaking -opt=2`.

**Wasm size is not a proxy for what a player downloads.** The example mod in this repository compiles to 387,267 bytes of wasm from Go and 81,536 bytes from Rust, a factor of 4.7; packaged by `fklua mod` those become 1,464,452 and 1,506,398 bytes of Lua respectively, so the Rust guest produces slightly more of what actually ships. Measured 2026-08-31.

## Repository layout

- `go/` is the Go half, module `github.com/Techrocket9/fkrecipes/go`. Everything except the emit layer is host-testable with plain `go test`.
- `rust/` is the Rust half, crate `fkrecipes`. The emit layer sits behind `cfg(target_family = "wasm")`, so `cargo test` needs no wasm target.
- [`go/examples/datastage`](go/examples/datastage) and [`rust/examples/datastage`](rust/examples/datastage) are one small mod written twice: a steelworks expansion declaring the same settings, items, recipes and technologies in both languages.
- [`scripts/run-mirror.sh`](scripts/run-mirror.sh) packages both example mods and runs their settings and data stages against a strict engine stand-in, comparing the two transcripts and a committed golden byte for byte.
- [`scripts/run-ingame.sh`](scripts/run-ingame.sh) runs both in a real Factorio with `--dump-data` and hashes the result against a golden keyed by engine version.
- `testdata/` holds those goldens and the locale checker's fixture.
- [`docs/usage.md`](docs/usage.md) is the consumer's tour of every verb.
- [`docs/migration.md`](docs/migration.md) is the path for a mod that already ships hand-rolled settings, with BetterBeltBalancer as the worked example.

## Licence

MIT, see [LICENSE](LICENSE).
