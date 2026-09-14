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

Two more constructors declare a text setting the player edits: an ingredient list, or a list of science packs. The setting's default text is the word `default`, which means the list you give here. The library composes five lines onto the setting's description, under whatever your own `[mod-setting-description]` entry says: a default line, which is your list written out in the form documented in [The ingredient list](ingredient-list.md), and then four sentences of its own. The wrap sentence says that a list too long for one line continues on the next and the continuation is part of the same list. The format sentence says the field takes internal names and at most 2000 characters. The switch sentence says which field decides while this one holds the word `default`: the dropdown above or below it when you declared one beside it, otherwise your own list. The fallback sentence says that a text the library cannot use is set aside and that the field then behaves as though it said `default`, with the reason in the log or in the load error. On an ingredient setting the format sentence also names the word `none`, which empties the list and makes the recipe free to craft; on a science-pack setting it does not, because a pack list turns that word down. Your entry says what the setting is for; the library says how to fill it in and what it costs to get it wrong. That last line states the narrow claim rather than promising a load, because the narrow claim is the one that holds: a fallback lands where the field's reserved word would have landed, which is the preset the dropdown beside it is currently on where you declared one and your declared list where you did not, and whatever it lands on is held to the rules it always was, so a modpack in which that cannot produce a legal result still stops the load, and on a failed load no log line reaches the game at all. See [Ingredients the player writes](#ingredients-the-player-writes) below for the whole rule.

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

A text setting must be bound to exactly one recipe or technology (below); one that is declared and bound nowhere is refused, because a setting the player can edit that nothing reads is a mistake. The two constructors are also the only place the ingredient language is linked from: a plan that never calls them ships no parser, no renderer, no amount formatter and no custom-cost resolver, so a mod whose settings are bools, numbers and dropdowns carries none of the code that reads a player's typed list. Those four and not everything: the description a text setting is emitted with is composed behind a check on the setting's kind that runs at load rather than behind that link-time seam, so 234 bytes of its prose ship in every guest, text setting or none, in a format line built from two literals, a fallback line and a preset head (the README carries the measured figures, taken on fixtures kept for the question). A default that names an item of this plan renders it under its emitted, prefixed name, and the player can name that item the same way: the resolver counts the plan's own items as present even though they are extended after the text is read. A pack list declared with no pack is refused for the reason a unit without one is: `fkrecipes: the packs setting chain-packs declares no science pack; research takes at least one`. Both have `Legacy` forms, `LegacyIngredientsSetting` and `LegacyPacksSetting`, for a text setting a mod already ships under a name of its own.

`NumericSpec` bounds an int or double setting and both bounds are optional: a zero value in Go, `None` in Rust, means unbounded rather than pinned to zero. `Between` and `NumericSpec::between` are shorthand for the common case of setting both.

Settings are emitted in declaration order. A generated setting's `order` string is two letters from its declaration index (`aa`, `ab`, ... `az`, `ba`, ...), so a plan of generated settings alone shows in the settings screen in the order you wrote it. A plan that mixes legacy and generated settings does not: the screen sorts by the order strings themselves, a legacy order is whatever you passed, and a generated one lands among the legacy orders by the alphabet rather than by where you declared it (the two letters count declaration slots, legacy declarations included: beside legacy orders `a` and `b`, a generated setting in any of the first twenty-six slots sorts between them and the twenty-seventh declaration carries `ba` and sorts after `b`). `OrderAfter` (`order_after`) is how a mixed plan places its generated settings: every generated setting declared after the call carries the given order followed by its two letters, so it sorts after the legacy setting with that order and before every legacy order that sorts after that one, and it keeps its generated name. Call it again to move on. Orders that sort before the named one are unaffected, so placing settings after `b` says nothing about where they stand relative to `a`.

```go
recipeCost := lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla", recipeValues, "a")
lib.OrderAfter("a")
parts := lib.IngredientsSetting("recipe-ingredients", vanilla) // order aab: after a, before b
techCost := lib.LegacyDropdownSettingNeedingLocale("bbb-tech-cost", "logistics", techValues, "b")
lib.OrderAfter("b")
packs := lib.PacksSetting("tech-packs", defaultPacks)          // bad
count := lib.IntSetting("tech-count", 20, fkrecipes.Between(1, 1000000)) // bae
seconds := lib.IntSetting("tech-seconds", 15, fkrecipes.Between(1, 3600)) // baf
```

```rust
let recipe_cost = lib.legacy_dropdown_setting_needing_locale("bbb-recipe-cost", "vanilla", &recipe_values, "a");
lib.order_after("a");
let parts = lib.ingredients_setting("recipe-ingredients", vanilla); // order aab: after a, before b
let tech_cost = lib.legacy_dropdown_setting_needing_locale("bbb-tech-cost", "logistics", &tech_values, "b");
lib.order_after("b");
let packs = lib.packs_setting("tech-packs", default_packs);          // bad
let count = lib.int_setting("tech-count", 20, NumericSpec::between(1.0, 1000000.0)); // bae
let seconds = lib.int_setting("tech-seconds", 15, NumericSpec::between(1.0, 3600.0)); // baf
```

