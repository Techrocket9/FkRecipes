# The ingredient list

An ingredient list is the text a player types into a startup setting to say what a recipe is made of, or which science packs a research takes. This page is the reference for that text: what to type, how it is read, and what every refusal means. Mod authors declare the setting as described at the end of this page and in [Using FkRecipes](usage.md); players see it in the Mod Settings screen under the Startup tab.

## What to type

Write the amount, then the name, and separate ingredients with commas:

```
2 iron-plate, 3 copper-cable
```

| Text | Meaning |
|---|---|
| `2 iron-plate, 3 copper-cable` | two iron plates and three copper cables |
| `iron-plate` | one iron plate; a missing amount means 1 |
| `2x iron-plate` or `iron-plate x2` or `iron-plate*2` | the same as `2 iron-plate`; an `x`, `*` or `×` between the two is allowed |
| `iron-plate 2` | also the same; either order works |
| `0.5 water` | half a unit of the fluid water, in a recipe whose category allows fluids |
| `[item=iron-plate] 2` | the name in the game's rich-text form, for a name that could be read as an amount |
| `default` | the mod's own list, exactly as the mod would have made the recipe |
| `none` | no ingredients at all: a recipe that is free to craft |

The setting starts out as the word `default`. Leave it there and the mod's own list applies, including any fallbacks the mod declared for a modpack that lacks an ingredient. The setting's description shows that list written out, so you can copy it and change it.

Names are the game's internal prototype names, the ones that appear in the game's data and in rich text, such as `iron-plate`, `advanced-circuit` or `water`. They are not the translated names shown on screen. Names use letters, digits, `-` and `_`, and are case sensitive. The mod's own items count, under the names the setting's description shows for them.

## The rules

- Spaces around the text, around commas and between the amount and the name do not matter. A single trailing comma is allowed.
- An amount is written in plain digits, with a dot for a fraction: `2`, `10`, `0.5`. There is no thousands separator: a thousand is `1000`. Every amount is more than 0. Items take whole amounts up to 65535. Fluids take any positive amount, fractions included, up to 1e301.
- A name is looked up first among the game's items, then among its fluids. A name that is both an item and a fluid means the item; write `[fluid=name]` to mean the fluid, or `[item=name]` to insist on the item.
- A comma inside `[` and `]` does not separate ingredients; a `[` with no closing `]` runs to the end of the text.
- The same ingredient cannot appear twice.
- A fluid is accepted only in a recipe whose category allows fluids. The default category, `crafting`, is the one the player crafts by hand and it takes items only; the game refuses a fluid there, so this library refuses it first with a sentence naming the category.
- The words `default` and `none`, alone, are the mod's list and the empty list. Neither can be combined with other entries.
- A name that could be read as an amount, as an `x`, or as one of those two words is written in its tag: `[item=42]`, `[item=2x4]`.
- The text is at most 2000 characters, and it is plain text: an invisible character pasted in from elsewhere is refused with its code point, so retype rather than paste.

## Science packs

The same text names the science packs of a research cost, with two differences: only items the game treats as science packs (prototype type `tool`) are accepted, and `none` is refused, because a research with no packs is not something this library will emit on a player's behalf.

```
1 automation-science-pack, 1 logistic-science-pack
```

The research count and its seconds per unit are separate numeric settings beside the pack list.

## Leaving the text on default

The word `default` means the mod's own declared list, with every fallback the mod declared, and it keeps meaning that when the mod changes its list in a later release. Anything else is taken as written: every name must exist in the game as loaded, and nothing is substituted. A typo is refused rather than guessed at, and the refusal names the setting, the entry and the problem.

When a recipe also has a dropdown of preset ingredient lists, the text applies only while the dropdown says `custom`; on any other value the preset applies and the text is ignored. The dropdown's description lists each preset written out, so the player can start a custom list from the preset they were using.

## What a refusal means

A refusal stops the game from loading and shows a message. Every message starts with `fkrecipes:` and the setting's full name; those about one entry quote the entry as typed and number it from 1 in the order written. Entries are read in order and the first problem found is the one reported.

