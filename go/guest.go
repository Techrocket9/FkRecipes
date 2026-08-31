//go:build tinygo.wasm

// The guest-only half: everything that touches a host import lives behind
// this build gate, which is what keeps the pure half testable with plain
// `go test` on the host (//go:wasmimport is rejected off-target).
//
// To call the Factorio API from here, import the consumer's generated
// bindings by their canonical path -- and read the contract in the package
// doc first: that import makes this library PIN-COUPLED, and it must never
// ship its own copy of fkapi.
package fkrecipes

import "github.com/Techrocket9/fklua/guest/go/fkdata"

// Announce is an example of guest-only glue over the pure state.
func (l *Lib) Announce() {
	fkdata.Log("fkrecipes: tick counter is running")
}