Three things are refused at the settings stage: an empty order; a generated setting whose order string equals a legacy setting's, whether or not `OrderAfter` was called (a plan that tied a generated `aa` with a legacy `aa` by accident loaded before with the two in the engine's hands, and is refused now); and a placed setting that would sort past a legacy order extending the one it was placed after, which is the misplacement `OrderAfter` exists to remove (with legacy `a` and `ab`, twenty-four generated settings fit under `a` before the twenty-fifth reaches `aba`; with legacy `b` and `ba`, nothing fits under `b`, so name `ba`):

```
fkrecipes: OrderAfter was given an empty order; name the order string the generated settings should follow
fkrecipes: the setting tech-packs would carry the order bad, which the legacy setting bbb-tech-detail already carries; give one of them an order of its own
fkrecipes: the setting tech-packs would carry the order bad and sort past the legacy setting bbb-tech-detail at ba, which extends b; OrderAfter(b) places settings before every legacy order that extends b
```

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

Two ladders can land on the same name, and what the library emits never carries it twice. The amounts are added, in the position of the first occurrence, so the order you declared survives, and a line records it. An item line names both amounts; a fluid line names the fallback that landed on the name instead, so that two lines about one fluid tell you which declaration each came from:

```
fkrecipes: balancer-part: iron-plate is in the list twice after the fallbacks, so the amounts are added: 4 plus 2 is 6
fkrecipes: sulfuric-mix: water is in the list twice after the fallbacks, so the amounts are added; the ladder from steam resolved onto it
```

This is what makes a staple at the end of every ladder safe. The engine refuses the entire load for a recipe that names one ingredient twice, with `Duplicate item ingredients are not allowed (iron-plate exists 2 or more times)` and no mention of a setting or of the missing item, and the player it hits is the one who never opened the settings, because the mod's own default is what resolved that way. An item and a fluid of the same name are two different ingredients and are not added together.

Adding up applies only to a list a ladder collapsed. A list you declared that names the same item, or the same fluid, twice is a mistake in the mod rather than a missing ingredient, and it is refused at plan time in every place one can be written: a plain `Ingredients` list, a dropdown preset, a hand-rolled `Unit`'s science packs and a `CostBy` fallback's:

```
fkrecipes: the recipe balancer-part names iron-plate twice; each ingredient is taken once
fkrecipes: the technology steel-axes names automation-science-pack twice; each science pack is taken once
```

A text setting's declared default is refused the same way, in the words the setting's own language uses (`fkrecipes: the ingredients setting axe-ingredients: entries 1 and 2 both name steel-plate`), because that list has to survive being written into the description and read back.

An added amount is held to the engine's ceiling, which is 65535 for an item and 1e301 for a fluid. Two declarations that are legal apart can add up to one that is not, and the total is capped at the ceiling with a line saying so, because which rungs the ladders landed on is a fact about the player's mod set rather than about your declaration:

```
fkrecipes: balancer-part: iron-plate is in the list twice after the fallbacks, and 40000 plus 30000 is above the item ceiling of 65535, so it is capped there
fkrecipes: sulfuric-mix: water is in the list twice after the fallbacks, and the added amount is above the fluid ceiling of 1e301, so it is capped there; the ladder from steam resolved onto it
```

A capped amount is arithmetic the player cannot check, so the recipe's own `localised_description` gains a trailing line saying it happened, and for a recipe that line carries what changing a recipe costs a player who has already built with it:

```
Two ingredients resolved onto iron-plate and the total was above what one slot holds, so it was capped at 65535. The reason is in the log. Changing a recipe empties an assembling machine's input slots of anything the new list does not use.
```

A DECLARED amount above the ceiling is a different thing and is still refused: `Ingredients`, a dropdown preset, a hand-rolled `Unit` and a `CostBy` fallback all read only your declaration, and an author who writes 70000 of something has written something the engine will not take.

A resolved list may name the very item the recipe makes, and that is accepted rather than refused, because the engine itself ships recipes that consume what they make. Base has exactly two (measured on Factorio 2.0.77, base alone): `kovarex-enrichment-process`, which takes 40 `uranium-235` and gives back 41 and is the item shape, and `coal-liquefaction`, which takes 25 `heavy-oil` and gives back 90 and is the fluid shape. What this library emits is always an item, so only the item shape can arise in a plan of yours, and only that shape gets the line. The recipe is emitted as it resolved and a line records it, because a recipe that consumes its own product cannot make the first one:

```
fkrecipes: balancer-part: bbb-balancer-part is in the list and is also what this recipe makes, so nothing can craft the first one unless something else produces it
```

The check sits where every resolved list is handed over, so all four ways of naming one reach it: `Ingredients`, a dropdown preset, a text that takes a dropdown's choice over and a text with no dropdown beside it. Three shapes actually produce the line. An `IngredientOf` (`Ingredient::of`) handle to the recipe's own result item is one. A list the player typed is the second, because their text is read against a world that already knows the items this plan is about to emit. The third is a ladder landing on the existing item a `ResultNamed` recipe makes, which is somebody else's item rather than one of yours. A declared ladder cannot land on an item of your own: a ladder's rungs are probed against the game as loaded and your items are not in it yet, so such a rung is simply absent and the ingredient is dropped with the ordinary drop line. The line names the recipe as you declared it and the ingredient under the name the game will hold. An ingredient that is a fluid of the product's name is a different ingredient and says nothing, because a product is always an item and the two namespaces are separate.

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