| Message | Cause |
|---|---|
| `<setting> contains characters that are not text; retype the list` | The stored value is not valid text. This comes from a file edited by hand, never from the settings screen. |
| `<setting> is longer than 2000 characters; that is not an ingredient list` | The text is too long to be a list. |
| `<setting> is empty; write the ingredients as "2 iron-plate, 3 copper-cable", the word default for the mod's own list, or the word none for a recipe with no ingredients` | The text is empty or only spaces. The settings screen itself restores the default when the field is cleared, so this is seen only with a file edited by hand. A pack list says `write the science packs as "1 automation-science-pack, 1 logistic-science-pack", or the word default for the mod's own list`. |
| `<setting>, entry N is empty; one comma separates two ingredients` | Two commas in a row, or a comma at the start. |
| `<setting>: none stands alone; remove the other entries or the word` | `none` written beside other entries. The same sentence exists for `default`. |
| `<setting>: research takes at least one science pack` | `none` in a pack list. |
| `<setting>: entries N and M both name <name>` | The same item or fluid twice, in either form. |
| `... entry N ("<text>"): "<text>" is a name that reads as an amount; write it in its tag, as [item=<text>]` | The entry is exactly the name of something the game has, but it reads as an amount or an `x`. A fluid is pointed at `[fluid=<text>]`. |
| `... a comma separates two ingredients, not the digits of one number; write a dot for a fraction, as in "0.5 water"` | A comma used as a decimal or thousands separator, such as `0,5 water` or `1,000 iron-plate`. |
| `... has no name; write the amount before the name, as in "2 iron-plate"` | An amount with nothing after it. |
| `... has two amounts` | Two numbers in one entry. |
| `... no item or fluid is named "<words>"; did you mean <name>` | Two or more words that, joined with `-`, name something the game has: `2 iron plates` suggests `iron-plate`. |
| `... no item or fluid is named "<words>"; names are the game's internal names, such as iron-plate, and a comma separates two ingredients` | Two or more words that do not name anything the game has. |
| `... names two ingredients; a comma separates them` | Two names in one entry that both exist, usually a missing comma. |
| `... has more than one "x"` | Two or more `x`, `*` or `×` signs in one entry. The sign quoted is the first one written. |
| `... has "x" with no amount beside it` | An `x`, `*` or `×` with no number in the entry. The sign quoted is the one written. |
| `... "x" goes between the amount and the name` | A sign that is not between the amount and the name, as in `2 iron-plate x`. |
| `... "<character>" has no place here; names use the letters a to z, digits, - and _, and an amount is plain digits, as in "2 iron-plate"` | A character outside the syntax, such as `:`, `=`, `;`, a full-width digit, or a dot outside a number. The first such character is quoted. |
| `... an invisible character (U+00A0) has no place here; retype the entry rather than pasting it` | A control character or an invisible Unicode character, named by its code point. |
| `... "<text>" is not an amount; amounts are plain digits such as 2 or 0.5` | A sign in front of a number (`-2`, `+2`), an exponent (`1e3`), or both (`-1e3`). |
| `... "1.000" is not an amount here; a dot marks a fraction, and a thousand is written 1000` | A number followed by a dot and exactly three zeros, which is a thousands separator in many countries and would be read as 1 here. `0.000` is an amount of zero and gets the zero message instead. |
| `... the amount must be more than 0` | An amount of 0. |
| `... <name> is an item, and items take whole amounts` | A fraction on an item. |
| `... <name> takes at most 65535` | An item amount above the game's limit. |
| `... the amount is too large; fluid amounts go up to 1e301` | A fluid amount above what the game can hold (measured on Factorio 2.0.77: above about 1e301 the game does not refuse, it crashes). |
| `... no item or fluid is named <name>` | An untagged name the game does not have. When lowercasing the name and replacing `_` with `-` produces a name the game has, the message ends `; did you mean <name>`. |
| `... no item is named <name>` or `... no fluid is named <name>` | A tagged name the game does not have, with the same suggestion where one exists in the same kind. |
| `... <name> is a fluid, and a recipe in the crafting category takes items only` | A fluid in a recipe whose category does not allow fluids. |
| `... <name> is a fluid, and research takes science packs only` | A fluid in a pack list, tagged or not. |
| `... <name> is an item, not a science pack` | An item in a pack list that the game does not treat as a science pack. |
| `... no science pack is named <name>` | A pack name the game does not have, with the same suggestion where one exists. |
| `... a tag is [item=name] or [fluid=name]` | A bracketed tag of any other shape, including one that is not closed. |
| `... ingredients carry no quality; write [item=<name>]` | A rich-text tag carrying `,quality=`. Recipes and research costs do not take a quality. |

## Declaring the setting

A mod declares the text setting with `IngredientsSetting` (Go) or `ingredients_setting` (Rust), giving the bare name and the default list, and binds it to a recipe through `IngredientsFrom`, or as the `Custom` arm of a dropdown of presets. A pack list is declared with `PacksSetting` and bound through `CostFrom` or as the `Custom` arm of a research-cost dropdown, together with an int setting for the count and a double setting for the seconds. The setting's default text is the word `default`; the list you declared is written out in the setting's description after your own description text. The name and description come from the mod's locale file under `mod-setting-name` and `mod-setting-description`, and the locale checker treats both as required for a text setting. A short description that tells the player the format is enough:

```
[mod-setting-description]
mymod-parts-ingredients=What the part is made of. Write the amount, then the item name, and separate ingredients with commas: 2 iron-plate, 3 copper-cable
```

See [Using FkRecipes](usage.md) for the constructors and [Migrating a mod that already ships settings](migration.md) for adding a custom arm to a dropdown a mod already ships.
