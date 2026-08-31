# FkRecipes

FkRecipes ("Factorio: konfigurierbare Recipes", joining the FkLua naming pattern) is a guest library for Factorio mods built with [FkLua](https://github.com/Techrocket9/FkLua): your mod declares crafting recipes, items and technologies in Go or Rust, and each recipe or technology can be driven by startup settings the library generates for you, so players configure your content from the mod settings screen. The library validates what you declared (presence of every name, prerequisite cycles, unit shape) before anything reaches the game, because the engine's own diagnostics for these failures are poor.

It is data-stage only and depends on fkdata alone, in both languages, which makes it pin-transparent: no FkLua API pin move can break it.

The library is under construction; this README grows with it.

## Requirements

- Go 1.24 or Rust 2021, matching the half you consume
- An FkLua toolchain for packaging your mod (`fklua mod`), per the FkLua documentation

## Repository layout

- `go/` is the Go half, module `github.com/Techrocket9/fkrecipes/go`
- `rust/` is the Rust half, crate `fkrecipes`
- `go/examples/datastage` and `rust/examples/datastage` are one small mod written twice, a steelworks expansion declaring the same settings, items, recipes and technologies in both languages; they are the arms of a mirror test that packages both and compares every observable effect byte for byte

## Licence

MIT, see [LICENSE](LICENSE).