The text starts out as the word `default`, which means the list you declared with its ladders, exactly as if you had written `Ingredients`; where you pair the text with `IngredientsBy` it means the preset the dropdown is currently on instead. Either way it keeps meaning that when you change the list in a later release. An edited text is taken as written: every name must exist in the game as loaded, and nothing is substituted. One text setting serves one recipe; a mod with several customizable recipes declares one setting per recipe. When the text applies, one line records what was read:

```
fkrecipes: steelworks-steel-rivet takes its ingredients from steelworks-rivet-ingredients: 3 steel-plate, 2 iron-stick
```

**A text the language cannot read is never the reason a load stops.** Your recipe comes out exactly as though the field said `default`: the list you declared, ladders and all, where the text stands alone, and the preset the dropdown is currently on where you paired the text with `IngredientsBy`. One line names the setting, quotes the problem in the words the reference gives, and tells the player where to fix it:

```
fkrecipes: ERROR: steelworks-rivet-ingredients, entry 1 ("2 iron-plat"): no item or fluid is named iron-plat. The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.
```

That is a rule about every field the player controls: a typed list, a stored value that is not text, and the numeric fields of a research cost (whose line ends `fix the number` instead). It is the narrow claim and not "a player is never refused": a fallback lands where the field's reserved word would have landed, which is the preset the dropdown beside it is currently on where you declared one and your declared list where you did not, and whatever it lands on is held to the rules it always was, so a declaration that cannot produce a legal result in this particular game can still stop the load. The next paragraph enumerates what is left of that, and a science pack is no longer on the list. Any such refusal is one a player who typed nothing meets too, and it names what the mod must change rather than a screen: the `Error loading mods` dialog cannot reach the Mod Settings screen at all. When a stored value was set aside on the way to it, the refusal carries one more sentence, `. The stored value of <setting> could not be used and was set aside, so what applied is what that field gives when it is left alone.`, naming the first such setting in the order the library reads them. It is there because no log line reaches the game on a failed load, and it is a fact rather than advice for the same reason the rest of the message is: there is nowhere to send the player. Measured behaviour of the client is what makes the fallback a rule rather than a preference. Factorio rewrites `mod-settings.dat` on every successful load and on no failed one, so a value that fails the load is a value nothing in the game will then edit; the dialog offers Disable listed mods, Disable all mods, Manage mods, Restart, Exit and a Reset mod settings checkbox, `Manage mods` reaches only the Mods screen, which has no Mod settings button and whose Back returns to the same dialog, and `Restart` comes back to an identical dialog over a file that has not moved. Disabling and re-enabling the mod does not help, because the engine keeps a disabled mod's settings and does not show them on the Mod Settings screen. The one way out is `Reset mod settings` with `Disable listed mods`, which costs every startup preference in the file. Measured on Factorio 2.0.77 (build 84539, mac-arm64).

**What the player's mod set does is a separate rule, and it degrades rather than refusing.** A check that asks the game a question about what is installed is one this library answers by using the nearest thing that works and saying so; a check that reads only your declaration refuses by name. So a science pack removed from a copied research cost is dropped, a cost whose packs are all gone falls back to the one you declared, a research left with no usable science pack anywhere is emitted with none at all, a copied cost in a form this library cannot read falls back or empties, a prerequisite that would close a loop through your own plan is dropped, and two ladder rungs collapsing above an amount ceiling are capped at it. What stays a refusal is enumerated rather than open-ended, and each entry carries the reason it cannot degrade: a name your plan would overwrite in `data.raw`, because degrading clobbers a stranger's prototype; a `CostOf` source that is absent or carries nothing this library can copy, and a `PlaceResult` or a `ResultNamed` naming a prototype the game does not have, because a bare name has no declared ladder behind it and degrading would mean inventing a value you never wrote, into your own prototype; a prerequisite cycle holding no edge your plan made, because that is the game's own tree looping without your mod in it and the only alternative to the message is rewriting somebody else's tree; a host that hands the library no `World` or no mod name; and every check that reads only your declaration.

**And the recipe or technology says so where the player looks.** The log is evidence for you; a player reads the settings screen, the changelog and the tooltip of the thing in front of them. So a prototype whose stored setting value was set aside carries one trailing line in its own `localised_description`, joined onto the description you declared or standing alone when you declared none:

```
The stored value of steelworks-rivet-ingredients could not be used, so the game loaded as though that setting had been left alone. The reason is in the log.
```

