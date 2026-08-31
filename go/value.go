package fkrecipes

import "math"

// Value is a pure mirror of the value model fkdata builds prototypes from:
// enough of Lua to describe a prototype and nothing more. The emit layer
// translates it one node at a time, which is what keeps this package free of
// any fkdata import.
//
// A map is a slice of pairs in DECLARATION ORDER, never a Go map: the plan is
// compared byte for byte against the Rust mirror, and Go map iteration is
// randomised per run. fkdata sorts on the way out, so the order here is for
// the mirror and the transcripts, not for the engine.
type Value struct {
	Kind Kind
	Bool bool
	Num  float64
	Str  string
	Arr  []Value
	Map  []KV
}

// Kind tags which arm of Value carries the payload.
type Kind uint8

// The tags. KindNil is the zero value, so an unset Value is nil rather than a
// silently empty string.
const (
	KindNil Kind = iota
	KindBool
	KindNum
	KindStr
	KindArr
	KindMap
)

// KV is one map entry. The key is always a string: nothing this library emits
// is keyed by anything else.
type KV struct {
	Key string
	Val Value
}

// Nil returns the absent value.
func Nil() Value { return Value{Kind: KindNil} }

// Bool returns a boolean value.
func Bool(b bool) Value { return Value{Kind: KindBool, Bool: b} }

// Num returns a numeric value. Factorio has one number type and it is a
// double, so integers ride here too.
func Num(n float64) Value { return Value{Kind: KindNum, Num: n} }

// Str returns a string value.
func Str(s string) Value { return Value{Kind: KindStr, Str: s} }

// Arr returns an array value, which the emit layer writes as a Lua sequence.
func Arr(items ...Value) Value { return Value{Kind: KindArr, Arr: items} }

// Obj returns a map value. The pairs keep the order they are given in.
func Obj(pairs ...KV) Value { return Value{Kind: KindMap, Map: pairs} }

// kv is internal sugar; the exported literal is KV{Key: ..., Val: ...}.
func kv(key string, val Value) KV { return KV{Key: key, Val: val} }

// strArr turns a name list into an array of strings, the shape every
// prerequisite list and every localised-string parameter uses.
func strArr(names []string) Value {
	items := make([]Value, 0, len(names))
	for _, n := range names {
		items = append(items, Str(n))
	}
	return Arr(items...)
}

// localised builds the inline localised-string form {"", text}. The engine
// takes a plain string as the parameter and refuses a number (measured), so
// the caller stringifies before it gets here.
func localised(text string) Value { return Arr(Str(""), Str(text)) }

// finite is the guard every float crosses before it can reach an op. An
// infinity or a NaN in a prototype is a load failure the consumer cannot read
// their way out of, and the two languages print them differently, so the
// planner refuses instead of emitting one.
func finite(n float64) bool { return !math.IsNaN(n) && !math.IsInf(n, 0) }

// maxExactInt is the largest integer a Lua double holds exactly, 2^53. Past
// it the engine's own numbers start rounding, so a plan that declared an
// amount above it would emit a different one, silently. The planner refuses
// instead. The Rust mirror carries the same constant.
const maxExactInt int64 = 9007199254740992
