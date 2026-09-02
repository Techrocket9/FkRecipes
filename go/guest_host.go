//go:build !tinygo.wasm

// The HOST side of the emit layer: the same three methods, present so that a
// consumer's ordinary toolchain sees a complete package.
//
// WHY THESE EXIST, measured by the pilot. The real Emit is behind
// //go:build tinygo.wasm, because a //go:wasmimport is rejected off-target.
// Without a host counterpart the methods simply do not exist on a host build,
// so a consumer's `go vet ./...` and `go build ./...` fail on their own data
// guest with "Emit undefined" and they have to pass -tags tinygo.wasm to every
// invocation. BetterBeltBalancer's make check carries exactly that workaround
// on one line. A guest is written once and vetted constantly, so the tag
// belongs on the build that produces the wasm, not on every check.
//
// THEY PANIC RATHER THAN RETURN. There is no fkdata on the host: no data.raw
// to read, no prototype to write, and nothing a caller could do with a silent
// no-op except ship a mod that emits nothing. A panic naming the cause is the
// honest answer to a call that cannot be served, and it keeps the host build
// from ever being mistaken for a working guest.
//
// A consumer's own host tests do not come through here. PlanSettings and
// PlanData are pure and exported for exactly that purpose: they take a World
// (or, for settings, a Named) the test supplies, and assert on the Op stream.

package fkrecipes

// hostOnly is the one message, so all three read identically wherever a
// consumer meets one.
const hostOnly = "fkrecipes: Emit runs only inside a wasm guest; this build is for the host"

// Emit panics on the host. See PlanSettings and PlanData for the pure seams a
// host test uses instead.
func (l *Lib) Emit() { panic(hostOnly) }

// EmitSettings panics on the host, like Emit.
func (l *Lib) EmitSettings() { panic(hostOnly) }

// EmitData panics on the host, like Emit.
func (l *Lib) EmitData() { panic(hostOnly) }