A fallback on a recipe's **ingredient text** carries one sentence more, because fixing that setting costs the player something the engine will not give back: `Changing a recipe empties an assembling machine's input slots of anything the new list does not use.` The same sentence ends the `ERROR:` line of a recipe's ingredient text, and no other note and no other fallback line carries it. A crafting time, a pack text and the two research numbers do not: a repriced research destroys nothing, and a recipe whose crafting time fell back emits the same ingredient list it always did and moves only its `energy_required`. One line per prototype, naming the first setting the walk set aside, and a crafting-time setting two recipes read puts it on both of them. A prototype nothing fell back on carries exactly what it carried before, byte for byte.

The line above is ONE SENTENCE TO A PLAYER AND MAY BE SEVERAL ELEMENTS in the prototype. A localised string element may hold 200 bytes and the engine refuses the whole load over it, naming the element, and the sentences here run past that as soon as a setting name is in them. So every literal this library writes into a `localised_name` or a `localised_description` is filled to at most 180 bytes and broken at a space, and the engine joins the pieces back together with nothing between them. Your own `Description` and `DisplayName` go the same way, so neither has a length you have to keep under. Nothing about the text a player reads changes; what changes is a test that compares a whole `localised_description` value rather than the sentence it renders.

A degradation the player's mod set caused writes its own trailing line in the same place and in the same voice, and it names no setting, because nothing was stored and there is nothing for anybody to go and fix:

```
This game has no military-science-pack, so this research was priced without it. The reason is in the log.
This game has none of the science packs the steel-processing cost names, so this mod's own declared cost applies. The reason is in the log.
Two ingredients resolved onto iron-plate and the total was above what one slot holds, so it was capped at 65535. The reason is in the log.
```

A resolve-or-drop ingredient ladder gets no such line, on purpose: it is what the ladder is for, and a dropdown's own composed description already tells the player that the nearest thing their mods do have is used instead. A dropped science pack and a capped amount are presence and arithmetic a player cannot check anywhere.

The line is English for every player, and that is deliberate rather than an omission: a locale key the game does not define deletes a prototype's whole description on the client, silently and with the load still exiting 0, so composing one would risk the sentence it was meant to carry. Every sentence this library composes onto a recipe or a technology is an English literal for the same reason.

**`RecipeSpec.Description` and `TechSpec.Description` are literal text, not a locale key.** Whatever you put there is emitted as the string itself, joined with the library's own trailing lines. If you write a locale key into one, the library cannot wrap it: the alternatives form it uses for the keys it composes itself is only available for keys it composed, and an undefined key anywhere in a recipe's composition drops the whole description silently, the literal sentences beside it included (measured on Factorio 2.0.77, build 84539). Ship the text, or ship the locale entry and be sure of it.

Anything **you** declare still refuses at plan time, and should: your own default list, your presets, your ladders and your setting bounds are bugs to catch while you are building the mod, not choices a player made.

A dropdown of presets can offer the same thing without growing a value. Declare `IngredientsFrom` beside `IngredientsBy` on the same recipe: the dropdown keeps exactly the option list you wrote, and the text setting decides which of the two is live.

```go
IngredientsBy: &fkrecipes.IngredientChoices{
	Setting: medium, // its values are water and oil
	Choices: []fkrecipes.IngredientChoice{
		{Value: "water", Ingredients: []fkrecipes.Ingredient{fkrecipes.IngredientNamed(2, "steel-plate"), fkrecipes.FluidIngredient(10, "water")}},
		{Value: "oil", Ingredients: []fkrecipes.Ingredient{fkrecipes.IngredientNamed(2, "steel-plate"), fkrecipes.FluidIngredient(5, "lubricant")}},
	},
},
IngredientsFrom: quench,
```

```rust
ingredients_by: Some(IngredientChoices {
    setting: medium,
    choices: vec![
        IngredientChoice { value: "water".into(), ingredients: vec![Ingredient::named(2, "steel-plate", &[]), Ingredient::fluid(10.0, "water", &[])] },
        IngredientChoice { value: "oil".into(), ingredients: vec![Ingredient::named(2, "steel-plate", &[]), Ingredient::fluid(5.0, "lubricant", &[])] },
    ],
}),
ingredients_from: Some(quench),
```

**The text is the switch.** While it says `default` the dropdown decides, exactly as it did before the text setting existed; anything else is the whole list, and the line that records it says which choice was set aside:

```
fkrecipes: steelworks-hardened-steel-plate-quenching takes its ingredients from steelworks-quench-ingredients: 2 steel-plate, 6 iron-stick; the steelworks-quench-medium choice oil is set aside
```

A text the language refuses behaves exactly as `default` does, so the dropdown decides and the ERROR line above is the only difference. Nothing is ever edited and ignored: every value that is not at its default is live.

The dropdown's description is composed for you: your own `[mod-setting-description]` entry, then two lines per preset, its localised label and then, on a second indented line opening with `type:`, its ingredients written out in the language, so a player about to type can start from the preset they were on; then a line saying that a list too long for one line continues on the next and the continuation is part of the same list; and last a line saying that the text setting above or below it applies while it does not say `default` (a cost dropdown's preset stays on one line, names the technology it copies through that technology's own localised name, and carries no wrap line because it renders no list to copy; see below). The text setting's own description carries the mirror of that sentence, naming the option above or below it. Which word each of them uses is decided by the emitted `order` strings, so the sentence is true whichever order you declared them in; the settings screen has no conditional visibility at all (measured on Factorio 2.0.77, build 84539), so those two lines are the only place the pairing can be stated.

Every locale key the library composes into a description, yours and the game's alike, goes out in the engine's alternatives form with a raw fallback last, so a key nothing defines degrades to legible text rather than costing the whole tooltip. A `[mod-setting-description]` entry falls back to the emitted setting name, a `[string-mod-setting]` entry to the dropdown value itself, and a `technology-name` entry to the technology's internal name. That is a safety net and not a substitute: the checker still requires the two entries inside your prefix, and the fallback is what a player sees in a modpack you did not anticipate.

The ingredient line is a line of its own because it is written in a different vocabulary from the label above it. Your label is display prose and the line under it is the internal names the text field takes (`4 iron-plate, 2 iron-gear-wheel`); only the second can be pasted into the field. The client truncates a closed dropdown's label at about 37 characters (measured on Factorio 2.0.77, build 84539), so joining the two with a colon would put the half that works past the truncation with nothing to say which half was which.

**Give a dropdown option a short name, not a sentence.** Write `Express belts`, not `Express belts: the vanilla recipe with express transport belts`. The library composes the preset's typeable list beside your label, and the tooltip wraps near 57 to 60 characters (measured on Factorio 2.0.77, build 84539), so every character of the label is a character the list has to give back. A wrapped line is not lost: the engine continues it on the next line and the description says so. But the continuation starts at the left margin rather than under the line it belongs to, so it reads as a line of its own, and a player who copies what looks like a whole line loses the end of the list. A short name keeps both halves on one line each.

A dropdown that takes a text setting from two recipes is refused, because its composed description can only describe one. The word `custom` is an ordinary dropdown value: this library reserves none and adds none, so a dropdown of yours that already offers a preset spelled `custom` keeps it and keeps every stored choice with it.

A stored dropdown value that is none of the values you offer cannot come through the settings screen, which resets it, but a file edited by hand can carry one; it is refused by name rather than read as any preset.

### Crafting time

`CraftTime` is a fixed number of seconds. Zero means "say nothing", and the engine applies its own default.

`CraftTimeFrom` binds the recipe to a double setting you declared, and the player chooses the seconds. The two are mutually exclusive and naming both is refused.

The engine will not load a recipe whose `energy_required` is at or below 0.001 (measured on Factorio 2.0.77, build 84539). The library holds that floor in three places: a declared `CraftTime` at or below it is refused, a double setting bound as a crafting time is given a `minimum_value` of 0.002 unless you declared a minimum of your own, so the settings screen cannot produce a value that kills the load, and a declared minimum at or below the floor is refused too. Should the setting answer below the floor anyway, which takes another mod declaring a startup setting of the same name and type, the declared default applies and the log carries the fallback line described above under the text settings, ending `fix the number`.

A double setting is the only setting type that can hold a value that is not a number. Measured on Factorio 2.0.77, build 84539: the engine's own range check turns a stored NaN down for an `int-setting` and accepts it for a `double-setting`, so a NaN reaches the mod. This library declares no double setting for anything it prices: a research count and its seconds per unit are int settings, and the only double setting in the library's own vocabulary is the one you bind to a crafting time. A double setting you declare is yours, and a value that is not a number stored under it is a value this library cannot make safe on your behalf. Prefer an int setting wherever the quantity is whole.

If the bound setting cannot be read, the declared default applies and a line goes to the log:

```
fkrecipes: the setting steelworks-forging-time was not readable, so its default applies
```

A recipe some technology unlocks is emitted with `enabled = false`, because the research is what turns it on. A recipe nothing unlocks is emitted enabled.

## Technologies

A technology needs exactly one source of cost and at most one anchor in the tree.

### Cost

`CostOf` names an existing technology and copies its whole `unit` unchanged, except for its science packs. This is the intended way to price research: cost and tree position then come from one named point, and a multi-level source brings its `count_formula` and its `max_level` across without this library needing to evaluate either.

The packs are filtered because a copied unit is somebody else's declaration and the engine is strict about what a research can be priced in. A pack the player's mod set removed, or demoted from a `tool` to a plain `item`, is left out of the copy with a line, and the technology's own tooltip says so:

```
fkrecipes: hardened-tips: military-science-pack is not a science pack this game has, so it is left out of the military-4 cost
```

Without that filter the load stops on the mod's default setting with `Invalid research unit (military-science-pack). Research unit(s) can only be tool type items at the moment.`, or, for a name the game does not have at all, with `Error in assignID: item with name 'water' does not exist.` (both measured on Factorio 2.0.77). Neither names your mod, and the second names neither the technology nor the property. A copied pack list written in a form this library cannot read is never passed through, because passing a name it never read to the engine earns the second of those two refusals. What happens instead depends on whether there is anything declared behind the source. Under `CostOf` there is not, so the unit is kept with its `ingredients` REPLACED BY AN EMPTY LIST: the count, the time, a `count_formula` and every field this library has never heard of survive untouched and nothing is invented.

```
fkrecipes: ERROR: hardened-tips: the unit of military-4 holds a table this library cannot copy faithfully, so the research is emitted with no science pack and completes for free
```

```
The military-4 cost this research copies cannot be read in this game, so it takes no science pack at all. The reason is in the log.
```

The note says the list could not be READ and not that the game has none of those packs, which would be a claim about names this library never decoded.

**Pointing a cost at an infinite technology mis-prices a one-level one, and nothing warns you.** A `count_formula` is written in terms of the level `L` and is copied verbatim, so a formula written for a source that starts at level 7 evaluates at level 1 on a technology of yours that has no `max_level`: measured on Factorio 2.0.77, `count_formula = "2^(L-7)*1000"` copied onto a one-level technology reads `research_unit_count = 15`. The load succeeds, the engine logs nothing, and the same hazard reaches a `CostBy` tier whose ladder lands on such a source. Read the source's own `unit` before you name it, and prefer a source whose cost does not depend on a level your technology does not have.

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

`CostBy` is `CostOf` with a ladder per dropdown value: the player picks a tier, and the ladder is walked to the first technology that exists and carries a unit. That unit is copied with its `max_level`, its packs filtered as above, and the source becomes the technology's sole prerequisite, so it does not combine with any of the placement fields. A `Fallback` unit applies when no rung works out. See [Migrating a mod that already ships settings](migration.md).

When the chosen source's packs are all unusable, your `Fallback` unit is what the technology is priced in instead, with its own ladders walked, and the prerequisite and the level cap stay where the tier put them:

```
fkrecipes: ERROR: steel-axes: the steel-processing cost names no science pack this game has, so this mod's own declared cost applies instead
```

That line is not about anything the player typed and points at no setting; the technology's tooltip carries the same fact. The same pair is written where the chosen source's pack list is in a form this library cannot read, with `holds a table this library cannot copy faithfully` in place of `names no science pack this game has`. `CostOf` has no `Fallback` behind it, so a copy there that keeps no pack is emitted with an empty unit instead, as below. If the `Fallback` loses every pack as well, the line below names both sets in the order they were asked and each pack once: the packs the copied unit lost first, then the rungs the `Fallback`'s own ladders tried. If the technology also declares `CostFrom` and the player has typed a pack list into it, neither the line nor the tooltip sentence is written at all, because the cost that applies is then theirs and not the one you declared.

`Unit` is the escape hatch when no existing technology has the price you want. It takes a count, a time in seconds and a list of science packs. A pack is a presence ladder like an ingredient: `Pack{Name: "automation-science-pack", Amount: 1}` is a one-rung ladder, `Fallbacks` adds rungs, and a pack whose rungs are all absent is dropped with a log line. A unit whose packs all drop is EMITTED WITH NO SCIENCE PACK, naming every rung it tried, and the technology says so in its own tooltip. A unit DECLARED with no pack at all is still refused, because that check reads only your declaration.

```
fkrecipes: ERROR: steel-axes: none of military-science-pack, space-science-pack is a science pack this game has, so the research is emitted with no science pack and completes for free
```

```
This game has none of the science packs this research names, so it takes no science pack at all. The reason is in the log.
```

The line and the note are per technology, so two technologies that lose their packs get two of each. The word `free` in them is literal and measured rather than a figure of speech: a research unit with an empty ingredient list loads with no complaint from the engine, `add_research` accepts it, the queue takes it, and it completes after `count * time` ticks in a lab holding nothing at all, consuming nothing (measured in play on Factorio 2.0.77, build 84539). A packless research is not a stuck technology; it is a free one, which is a balance change nobody chose, so it is disclosed where the player looks. It used to stop the load instead, and that was worse: the engine's own error dialog cannot reach the Mod Settings screen, so a modpack the player did not assemble locked them out of a game the library could have loaded. Where there is an author-declared cost to fall back on the library still prefers it, and only where there is none does the unit come out empty.

Two pack ladders that land on the same pack add their amounts exactly as two ingredient ladders do, with the technology as the subject of the line.

A science pack amount goes up to 65535, declared or added, and that is the engine's own limit rather than this library's caution. Measured on Factorio 2.0.77 (build 84539, mac-arm64), on a technology whose `unit.ingredients` carries one pack: 65535 loads and is dumped as written, while 65536, 2147483648 and 9007199254740992 each fail the load with `Value (<n>) outside of range. The data type allows values from 0 to 65535 in property tree at ROOT.technology.<name>.unit.ingredients[0][1]`, exit code 1 and no dump. It is the same 16 bits an item ingredient's amount is held in, so this library refuses such a pack by name rather than letting the engine blame your mod for it.

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

Before anything is emitted, the library walks the tree your plan is about to produce: every existing technology's prerequisites, with your splices applied and your own technologies added. Where a ring closes through an edge your plan made, that edge is dropped and the walk runs again; where it closes without one, the load stops with the whole ring named in order.

An edge your plan made is one of exactly two things, and each has its own line and its own tooltip sentence. A prerequisite of one of your own technologies:

```
fkrecipes: ERROR: steel-axes: requiring logistics-3 would loop this game's technology tree (logistics-2 -> steel-processing -> steelworks-steel-axes -> logistics-3 -> logistics-2), so the prerequisite is dropped
```

```
Requiring logistics-3 would loop this game's technology tree, so this research was left without that prerequisite. The reason is in the log.
```

Or a splice your plan inserted into another technology's prerequisite list, through `Before`:

```
fkrecipes: ERROR: steel-axes: making it a prerequisite of logistics-3 would loop this game's technology tree (logistics-2 -> steel-processing -> steelworks-steel-axes -> logistics-3 -> logistics-2), so the splice is dropped
```

```
Making this research a prerequisite of logistics-3 would loop this game's technology tree, so it was left out of it. The reason is in the log.
```

A dropped splice gives back the prerequisite it replaced, to every later splice built on top of it, so undoing yours never deletes another mod's own edge. The whole ring is in the log line and not in the tooltip, because a player hovering a technology cannot act on a list of prototype names.

What is left as a refusal is a ring your plan holds no edge in. That is the game's own technology tree looping without your mod in it, which the engine refuses on its own, and there is nothing of yours to take back:

```
fkrecipes: a prerequisite cycle: logistics-2 -> steel-processing -> logistics-3 -> logistics-2
```

### A research cost the player writes

`CostFrom` is `Unit` with its three numbers in the player's hands: a `PacksSetting` for the science packs, written as an ingredient list, and an int setting each for the count and the seconds. On its own, each number setting must declare a minimum of at least 1 and a maximum, because the engine refuses a unit with a count of 0 or a time of 0 (measured on Factorio 2.0.77) and a field the player types into needs a ceiling the screen can show; a `CustomCost` whose settings do not is refused at plan time, and the engine's own rule that an out-of-range stored value resets to the default keeps every value the library reads legal. Should one of the two answer with something the engine would not take anyway, it takes the setting's declared default and logs the fallback line ending `fix the number`, exactly as a refused text takes yours. Placement is the ordinary placement fields, as for `Unit`.

```go
count := lib.IntSetting("chain-count", 20, fkrecipes.Between(1, 100000))
seconds := lib.IntSetting("chain-seconds", 10, fkrecipes.Between(1, 600))
lib.Technology("chain-forging", fkrecipes.TechSpec{
	CostFrom: &fkrecipes.CustomCost{Packs: packs, Count: count, Seconds: seconds},
	After:    "steel-processing",
	Unlocks:  []fkrecipes.RecipeRef{chain},
})
```

```rust
let count = lib.int_setting("chain-count", 20, NumericSpec::between(1.0, 100000.0));
let seconds = lib.int_setting("chain-seconds", 10, NumericSpec::between(1.0, 600.0));
lib.technology("chain-forging", TechSpec {
    cost_from: Some(CustomCost { packs, count, seconds }),
    after: "steel-processing".into(),
    unlocks: vec![chain],
    ..Default::default()
});
```

Only items the game treats as science packs (prototype type `tool`) are accepted in the pack list, and the word `default` means the packs you declared, with their fallbacks. The unit is emitted in the engine's short tuple form, and one line records what was read:

```
fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 25, time 12, packs 1 automation-science-pack, 1 logistic-science-pack
```

Where the resolved list comes out empty, because every pack in it dropped, that field reads `packs no science pack` rather than naming the language's reserved word for an empty list: a pack field turns that word down, so printing it would be inviting the player to paste back the one text it will not take.

`CostFrom` also combines with `CostBy`, and that is the customizable research cost: the dropdown chooses a tier and the three settings overwrite that tier's numbers **one field at a time**. A field left at its declared default comes from the tier, so a player who moves the count alone gets their count with the tier's time and the tier's packs, and a player who touches nothing gets the tier byte for byte. Beside a dropdown, 0 is what a number says instead of the reserved word, so each of the two number settings declares a default of 0, a minimum of 0 and a maximum; with no dropdown there is nothing to defer to and the minimum of at least 1 applies. The tier still places the technology: the source whose cost it names is the prerequisite, whatever the settings say.

```go
CostBy: &fkrecipes.CostChoices{
	Setting:  tier, // its values are projectile and military
	Choices:  tiers,
	Fallback: fallback,
},
CostFrom: &fkrecipes.CustomCost{Packs: tipsPacks, Count: tipsCount, Seconds: tipsSeconds},
```

```rust
cost_by: Some(CostChoices { setting: tier, choices: tiers, fallback }),
cost_from: Some(CustomCost { packs: tips_packs, count: tips_count, seconds: tips_seconds }),
```

When any of the three is not at its default, one line records what the cost came out as and says what the tier still supplied:

```
fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 45, time 30, packs 1 automation-science-pack, 1 logistic-science-pack, 1 military-science-pack; the steelworks-tips-research-tier choice military supplies what the settings leave at default
```

The tier's unit is taken whole and written over field by field, so a `count_formula`, a `max_level` and any field this library has never heard of come through untouched. One exception: the engine refuses outright a unit carrying both a count and a `count_formula` (`Ambiguous definition: count and count_formula are both defined.`, measured on Factorio 2.0.77), so leaving the formula beside a count the player typed is not an option that exists. The formula is dropped in that case and one line says which setting took it:

```
fkrecipes: hardened-tips: steelworks-tips-count replaces the count_formula the projectile cost carries
```

**What dropping the formula costs, stated because nothing else states it.** An infinite technology priced by a flat count still levels, and every level then costs the same: measured on Factorio 2.0.77, a unit with `count = 500` and no formula under `max_level = "infinite"` reads `research_unit_count = 500` at level 1, level 2 and level 5. The scaling is gone from the prototype and the engine says nothing about it. The library does not refuse, because the trigger is a number a player typed and a value a player types never stops a load; the line above and the trailing line on the technology's own description are where it is disclosed.

When the count setting is left at 0 beside a formula-priced tier nothing is dropped, and the log line says `count by formula` rather than a number, because there is no number in that price:

```
fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count by formula, time 45, packs 1 automation-science-pack
```

A count or seconds setting serves exactly one technology: one read as a research count or time by two of them is refused at plan time (`fkrecipes: the setting tips-count is read as a research count or time by more than one declaration; a custom cost's number serves exactly one`), because the description composed onto it names one dropdown as the thing deciding while the field is 0. Two recipes may still share one crafting-time setting, which is a double and so cannot be a research number's handle at all.

