//go:build tinygo.wasm

// The emit layer: the only code in this library that touches fkdata, gated so
// the pure half stays host-testable (a //go:wasmimport is rejected off-target).
//
// IT IS DELIBERATELY THIN. Every decision was made in the pure half and is
// already in the Op stream by the time anything here runs; this file gathers
// what the World asks for, hands the plan back, and executes what comes out.
// Nothing here may branch on a value it read, because a branch here is a
// branch no host test can reach.
//
// NO HOOKS ARE EXPORTED. A wasm module has one export per name, so
// //go:wasmexport fk_data here would take that name away from the consuming
// mod. The consumer owns the four stage exports and calls Emit from them.
package fkrecipes

import "github.com/Techrocket9/fklua/guest/go/fkdata"

// Emit plans and then writes. It is the one call a consumer makes:
//
//	//go:wasmexport fk_data
//	func onData() { lib.Emit() }
//
// ROUTE fk_settings AND EXACTLY ONE DATA-FAMILY HOOK INTO IT. data, updates
// and final-fixes share one Lua state and one data.raw, so a second
// data-family call would find the first pass's prototypes already there and
// refuse as an overwrite. Which one is the consumer's choice: fk_data for
// content of their own, fk_data_updates to sit after other mods.
//
// The stage decides which plan runs, and the plan is rebuilt every time: the
// module is instantiated fresh per stage, so the consumer's declarations run
// again and nothing carries across a stage boundary.
func (l *Lib) Emit() {
	// The dispatch is decided in the pure half, where a test can reach it.
	// A stage this library does not plan for is REFUSED rather than treated
	// as a data stage: see StageKindOf for why the old everything-else-is-data
	// shape was a misroute waiting for a fifth stage.
	stage := fkdata.Stage().Name()
	kind, ok := StageKindOf(stage)
	if !ok {
		fkdata.Raise("fkrecipes: the stage " + stage + " is not one this library plans for; route fk_settings and one data-family hook into Emit")
	}
	if kind == StageKindSettings {
		l.EmitSettings()
		return
	}
	l.EmitData()
}

// EmitSettings plans and writes the SETTINGS stage only, and raises if it is
// called anywhere else.
//
// WHY THE SPLIT EXISTS, measured by the pilot. Emit reaches both planners, so
// a guest that only generates settings still links PlanData: BetterBeltBalancer
// measured it as a 21047-line Lua function that never runs, carried in every
// player's download. Naming the half you use lets the linker drop the other.
//
// The stage check is not a formality either. A settings plan run at a data
// stage would ask for prototypes that do not exist yet, so the wrong routing
// is caught with a sentence naming the hook rather than as a confusing probe
// failure later.
func (l *Lib) EmitSettings() {
	stage := fkdata.Stage().Name()
	if kind, ok := StageKindOf(stage); !ok || kind != StageKindSettings {
		fkdata.Raise("fkrecipes: EmitSettings was called at the " + stage + " stage; route it from fk_settings")
	}
	ops, err := l.PlanSettings(newDataWorld())
	l.run(ops, err)
}

// EmitData plans and writes a DATA-family stage only, and raises if it is
// called anywhere else. See EmitSettings for why the split exists.
func (l *Lib) EmitData() {
	stage := fkdata.Stage().Name()
	if kind, ok := StageKindOf(stage); !ok || kind != StageKindData {
		fkdata.Raise("fkrecipes: EmitData was called at the " + stage + " stage; route it from one data-family hook")
	}
	ops, err := l.PlanData(newDataWorld())
	l.run(ops, err)
}

// run is the shared tail: refuse, or execute the stream.
func (l *Lib) run(ops []Op, err error) {
	if err != nil {
		// THE MESSAGE CARRIES NO STAGE OF ITS OWN. Raise is the host's own
		// failure path, and fk_data.lua's fail() prefixes "fklua: at the
		// <stage> stage, " before this text; a stage in the planner's string
		// too would say it twice.
		fkdata.Raise(err.Error())
	}

	for _, op := range ops {
		switch op.Kind {
		case OpExtend:
			// One prototype per call. Extend takes a variadic, but a plan's
			// prototypes are independent and a failure names the offending
			// one better when it is the only one in the call.
			fkdata.Extend(toV(op.Proto))
		case OpSet:
			// A nil VALUE here would DELETE the key rather than write one.
			// The planner never puts a Nil in a Set op and a pure-half test
			// asserts it, which is what keeps this call a write.
			fkdata.Set(toV(op.Val), pathOf(op.Path)...)
		case OpLog:
			fkdata.Log(op.Line)
		}
	}
}

