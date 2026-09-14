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

// Pair builds one Extra field. KV's fields are exported, so a struct literal
// works too; this is the shorter form for a list of them.
//
// NAMED Pair RATHER THAN kv, and only because Go will not have both: this
// package already has an unexported kv that every prototype builder uses, and
// one package cannot carry two identifiers of one name. The Rust mirror
// exports its kv directly.
func Pair(key string, val Value) KV { return KV{Key: key, Val: val} }

// strArr turns a name list into an array of strings, the shape every
// prerequisite list and every localised-string parameter uses.
func strArr(names []string) Value {
	items := make([]Value, 0, len(names))
	for _, n := range names {
		items = append(items, Str(n))
	}
	return Arr(items...)
}

// localisedElementCeiling is the ENGINE's limit on one STRING ELEMENT of a
// localised string on a DATA-STAGE prototype, and localisedChunkBudget is THIS
// LIBRARY's own, the size a chunk is filled to. They are different numbers on
// purpose; see the two blocks below.
//
// MEASURED, not assumed (Factorio 2.0.77, build 84539, mac-arm64, steam; the
// binary was re-asked its version before the runs), with a throwaway probe mod:
//
//   - 200 BYTES PER STRING ELEMENT, policed on the key slot and on every
//     literal parameter alike. 200 loads; 201 refuses the whole load with
//     Error while loading recipe prototype "..." (recipe): Localised string key
//     is too large: 201 > 200 (limit). in property tree at
//     ROOT.recipe.<name>.localised_description[1]. The index is 0-BASED over
//     the elements, so {"", X} reports X at index 1 and a one-element {X} at 0.
//   - NO AGGREGATE BUDGET: sixteen elements of which fifteen are 199 bytes,
//     2985 bytes in one description, exit 0.
//   - BYTES AND NOT CHARACTERS: 100 x U+00E9 is 100 characters and 200 bytes
//     and loads; 101 is 202 bytes and refuses reporting 202 > 200.
//   - localised_name is policed identically (ROOT.recipe.inserter.
//     localised_name[1]), and item, recipe and technology prototypes all carry
//     the rule.
//   - A SETTING PROTOTYPE IS NOT SUBJECT TO IT AT ALL. A string-setting loaded
//     with 201, 400, 1000, 2000 and 5000-byte elements in localised_description
//     and in localised_name: exit 0 every time, no message of any kind, every
//     byte reaching mod-settings-dump.json.
//
// THE SETTINGS SIDE IS THEREFORE NOT CHUNKED, and that is a rule rather than an
// oversight. The measured negative above says it does not have to be, and the
// locale guard says it must not be: localisedCarries compares a composed line
// WHOLE against one Str element (locale.go), so a chunked settings line would
// make CheckLocale report every composed line as missing and move
// testdata/locale/findings.golden. Do not unify the two sides.
//
// The ceiling is COMPARED AGAINST AND NEVER COMPUTED WITH, the habit
// craftTimeFloor established: nothing here derives the budget from it. It has
// TWO readers now, the host test that walks every composition this library can
// build and descriptionRef, which cannot chunk the key it composes and drops it
// above this length instead. The second is a COMPARISON and not a derivation
// (localisedChunkBudget is still the only thing a chunk is filled to, and it is
// its own constant), so the rule holds with a production reader beside the
// test.
const localisedElementCeiling = 200

// localisedChunkBudget is what a chunk is filled to, and it is twenty bytes
// short of the engine's ceiling for three reasons, none of them arithmetic:
// an off-by-one in the chunker cannot land exactly on a refusal; a hard cut
// inside an over-long word backs off to a UTF-8 character boundary and loses up
// to three bytes without approaching the ceiling; and a later edit to one of
// these sentences has room before anything has to be rechunked.
const localisedChunkBudget = 180