Each of the two number settings is emitted with a description of its own: your `[mod-setting-description]` entry, then the range it takes and, beside a research dropdown, what 0 means. The checker requires that entry for the same reason it requires one on a text setting.

```
A whole number from 0 to 100000. While it is 0 the option chosen above decides.
```

The composed description of a cost dropdown names, after each tier's localised label and on the same line, the technology whose cost that tier copies, through the technology's own localised name (after `: cost of`), or `: the fallback cost` for a tier with no source. It stays on one line where an ingredient preset takes two, because a localised label followed by a localised technology name is one vocabulary and there is nothing in it to copy. Where the game has no such technology or no entry for it, the line degrades to the raw internal name and the rest of the tooltip is unaffected. A last line says that the pack text above or below it applies while it does not say `default`, exactly as an ingredient dropdown's does.

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

A fixture World of your own should embed `fkrecipes.UnimplementedWorld` in Go. In Rust, `World` requires `Named`, so a fixture implements both (an `impl Named` supplying `mod_name` beside the `impl World`; `use fkrecipes::{Named, World};` is what it needs), and the trait's newer questions are default methods. Either way a question this library adds in a later version panics with its own name when your plan first asks it, instead of breaking your build the day you update:

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
- a text setting (an ingredient list or a pack list) needs both a `[mod-setting-name]` entry and a `[mod-setting-description]` entry, because the library composes the declared list, the format, the length limit, the switch line and the fallback under that entry and an absent one loses all of them; a research count or seconds setting needs a description for the same reason, since the range and what 0 means are composed onto it; and a dropdown with a text setting beside it needs one too, since the library composes the preset texts onto it.

