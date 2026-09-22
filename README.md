# FkRecipes

FkRecipes ("Factorio: konfigurierbare Recipes") is a guest library for Factorio mods built with FkLua. Your mod declares items, crafting recipes and technologies in Go or Rust; the library turns them into prototypes at the data stage and generates the startup settings that let a player configure them from the mod settings screen.

Two kinds of configuration are wired for you. A technology's enablement binds to a generated bool setting: switch it off and the technology is emitted hidden rather than dropped, so a save that already researched it does not lose it. A recipe's crafting time binds to a generated double setting: the player picks the seconds, and the library gives that setting a minimum the engine will accept.

Everything is validated before a prototype reaches the game, because the engine's own diagnostics for these failures are thin. Factorio reports a prerequisite cycle without naming any of the technologies in it; this library names the whole ring, in order, and where the loop closes through an edge your plan made it drops that edge and says so in the technology's own tooltip instead of stopping the load. It also checks that every name you reference is actually present, that a research cost you copy has a shape it can copy, and that a crafting time clears the engine's floor. The Go and Rust halves are written to produce identical output, and two gates in this repository check that against a stand-in and against a real Factorio.

## Status

Both halves implement the v1 surface and are mirrored: the same verbs, the same refusal text byte for byte, and the same emitted prototypes. The library has host tests in both languages, a harness that packages both example mods and compares their behaviour byte for byte, and a gate that runs both in a real Factorio and hashes the result against a committed golden.

The current release is tagged: the Go module as `go/v0.1.1` (require `github.com/Techrocket9/fkrecipes/go v0.1.1`) and the Rust crate as `rust/v0.1.1` (a git dependency on this repository at that tag). v0.1.1 is the release to be on for Factorio 2.1: under v0.1.0 a recipe that declares a crafting category stops the load on that engine, and every research this library prices comes out free. The surface may still change before 1.0. The quickstart below shows the release form and the checkout form.

## Requirements

