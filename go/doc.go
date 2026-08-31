// Package fkrecipes is a FkLua guest library: a Go package a mod's guest
// imports, compiled into the CONSUMER's wasm by their own `fklua mod`. It
// declares user-configurable crafting recipes and research, validates them,
// and emits them at the data stage through fkdata.
//
// THE COMPOSITION CONTRACT, in the order it bites:
//
//   - ROUTE, NEVER OWN. Never export a hook (fk_settings, fk_data, fk_alloc,
//     ...): a wasm module has ONE export per name, so an export here takes it
//     away from the consuming mod. The consumer owns the stage exports and
//     calls in.
//   - fkdata AND NOTHING ELSE. No fkapi under any feature, ever. That is what
//     makes this library PIN-TRANSPARENT: no API version can break it, and it
//     sits outside the whole stamp/lock machinery.
//   - UNPREFIXED NAMES ARE UNREPRESENTABLE. The prefix derives from the mod
//     name the World reports; there is no prefix parameter, because a
//     parameter can drift from the packaged mod and the engine keeps the last
//     of two same-named settings SILENTLY.
//   - DETERMINISM IS CORRECTNESS. Plans are slices in declaration order and
//     nothing here iterates a map. A map iteration that decided what to write
//     would be a multiplayer desync with the CONSUMER's name on it.
//
// PLAN, THEN EMIT. Everything in this package is ordinary values: the plan is
// built by the declaration methods on Lib, turned into an Op stream by
// PlanSettings and PlanData, and only the emit layer (guest.go, behind the
// wasm build gate) executes that stream against fkdata. That split is why the
// validators, the ingredient ladders and the cycle walk are testable with
// plain `go test` on a host with no wasm toolchain.
//
// Consumers call Emit. PlanSettings and PlanData are the seam the emit layer
// stands on, exported so that a consumer's own tests can assert on a plan.
package fkrecipes