// chunkLocalised splits text into pieces no localised-string element ceiling
// can refuse, over BYTES, preferring to end a chunk after an ASCII space.
//
// FOUR PROPERTIES, BY CONSTRUCTION:
//
//   - every chunk is at most localisedChunkBudget bytes long, so no chunk can
//     reach localisedElementCeiling;
//   - the chunks concatenated are the input byte for byte. The engine
//     concatenates a localised string's parameters, so nothing a player reads
//     changes;
//   - no chunk ever ends inside a UTF-8 character;
//   - the split is a pure function of the bytes, so the Go and the Rust half
//     cannot disagree about it.
//
// THE CONTINUATION-BYTE BACK-OFF CANNOT SPIN. A run of continuation bytes
// longer than the budget is not UTF-8 at all, which a Go string is free to hold
// and a Rust &str is not; the guard below keeps the loop advancing on such a
// string rather than emitting an empty chunk forever, and the Rust mirror
// carries the same guard so the two halves are the same code and not merely the
// same outcome.
func chunkLocalised(text string) []string {
	if len(text) <= localisedChunkBudget {
		return []string{text}
	}
	var out []string
	for start := 0; start < len(text); {
		rem := text[start:]
		if len(rem) <= localisedChunkBudget {
			out = append(out, rem)
			break
		}
		var cut int
		if sp := lastSpace(rem[:localisedChunkBudget]); sp >= 0 {
			// The space ENDS the chunk it was found in rather than opening
			// the next one, so a break between words keeps the space with
			// the words before it.
			//
			// IT IS NOT A CLAIM ABOUT EVERY CHUNK. The hard cut below takes no
			// notice of what follows it, so "z"x180 + " " + "z"x100 splits
			// into 180 and 101 bytes and the second chunk DOES begin with the
			// space. The engine concatenates the parameters, so a reader sees
			// the same text either way; this arm is about where a break falls
			// when there is a choice, not about what a chunk may start with.
			cut = sp + 1
		} else {
			cut = localisedChunkBudget
			for cut > 0 && isUTF8Continuation(rem[cut]) {
				cut--
			}
			if cut == 0 {
				cut = localisedChunkBudget
			}
		}
		out = append(out, rem[:cut])
		start += cut
	}
	return out
}

// lastSpace is the byte index of the last ASCII space in s, or -1.
func lastSpace(s string) int {
	for i := len(s) - 1; i >= 0; i-- {
		if s[i] == ' ' {
			return i
		}
	}
	return -1
}

// isUTF8Continuation reports whether b is the second or later byte of a UTF-8
// character, which is where a cut may not land.
func isUTF8Continuation(b byte) bool { return b&0xC0 == 0x80 }

// localisedChunks is chunkLocalised's result as localised-string parameters.
func localisedChunks(text string) []Value {
	pieces := chunkLocalised(text)
	out := make([]Value, 0, len(pieces))
	for _, p := range pieces {
		out = append(out, Str(p))
	}
	return out
}

// localised builds the inline localised-string form {"", text}, splitting text
// across as many parameters as the engine's element ceiling needs. The engine
// takes a plain string as the parameter and refuses a number (measured), so the
// caller stringifies before it gets here.
//
// A TEXT INSIDE THE BUDGET IS ONE CHUNK, so the shape stays {"", text} byte for
// byte and a golden taken before this splitter existed does not move for it.
func localised(text string) Value { return localisedGroup(localisedChunks(text)) }

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

// craftTimeFloor is the engine's exclusive floor for a recipe's
// energy_required. A value must be STRICTLY GREATER than this.
//
// MEASURED, not assumed (Factorio 2.0.77, build 84539, mac-arm64, steam; the
// binary was re-asked its version before the runs). energy_required = 0 and
// energy_required = -1 both refuse the load, exit 1, with the engine's own
// message: Error while loading recipe prototype "..." (recipe):
// energy_required can't be <= 0.001. Values 0.0011 and 0.002 load (exit 0)
// and reach data-raw-dump.json unchanged. An omitted energy_required stays
// absent, and the engine applies its 0.5 default at runtime.
//
// Compared against, never computed with: a const expression folded by two
// languages is the measured FkLua trap this repository already carries.
const craftTimeFloor float64 = 0.001

// craftTimeAutoMinimum is the smallest round value safely above the floor. A
// generated setting that backs a crafting time gets it as its minimum unless
// the consumer set one, so the settings GUI cannot produce a value that kills
// the load.
const craftTimeAutoMinimum float64 = 0.002