// dataWorld answers the planner's questions out of data.raw.
//
// It carries two caches, both scoped to one Emit and both there because a
// probe is not free: every fkdata call marshals through the boundary, and the
// planner asks about the same ingredient names once per recipe that mentions
// them.
type dataWorld struct {
	// The engine's item types, read ONCE. env(5) cannot change during a
	// stage, and fkdata rebuilds its answer per call.
	itemTypes []string

	// The engine's entity types, read the same way and for the same reason:
	// PlaceResult names one, and "entity" is an abstract base with dozens of
	// concrete children.
	//
	// READ LAZILY, unlike itemTypes. Every plan has ingredients, so the item
	// list always pays for itself; PlaceResult is rare, and a plan with none
	// should not pay a DerivedTypes call for a probe it never makes.
	entityTypes []string
	entityRead  bool

	// Answers already given, as a slice of pairs scanned linearly. Not a map:
	// this package does not iterate maps, and a plan's ingredient set is
	// small enough that a scan is cheaper than the probe it saves.
	itemAnswers   []itemAnswer
	entityAnswers []itemAnswer
}

type itemAnswer struct {
	name    string
	present bool
}

func newDataWorld() *dataWorld {
	return &dataWorld{itemTypes: fkdata.DerivedTypes("item")}
}

func (w *dataWorld) ModName() string { return fkdata.ModName() }

func (w *dataWorld) StartupSetting(name string) (Value, bool) {
	v, ok := fkdata.StartupSetting(name)
	if !ok {
		return Nil(), false
	}
	return fromV(v), true
}

// TechNames is Keys, which fkdata returns SORTED at every path. That is what
// the World contract asks of this method, so the guarantee is the host shim's
// rather than something this layer has to arrange.
func (w *dataWorld) TechNames() []string { return fkdata.Keys("technology") }

func (w *dataWorld) TechPrereqs(name string) []string {
	v, ok := fkdata.Get("technology", name, "prerequisites")
	if !ok {
		return nil
	}
	out := make([]string, 0, len(v.Arr))
	for _, p := range v.Arr {
		if p.Tag == fkdata.TagString {
			out = append(out, p.Str)
		}
	}
	return out
}

func (w *dataWorld) TechUnit(name string) (Value, bool) {
	v, ok := fkdata.Get("technology", name, "unit")
	if !ok {
		return Nil(), false
	}
	return fromV(v), true
}

func (w *dataWorld) TechMaxLevel(name string) (Value, bool) {
	v, ok := fkdata.Get("technology", name, "max_level")
	if !ok {
		return Nil(), false
	}
	return fromV(v), true
}

// TechHasResearchTrigger asks whether the FIELD is there, and reads nothing
// out of it: a research_trigger technology is identified by carrying one, and
// its shape is the engine's business.
func (w *dataWorld) TechHasResearchTrigger(name string) bool {
	_, ok := fkdata.Get("technology", name, "research_trigger")
	return ok
}

// TechExists and RecipeExists probe the prototype's NAME LEAF rather than the
// prototype. "Is this defined" is a yes or no, and a Get of the prototype
// marshals every field it has across the boundary to answer it; the leaf is
// one string. Measured in BetterBeltBalancer, which is where the shape comes
// from.
func (w *dataWorld) TechExists(name string) bool {
	_, ok := fkdata.Get("technology", name, "name")
	return ok
}

func (w *dataWorld) RecipeExists(name string) bool {
	_, ok := fkdata.Get("recipe", name, "name")
	return ok
}

// ItemExists probes the plain "item" type FIRST and only then walks the rest.
//
// MEASURED (Factorio 2.0.77, build 84539): defines.prototypes.item has 21
// keys and INCLUDES "item" itself, alongside ammo, armor, capsule, gun,
// module, tool and the rest. data.raw has an "item" table like any other, and
// the overwhelming majority of the names a recipe names live in it, so trying
// it first answers the common case in one probe. The remaining types are
// walked in the sorted order DerivedTypes returns, skipping the one already
// tried; an item name is unique across those types, because they share one
// namespace, so the first hit is the answer.
//
// The answer is remembered for the rest of this Emit: the planner asks about
// the same names once per recipe that mentions them.
func (w *dataWorld) ItemExists(name string) bool {
	for _, seen := range w.itemAnswers {
		if seen.name == name {
			return seen.present
		}
	}
	present := w.probeItem(name)
	w.itemAnswers = append(w.itemAnswers, itemAnswer{name: name, present: present})
	return present
}

