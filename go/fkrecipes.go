// Package fkrecipes is a FkLua guest library: a Go package a mod's guest
// imports, compiled into the CONSUMER's wasm by their own `fklua mod`.
//
// THE COMPOSITION CONTRACT, in the order it bites:
//
//   - ROUTE, NEVER OWN. Never export a hook (fk_on_tick, fk_on_event,
//     fk_on_init, fk_alloc, ...): a wasm module has ONE export per name, so an
//     export here takes it away from the consuming mod. The consumer owns the
//     hooks and routes in -- OnEvent below returns whether this library
//     handled the call, which is what lets several libraries share one mod.
//   - THE CONSUMER'S BINDINGS, NEVER YOUR OWN. Depending on `fk` alone keeps
//     this library PIN-TRANSPARENT: no API version can break it. If it ever
//     needs the generated fkapi bindings, import the CONSUMER's copy (the
//     canonical module path) and never vendor one -- two binding sets in one
//     module are refused at package time, and ids are pin-relative.
//   - IDS STAY INLINE. The packager prunes the member/event/define tables by
//     scanning for compile-time-constant ids. Keep any wrapper that carries an
//     id small enough to inline, never loop over ids, and write calls out --
//     the failure is silent and costs every consumer the full tables in every
//     save and download.
//   - JOIN SAFETY PASSES THROUGH. Never store whether an outbound host call
//     succeeded (that is a fact about one peer), never keep anything computed
//     under a load hook, and never iterate a Go map to decide what to write.
//     A consumer cannot see this library break these rules; the symptom is a
//     multiplayer desync with their mod's name on it.
//
// The pure half lives here and is testable with plain `go test`; everything
// that touches a host import lives in guest.go behind the build gate.
package fkrecipes

// Lib is the library's state. Everything here lives in the guest heap and
// therefore in every save: allocate on a schedule you chose, not per tick.
type Lib struct {
	ticks uint32
}

// OnTick is an example entry point the consumer calls from its own fk_on_tick
// export. Pure, so it is host-testable.
func (l *Lib) OnTick() uint32 {
	l.ticks++
	return l.ticks
}

// OnEvent is the routing convention: the consumer's fk_on_event calls every
// library it imports in turn, and the return says whether this one handled
// the event -- so libraries compose without owning the export.
func (l *Lib) OnEvent(id uint32, ptr uint32) bool {
	_, _ = id, ptr
	return false
}