Sample output over a file missing one name and one dropdown value, and carrying two leftovers:

```
the dropdown setting steelworks-quench-medium has no [string-mod-setting] entry for its value oil
the setting steelworks-bonus-research has no [mod-setting-name] entry
the [mod-setting-name] entry steelworks-scrap-recovery matches no setting this plan declares
the [string-mod-setting] entry steelworks-quench-medium-brine matches no dropdown value this plan declares
```

### Advisories, which are not findings

A key **inside your mod's prefix** is required, and everything above is about those. A key **outside it** is never required, and the difference is the locale namespace itself: Factorio's is flat and shared, so a `[technology-name]` entry in your `.cfg` sets the displayed name of that technology for every mod in the game. The library will not ask you to do that, and the collision scan above exists to catch the same hazard.

The one key the library composes out of your prefix is `technology-name.<source>`, in a cost dropdown's preset lines. `CheckLocaleAdvisories` (`check_locale_advisories`) is where the library says so, once per such key, in composition order:

```go
for _, note := range plan().CheckLocaleAdvisories("steelworks") {
	t.Log(note)
}
```

```rust
for note in plan().check_locale_advisories("steelworks") {
    println!("{}", note);
}
```

```
note: the dropdown setting steelworks-tips-research-tier composes the game's own key technology-name.military-4, which this plan does not own; where the game does not define it the tooltip shows military-4 instead, and defining it here would rename it for every mod
```

**This is information, not a checklist, and a test suite must not fail on it.** `CheckLocale` and `CheckLocaleWith` never return an advisory: an empty report from either still means a clean file, which is what your suite should assert. An advisory is not asking for anything either, because there is nothing to do: where the game does not define the key the composed line degrades to the raw internal name, which is legible, and the tooltip survives whole. Defining the key would be the one action that makes things worse.

It takes no `.cfg`, and that is the shape of the fact rather than a convenience: an advisory is about your plan and the game's namespace, so a file that defines the key produces exactly the same sentence. It has a cap of its own, with the same closing line the findings cap uses, so neither report can crowd out the other.

### The mod name, and what the checker cannot see

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