- Go 1.24 with TinyGo, or Rust 2021 with the `wasm32-unknown-unknown` target, matching the half you consume.
- An FkLua checkout for `fklua mod`, which packages your guest into a mod. See [FkLua's data-stage documentation](https://github.com/Techrocket9/FkLua/blob/master/docs/data-stage.md).
- Factorio 2.0 or 2.1 to run the result. The library is measured on both and asks each one its own questions; the mod you package has to declare the one the player runs.

## Quickstart

Your mod owns the stage exports; this library never declares one. Route `fk_settings` and one data-family hook into `Emit`, which plans and writes. That is one data hook per plan rather than per mod: a mod may carry a second plan on another data stage, which [the usage guide](docs/usage.md) shows.

### Go

The Go half is the module `github.com/Techrocket9/fkrecipes/go`, rooted in this repository's `go/` directory.

```
require (
	github.com/Techrocket9/fklua/guest/go v0.2.0
	github.com/Techrocket9/fkrecipes/go v0.1.1
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

Both guests above declare the same mod. Package either with `fklua mod --data-module datastage.wasm --name your-mod --factorio-version <series>`, where `<series>` is the Factorio series the player runs, `2.0` or `2.1`. IT HAS TO MATCH: measured on 2.1.17, a mod whose `info.json` declares `2.0` is refused at game start, before a line of it runs, with `Incompatible Factorio version (current: 2.1, required: 2.0)`.

## What it does

- **Generates the settings.** Bool, int, double, dropdown and ingredient-list text startup settings, named after your mod, ordered as you declared them or placed under a legacy order you name. A double setting bound as a crafting time is given a minimum of 0.002 unless you set one yourself.
- **Prefixes everything.** The prefix comes from the mod name FkLua packaged, read at emit time. There is no prefix parameter, so a generated name cannot drift from the mod it ships in.
- **Resolves ingredient names, or drops them.** `IngredientNamed` takes a list of candidates and uses the first one the game actually has. If none is present the ingredient is dropped and a line is written to the log, because a name the game does not have is a hard load failure that names your mod, and a guess is worse than an omission.
- **Copies a research cost from a technology you name.** `CostOf` takes the source's whole `unit`, filtering only its science packs, so a `count_formula` and a multi-level technology's `max_level` come across without this library needing to understand either. A pack the player's mod set removed, or demoted out of whatever the running engine treats as a science pack, is left out of the copy with a line rather than handed to an engine that stops the load over it. What a science pack IS is the engine's answer and not a fixed one: a prototype of type `tool` on Factorio 2.0, an item whose subgroup is `science-pack` on 2.1, where base and its bundled expansions declare no `tool` prototype at all. A mod may still declare one there, and the library still finds it.
- **Lets a setting choose the ingredients or the cost.** `IngredientsBy` binds a recipe's whole ingredient list to a dropdown, one plan per value. `CostBy` binds a technology's research cost to a dropdown, walking a ladder of source technologies per value to the first one the game actually has; the source it settles on also becomes the technology's prerequisite. Several declarations may name one dropdown, and a setting carries one description, so `Describes` says which of them the dropdown shows. A cost option can say what it costs in your own words, with `Display`, which is the answer where the ladder's first rung is a technology only some mod sets have.
- **Lets the plan write a description.** `DescribeSetting` takes any setting handle and a literal, and stands in for that setting's `[mod-setting-description]` entry. A locale entry is one string for every mod set; a plan that already branches on what is installed can say the true thing for the game actually running.
- **Lets the player write the recipe.** `IngredientsFrom` binds a recipe's ingredients to a text setting holding an ingredient list, `2 iron-plate, 3 copper-cable`, checked at load with a sentence per mistake and never guessed at; `CostFrom` does the same for a research cost with its count and seconds beside it. Either may be declared beside a dropdown of presets, and then the text is the switch: while it says `default` the dropdown decides, and anything else applies instead. The dropdown keeps its exact option list, so adopting the customizer on one a mod already ships disturbs no stored choice at all. The text is documented in [The ingredient list](docs/ingredient-list.md).
- **Keeps the names an existing mod already ships.** Four `Legacy` setting constructors take a full name and an explicit order and emit both verbatim, because Factorio persists startup values by name with no rename mechanism and a regenerated name resets every existing save to its default. `LegacyItem`, `LegacyRecipe` and `LegacyTechnology` do the same for prototypes, where the stakes are higher: a save references a prototype by name, and a renamed one is an `assignID` abort rather than a reset.
- **Gets out of the way for fields it has no slot for.** `Order` and `PlaceResult` are slots; `Extra` passes any other prototype field through verbatim, refusing only a key this library writes itself. `ResultNamed` lets a recipe produce an item this plan does not declare.
- **Places a technology in the tree.** Behind an existing one, between two of them, or behind another technology from your own plan. If an endpoint is missing because another mod removed it, the placement degrades and logs the degradation rather than guessing a substitute.
- **Hides rather than deletes.** A technology whose bool setting is off is emitted with `enabled = false` and `hidden = true`. A prototype that vanishes is dropped from any save that had researched it, and flipping a startup setting is exactly the mid-save event this library invites.
- **Degrades what the mod set broke, and says so where the player looks.** A check that asks the game what is installed uses the nearest thing that works and writes a line; a check that reads only your declaration refuses by name. So a missing science pack is dropped from a copied cost, a cost whose packs are all gone falls back to the one you declared, a research left with no usable science pack anywhere is emitted with none rather than stopping the load, a copied cost written in a form the library cannot read falls back or empties rather than stopping it, a prerequisite that would close a loop is dropped, and two ladder rungs collapsing above an amount ceiling are capped at it. Every one of those also puts one trailing English sentence into the recipe's or technology's own `localised_description`, because the log is evidence for you and a tooltip is where the player looks.
- **Refuses before the game does.** Duplicate names, a name your plan would overwrite, a `CostOf` source that does not exist or is a `research_trigger` technology and therefore has no unit to copy, a `PlaceResult` or a `ResultNamed` naming a prototype the game does not have, a crafting time the engine will not take, a prerequisite cycle your plan is not part of, and handles from a different plan.
- **Checks your locale file.** `CheckLocale` is a host-side function for your own tests: it reads your `.cfg` and reports both settings with no entry and entries matching no setting, including the per-value entries a dropdown needs. Keys inside your mod's prefix are required, and a clean file returns nothing. A setting the plan describes inline needs no description entry, and one it still has is reported as text the engine never shows. The one key composed outside it, the `technology-name` a cost preset names, is never required and is not in that report at all: `CheckLocaleAdvisories` returns it as information, because Factorio's locale namespace is flat and defining that entry would rename the technology for every mod in the game.
- **Composes every locale key so a missing one is survivable.** A key the library writes into a description goes out in the engine's alternatives form with a raw fallback last, so where the game has no entry the tooltip shows the internal name instead of losing the whole tooltip. Measured on Factorio 2.0.77 (build 84539): an unwrapped key the game does not define leaves a setting row with no info icon and no tooltip at all, and deletes a recipe's entire description, with the load still exiting 0 and no warning anywhere.

## What it does not do

- No control stage. It touches `fkdata` and nothing else, in both languages, and never `fkapi` under any feature. That is what makes it pin-transparent: no FkLua API pin move can break it.
- No deleting or hiding another mod's content. The one thing it writes outside its own prototypes is a prerequisite splice into a technology you name.
- No arithmetic on a `count_formula`. A formula is copied as the string it is; scaling one would need an expression evaluator.
- No enumeration helpers or cached indexes. Reads go through the World interface one question at a time.
- No unprefixed names, and no prefix parameter to get wrong.

## Measured behaviour

Numbers here carry the environment that produced them.

**What a mod set can stop the load over, and the one measurement that decides it.** Three things, and none of them is a science pack: a name your plan would overwrite in `data.raw`, a bare name with no declared ladder behind it (a `CostOf` source, a `PlaceResult`, a `ResultNamed`), and a prerequisite ring your plan holds no edge in. Everything else a mod set can do degrades with a line in the log and a sentence in the recipe's or technology's own tooltip. The case that used to be the exception was a research left with no usable science pack, and what settled it is a measurement rather than an argument: on Factorio 2.0.77 (build 84539, mac-arm64, steam), a technology whose `unit` carries `{count = 10, time = 15, ingredients = {}}` loads with exit 0 and no engine line, `force.add_research` returns true, `research_progress` advances every tick in a lab holding no science pack at all, and the technology completes after `count * time` ticks having consumed nothing. A packless research is not a stuck one, it is a free one, so emitting it is a balance change the player did not choose, and it is disclosed where a player looks instead of being paid for with a game that will not start.

**The crafting-time floor.** Factorio 2.0.77 (build 84539, mac-arm64, steam) refuses to load a recipe whose `energy_required` is at or below 0.001, with `energy_required can't be <= 0.001`. Values of 0.0011 and 0.002 load and survive to a data dump unchanged; an omitted `energy_required` stays absent and the engine applies its own default. The library refuses a declared crafting time at or below the floor and gives a generated craft-time setting a minimum of 0.002 so the settings screen cannot produce one; a setting that answers below the floor anyway falls back to its declared default and logs a line, because a stored value is the player's.

**What a refusal looks like in game.** A refusal is about something the MOD declares, and the load stops with the sentence itself, through `fkdata.Raise`. FkLua's data-stage runtime prefixes the stage and reports it the way it reports its own failures, so the player reads one line naming the stage, this library and the declaration to fix:

```
fklua: at the data stage, fkrecipes: two technologies share the name hardened-tips; the second would overwrite the first
```

The stage comes from the host, which is why nothing this library builds carries a stage of its own.

**What the player types is never the reason a load stops.** A startup setting a player edits, an ingredient list or a research number, never refuses on its own: the value is set aside, the mod loads on the list or number it declared, and one line goes to the log naming the setting, quoting the problem and saying where to fix it. What the mod declared is then held to the same rules it always was, and the few of those a mod set can still reach are in the next paragraph; where one is reached the load stops with the message a player who typed nothing would have read, and when a stored value was set aside on the way there the message carries one more sentence saying so and naming the setting, because none of those log lines reaches the game on a failed load. That sentence is a fact and not a route: it tells the player their value was set aside and sends them nowhere, because the error dialog can reach nowhere. Factorio rewrites `mod-settings.dat` on every successful load and on no failed one, and the `Error loading mods` dialog cannot reach the Mod Settings screen (`Manage mods` shows only the Mods list and its Back returns to the dialog), so a refusal over a typed value is a state the player cannot get out of without resetting every startup setting they have. Measured on Factorio 2.0.77 (build 84539, mac-arm64, steam).

```
fkrecipes: ERROR: mymod-parts, entry 1 ("2 iron-plat"): no item or fluid is named iron-plat. The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.
```

**What it costs to adopt.** Two figures, because the wasm one understates what ships.

A minimal guest that declares one item and one recipe through this library is 370,859 bytes of wasm; the same guest built against `fkdata` alone is 42,285 bytes, so the library adds about 320 KiB of wasm. Measured 2026-08-31 with TinyGo 0.41.1, target `wasm-unknown`, `-scheduler=none -gc=leaking -opt=2`. Those Go figures are three quarters DWARF: the example guest is 996,001 bytes of wasm with those flags and 228,773 with `-no-debug` added (measured 2026-09-08). The stripped build is not the same module once packaged, because TinyGo inlines differently without debug information (the widest function's jump span grows by a third), so the figures here and the gates keep the flags a shipping guest uses; the packaged Lua is about 0.7 percent smaller with the flag, and both gates pass with it.

What a player downloads and the engine parses is the LOWERED LUA, which is larger. BetterBeltBalancer measured its own adoption end to end: `fk_data_module.lua` 1,763,788 to 3,216,828 bytes (+82.4 percent), the packaged zip 583,756 to 665,986 bytes (+14.1 percent), and about +0.10 s of parse time per load across three stages. Nothing on any tick moved, because this library runs only at load. If a settings-only guest is what you need, `EmitSettings` lets the linker drop the data planner rather than carrying it unused.

**A plan that declares no text setting links no parser, no renderer, no amount formatter and no custom-cost resolver.** Those four are reached only through `IngredientsSetting` and `PacksSetting` (`ingredients_setting`, `packs_setting`), so a mod whose settings are bools, numbers and dropdowns carries none of the code that reads a player's typed list, and none of the language's own messages: `invisible character`, `is longer than`, `stands alone` and `no item or fluid is named` are absent from both fixture modules and present in both example guests. Those four and not everything. The descriptions a text setting, a research number and a dropdown are emitted with are composed behind checks on the declaration that run at load rather than behind that link-time seam, so their prose ships in every guest: 1,142 bytes of string data over 25 literals, twenty-four of them verified present in both packaged fixtures below by searching for the literal itself. The twenty-fifth is the two-byte `: ` an authored cost override composes in place of the library's own tail, which is too short to locate that way and is counted from the source; it is in the total so that the enumeration is complete rather than nearly so. They are the format line's two pieces (47 and 12 bytes) and its ingredient-only `none` clause (70), the fallback line (109), the preset head (12), the switch lines (45 and 40 around the word `above` or `below`, 86 for the standalone arm, 13 and 47 for the dropdown's own, and 5 each for the two words), the range lines (39 and 53 beside a dropdown, 21 and 4 without one, and the `.` they share), the three ladder sentences (153, 155 and 161), a cost preset's three pieces (10, 19 and the 2 of the separator an authored override composes) and the frame that holds them (`\ndefault: ` at 10 and `mod-setting-description` at 23). Measured on the fixtures in [`go/examples/notext`](go/examples/notext) and [`rust/examples/notext`](rust/examples/notext), a BetterBeltBalancer-shaped plan with two dropdowns over `IngredientsBy` and `CostBy` and no text setting, packaged by `fklua mod`: the Go guest's `fk_data_module.lua` is 3,107,518 bytes (78,849 lines) and the Rust guest's 2,675,831 bytes (65,451 lines). Both figures were measured 2026-09-22 with TinyGo 0.41.1 (`-scheduler=none -gc=leaking -opt=2`, `GOTOOLCHAIN=go1.26.6`), cargo 1.97.1 (`opt-level = "s"`, LTO) and an `fklua` built from [FkLua](https://github.com/Techrocket9/FkLua) at `16508c8`; the commands that produce them are in the maintainer notes, [`agents/implementation-notes.md`](agents/implementation-notes.md). A packaged size is a figure for the `fklua` head it was taken at as much as for this library, so compare two of them only when both name the same head.

**Wasm size is not a proxy for what a player downloads.** The example mod in this repository compiles to 387,267 bytes of wasm from Go and 81,536 bytes from Rust, a factor of 4.7; packaged by `fklua mod` those become 1,464,452 and 1,506,398 bytes of Lua respectively, so the Rust guest produces slightly more of what actually ships. Measured 2026-08-31.

## Repository layout

- `go/` is the Go half, module `github.com/Techrocket9/fkrecipes/go`. Everything except the emit layer is host-testable with plain `go test`.
- `rust/` is the Rust half, crate `fkrecipes`. The emit layer sits behind `cfg(target_family = "wasm")`, so `cargo test` needs no wasm target.
- [`go/examples/datastage`](go/examples/datastage) and [`rust/examples/datastage`](rust/examples/datastage) are one small mod written twice: a steelworks expansion declaring the same settings, items, recipes and technologies in both languages.
- [`scripts/run-mirror.sh`](scripts/run-mirror.sh) packages both example mods and runs their settings and data stages against a strict engine stand-in, comparing the two transcripts and a committed golden byte for byte.
- [`scripts/run-ingame.sh`](scripts/run-ingame.sh) runs both in a real Factorio with `--dump-data` and hashes the result against a golden keyed by engine version. It also runs both against a second mod set, a fixture that demotes a science pack from a `tool` to a plain `item`, and checks that the load does not stop and that the degradation reaches the log and the technology's own tooltip.
- `testdata/` holds those goldens and the locale checker's fixture.
- [`docs/usage.md`](docs/usage.md) is the consumer's tour of every verb.
- [`docs/migration.md`](docs/migration.md) is the path for a mod that already ships hand-rolled settings, with BetterBeltBalancer as the worked example.

## Licence

MIT, see [LICENSE](LICENSE).