// EntityExists is ItemExists over the entity family: the same memo, the same
// named-type-first-then-derived walk, because "entity" is an abstract base and
// a simple-entity-with-force is not in data.raw.entity.
func (w *dataWorld) EntityExists(name string) bool {
	for _, seen := range w.entityAnswers {
		if seen.name == name {
			return seen.present
		}
	}
	if !w.entityRead {
		w.entityTypes = fkdata.DerivedTypes("entity")
		w.entityRead = true
	}
	present := probeIn("entity", w.entityTypes, name)
	w.entityAnswers = append(w.entityAnswers, itemAnswer{name: name, present: present})
	return present
}

func (w *dataWorld) probeItem(name string) bool {
	return probeIn("item", w.itemTypes, name)
}

// probeIn asks the named type first and then every type derived from it. The
// order matters for cost rather than correctness: the overwhelmingly common
// answer is the base type, and asking it first skips the walk.
func probeIn(base string, derived []string, name string) bool {
	if _, ok := fkdata.Get(base, name, "name"); ok {
		return true
	}
	for _, typ := range derived {
		if typ == base {
			continue
		}
		if _, ok := fkdata.Get(typ, name, "name"); ok {
			return true
		}
	}
	return false
}

// pathOf turns an Op path into the argument list Get, Set and Keys take.
func pathOf(path []PathEl) []any {
	out := make([]any, 0, len(path))
	for _, el := range path {
		if el.IsNum {
			out = append(out, el.Num)
			continue
		}
		out = append(out, el.Str)
	}
	return out
}

// toV and fromV are the whole boundary between the pure value model and
// fkdata's. They are exact and total in both directions: a map's pairs keep
// the order the planner built them in (fkdata sorts on the way out), an array
// stays an array, and a number is a float64 on both sides because Factorio has
// one number type.
func toV(v Value) fkdata.V {
	switch v.Kind {
	case KindBool:
		return fkdata.Bool(v.Bool)
	case KindNum:
		return fkdata.Num(v.Num)
	case KindStr:
		return fkdata.Str(v.Str)
	case KindArr:
		items := make([]fkdata.V, 0, len(v.Arr))
		for _, item := range v.Arr {
			items = append(items, toV(item))
		}
		return fkdata.Arr(items...)
	case KindMap:
		pairs := make([]fkdata.KV, 0, len(v.Map))
		for _, p := range v.Map {
			pairs = append(pairs, fkdata.KVs(p.Key, toV(p.Val)))
		}
		return fkdata.Obj(pairs...)
	}
	return fkdata.Nil()
}

// fromV carries a read back, TOTALLY: every value either converts or becomes
// Nil as a whole subtree, and nothing is ever partly kept.
//
// A NUMBER-KEYED MAP REALLY DOES ARRIVE. fk_data.lua's key_rank accepts
// numbers (rank 1) as well as strings (rank 2) and refuses only the rest, so
// any holed or mixed Lua table crosses as a map with numeric keys. Keeping the
// string pairs of such a table and dropping the others would turn a copied
// unit's ingredient list into an empty one, which is a technology researchable
// for free. So the whole subtree becomes Nil instead, and because fkdata's own
// write side skips nils it never delivers one, which makes a Nil here an
// unambiguous marker: the pure half refuses any copied unit that contains one,
// by name.
func fromV(v fkdata.V) Value {
	switch v.Tag {
	case fkdata.TagBool:
		return Bool(v.Bool)
	case fkdata.TagNumber:
		return Num(v.Num)
	case fkdata.TagString:
		return Str(v.Str)
	case fkdata.TagArray:
		items := make([]Value, 0, len(v.Arr))
		for _, item := range v.Arr {
			items = append(items, fromV(item))
		}
		return Arr(items...)
	case fkdata.TagMap:
		pairs := make([]KV, 0, len(v.Map))
		for _, p := range v.Map {
			if p.Key.Tag != fkdata.TagString {
				return Nil()
			}
			pairs = append(pairs, KV{Key: p.Key.Str, Val: fromV(p.Val)})
		}
		return Obj(pairs...)
	}
	// The default arm is the closed tag set's remainder: TagNil, and TagObject
	// for a LuaObject. fk_data.lua binds no handle table, so an object read is
	// not expected at a data stage; it lands here rather than being called
	// impossible, and a copied unit that contains one is refused by name.
	return Nil()
}
