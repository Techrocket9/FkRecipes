package fkrecipes

import (
	"strconv"
	"strings"
	"testing"
)

// THE ENGINE'S ELEMENT CEILING, OVER EVERY COMPOSITION THIS LIBRARY CAN WRITE
// ONTO A DATA-STAGE PROTOTYPE.
//
// The rule is measured and recorded at localisedElementCeiling: ONE STRING
// ELEMENT of a localised string on a data prototype is at most 200 BYTES, and
// the 201st refuses the WHOLE LOAD with a message naming the prototype and the
// element index. That is a lock-out rather than a degradation, so the property
// this file asserts is a load-bearing one: nothing this library composes may
// reach it, on any mod name, with any setting spelling, on any mod set.
//
// IT IS A WALK AND NOT A LIST OF SENTENCES, deliberately. A test that measured
// the six notes one at a time would pass the day a seventh is added and would
// say nothing about the composition each note lands in: a note joins an
// author's own Description, and the pair is what the engine reads. So the
// fixture below reaches every composition from the PUBLIC surface, with the
// longest names the engine's own name ceiling allows substituted into every
// slot a sentence names, and the assertion walks what PlanData actually
// emitted.
//
// THE DATA SIDE ONLY. A setting prototype is not subject to the rule at all
// (measured; see localisedElementCeiling), its composed lines are compared
// WHOLE by the locale guard, and chunking them would move
// testdata/locale/findings.golden. PlanSettings is therefore not walked here,
// and that is the rule rather than an omission.

// prototypeNameCeiling is the ENGINE's own limit on a PROTOTYPE NAME, recorded
// in agents/customizer-design.md: 201 bytes refuses the load with `Name field
// is too large. Max allowed size is: 200.` and 200 loads. It is a DIFFERENT
// RULE from the element ceiling, with a different message, which is why
// assertNameFits is a different assertion.
const prototypeNameCeiling = 200

// fixtureNameBytes is how long THIS FIXTURE makes its names, and it is a second
// constant at the same number rather than a reuse of the first, for the reason
// localisedChunkBudget is a second constant beside localisedElementCeiling: one
// belongs to the engine and one to this file, and assertNameFits is the claim
// that the second is inside the first. Reusing one constant for both would make
// that assertion true by construction and impossible to red-prove.
const fixtureNameBytes = 200

// fixturePrefix is what the data stage puts in front of every name this plan
// declares, derived from the fixture World's ModName.
const fixturePrefix = "steelworks-"

// declaredName is a name this plan declares, sized so the name the library
// EMITS is exactly fixtureNameBytes.
func declaredName(stem string) string { return padName(stem, fixtureNameBytes-len(fixturePrefix)) }

// existingName is a name the GAME carries, at the same length. Nothing is
// prefixed onto a name this library only reads.
func existingName(stem string) string { return padName(stem, fixtureNameBytes) }

func padName(stem string, n int) string {
	if len(stem) >= n {
		panic("the stem " + stem + " does not fit in " + strconv.Itoa(n) + " bytes")
	}
	return stem + "-" + strings.Repeat("q", n-len(stem)-1)
}

// fixtureProse is n bytes of ordinary spaced words, which is what a consumer's
// Description looks like and what the chunker's SPACE arm walks.
func fixtureProse(n int) string {
	var b strings.Builder
	for i := 0; b.Len() < n; i++ {
		if i > 0 {
			b.WriteByte(' ')
		}
		b.WriteString("word")
		b.WriteString(strconv.Itoa(i))
	}
	return b.String()[:n]
}

// fixtureUnbrokenProse is the one input that reaches the chunker's HARD CUT and
// its UTF-8 back-off: a single run longer than the budget, with a two-byte
// character sitting exactly across the byte a blind cut would land on.
func fixtureUnbrokenProse() string {
	return strings.Repeat("z", localisedChunkBudget-1) + "\u00e9" +
		strings.Repeat("z", 60) + " and a tail."
}

// TestNoCompositionReachesTheElementCeiling walks every prototype the fixture
// plan emits and asserts the two ceilings a localised string has: 200 bytes per
// string element, and twenty-one elements per table (the leading "" plus the
// twenty parameters localisedGroup fills a level to).
func TestNoCompositionReachesTheElementCeiling(t *testing.T) {
	lib, w := worstCasePlan()

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	// THE WALK IS WORTHLESS OVER AN EMPTY STREAM, and a fixture that stopped
	// reaching the notes would leave every assertion below vacuous, so the
	// compositions are counted before they are measured.
	described := 0
	for _, op := range ops {
		if op.Kind == OpExtend {
			described += localisedDescriptions(op.Proto)
		}
	}
	if described != 16 {
		t.Fatalf("the fixture emitted %d localised_description fields, not the 16 it declares;"+
			" the walk below would prove nothing about the ones it lost", described)
	}

	for _, op := range ops {
		switch op.Kind {
		case OpExtend:
			assertNameFits(t, op.Proto)
			walkForCeilings(t, protoWhere(op.Proto), op.Proto)
		case OpSet:
			walkForCeilings(t, "a spliced field", op.Val)
		}
	}
}

// assertNameFits is a SEPARATE RULE WITH A SEPARATE MESSAGE, and it is separate
// on purpose.
//
// The element ceiling below is about localised strings and nothing else: the
// engine carries non-localised strings far past 200 bytes without complaint
// (base's own utility-constants holds a 2697-byte one), so a walk that measured
// every string in a prototype would refuse what the engine loads and would cite
// the wrong sentence doing it. A prototype NAME has its own limit at the same
// number and its own refusal: see prototypeNameCeiling.
//
// WHAT IT IS FOR IS THE FIXTURE. Every name below is padded to
// fixtureNameBytes so that every sentence is composed at its worst case, and
// this is the check that the padding did not walk off the end of what the
// engine takes: a fixture whose names the engine would refuse is a fixture
// proving nothing about a load that can happen.
func assertNameFits(t *testing.T, proto Value) {
	t.Helper()
	name, ok := field(proto, "name")
	if !ok || name.Kind != KindStr {
		return
	}
	if len(name.Str) > prototypeNameCeiling {
		t.Errorf("%s is %d bytes, over the engine's PROTOTYPE NAME ceiling of %d"+
			" (a different rule from the element ceiling: `Name field is too large.`)",
			protoWhere(proto), len(name.Str), prototypeNameCeiling)
	}
}

// protoWhere names a prototype the way the engine's own refusal does, so a
// failure here is greppable against a real load failure.
func protoWhere(proto Value) string {
	kind, _ := field(proto, "type")
	name, _ := field(proto, "name")
	return "ROOT." + kind.Str + "." + name.Str
}

// walkForCeilings recurses into every Value a prototype carries. The localised
// flag turns on under a localised_name or a localised_description and stays on
// below it.
//
// BOTH RULES ARE SCOPED BY THAT FLAG, and that is the whole point of it. The
// engine's element ceiling is a rule about LOCALISED STRINGS: a prototype is
// free to carry a longer string anywhere else, and base's own utility-constants
// does. A walk that measured every string would fail a consumer's long order or
// icon field citing a rule the engine does not apply there. The two named
// fields are exactly what testdata/mirror/standin.lua's check_localised and the
// in-game gate's own assertion measure, so the three enforcement points say one
// thing. A prototype name is policed at the same number by a different rule and
// has its own assertion: see assertNameFits.
func walkForCeilings(t *testing.T, where string, proto Value) {
	t.Helper()
	walkValueForCeilings(t, where, "", proto, false)
}

func walkValueForCeilings(t *testing.T, where, path string, v Value, localised bool) {
	t.Helper()
	switch v.Kind {
	case KindStr:
		if localised && len(v.Str) > localisedElementCeiling {
			t.Errorf("%s%s is %d bytes, over the engine's element ceiling of %d:\n  %q",
				where, path, len(v.Str), localisedElementCeiling, v.Str)
		}
	case KindArr:
		if localised && len(v.Arr) > maxLocalisedParams+1 {
			t.Errorf("%s%s holds %d elements, over the %d a localised string takes",
				where, path, len(v.Arr), maxLocalisedParams+1)
		}
		for i, item := range v.Arr {
			walkValueForCeilings(t, where, path+"["+strconv.Itoa(i)+"]", item, localised)
		}
	case KindMap:
		for _, p := range v.Map {
			under := localised || p.Key == "localised_name" || p.Key == "localised_description"
			walkValueForCeilings(t, where, path+"."+p.Key, p.Val, under)
		}
	}
}

// worstCasePlan is every composition this library can write onto a data
// prototype, each one reached from the PUBLIC surface and each one with the
// longest legal name in the slot its sentence names.
//
// SIXTEEN DESCRIPTIONS, and the count is asserted above:
//
//	an item with a display name and a description;
//	a recipe carrying the FALLBACK note, with a Description and without;
//	a recipe carrying the clamped ITEM note, with and without;
//	a recipe carrying the clamped FLUID note, with and without;
//	a technology carrying the FALLBACK note, with and without;
//	a technology carrying the DROPPED-PACK note, with and without;
//	a technology carrying the PACKLESS-SOURCE note, with and without;
//	a technology carrying the clamped PACK note, with and without;
//	and an item whose Description alone is long enough to NEST.
//
// THE PAIRS ARE PAIRS BECAUSE THE COMPOSITION IS WHAT IS WALKED, not the note:
// a note beside an author's Description and a note alone are two different
// localised strings, and only one of them can nest. That is why a seventh note
// adds TWO rows here and not one.
//
// THE CLAMPED PACK NOTE IS THE SAME SENTENCE AS THE CLAMPED ITEM ONE AND IS NOT
// THE SAME COMPOSITION. mergePack writes clampedItemNote BARE, with no
// destruction sentence, because a research costs no assembling machine
// anything; mergeIngredient writes it with the sentence. Two composers, two
// lengths, and the bare one had no test in either suite before this row.
//
// Every RECIPE note carries the destruction sentence, which is the longest of
// the shapes each can take: the ingredient list is what moved in all three
// recipe cases.
func worstCasePlan() (*Lib, *fixtureWorld) {
	item := existingName("iron-plate")
	fluid := existingName("water")
	pack := existingName("automation-science-pack")
	absentPack := existingName("logistic-science-pack")
	dropSource := existingName("logistics-2")
	packlessSource := existingName("steel-processing")

	// A Description that forces localisedGroup to NEST: 7200 bytes is forty
	// chunks at the budget, and one level holds twenty.
	nesting := fixtureProse(7200)

	lib := New()

	// The item, and the two shapes an item's own prose takes: a display name
	// and a description, both past the budget, and a description long enough
	// to nest on its own.
	lib.Item(declaredName("described-item"), ItemSpec{
		DisplayName: fixtureProse(900),
		Description: fixtureUnbrokenProse(),
	})
	lib.Item(declaredName("nesting-item"), ItemSpec{Description: nesting})

	// THE FALLBACK NOTE ON A RECIPE. Two settings rather than one, so the
	// described and the undescribed recipe are two independent prototypes with
	// two independent notes.
	for i, describe := range []bool{false, true} {
		stem := "fallback-recipe-" + strconv.Itoa(i)
		result := lib.Item(declaredName(stem+"-item"), ItemSpec{})
		parts := lib.IngredientsSetting(declaredName(stem+"-setting"),
			[]Ingredient{IngredientNamed(1, item)})
		lib.Recipe(result, RecipeSpec{
			Name:            declaredName(stem),
			Description:     describedProse(describe, nesting),
			IngredientsFrom: parts,
		})
	}

	// THE CLAMPED ITEM NOTE, a merge above the engine's 65535.
	for i, describe := range []bool{false, true} {
		stem := "clamped-item-recipe-" + strconv.Itoa(i)
		result := lib.Item(declaredName(stem+"-item"), ItemSpec{})
		lib.Recipe(result, RecipeSpec{
			Name:        declaredName(stem),
			Description: describedProse(describe, fixtureProse(400)),
			Ingredients: []Ingredient{
				IngredientNamed(40000, item),
				IngredientNamed(30000, existingName("transport-belt"), item),
			},
		})
	}

	// THE CLAMPED FLUID NOTE, the same merge above the fluid ceiling.
	for i, describe := range []bool{false, true} {
		stem := "clamped-fluid-recipe-" + strconv.Itoa(i)
		result := lib.Item(declaredName(stem+"-item"), ItemSpec{})
		lib.Recipe(result, RecipeSpec{
			Name:        declaredName(stem),
			Category:    "chemistry",
			Description: describedProse(describe, fixtureProse(400)),
			Ingredients: []Ingredient{
				FluidIngredient(5e300, fluid),
				FluidIngredient(6e300, existingName("steam"), fluid),
			},
		})
	}

	// THE FALLBACK NOTE ON A TECHNOLOGY, through a custom research cost.
	for i, describe := range []bool{false, true} {
		stem := "fallback-tech-" + strconv.Itoa(i)
		packs := lib.PacksSetting(declaredName(stem+"-packs"),
			[]Pack{{Name: "automation-science-pack", Amount: 1}})
		count := lib.IntSetting(declaredName(stem+"-count"), 20, Between(1, 100000))
		seconds := lib.IntSetting(declaredName(stem+"-seconds"), 10, Between(1, 600))
		lib.Technology(declaredName(stem), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			CostFrom:    &CustomCost{Packs: packs, Count: count, Seconds: seconds},
		})
	}

	// THE DROPPED-PACK NOTE: a copied unit naming a pack this game does not
	// have as a tool.
	for i, describe := range []bool{false, true} {
		lib.Technology(declaredName("dropped-pack-tech-"+strconv.Itoa(i)), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			CostOf:      dropSource,
		})
	}

	// THE PACKLESS-SOURCE NOTE: a tier whose source names no pack this game
	// has, so the author's own declared cost applies.
	for i, describe := range []bool{false, true} {
		stem := "packless-tech-" + strconv.Itoa(i)
		tier := lib.DropdownSettingNeedingLocale(declaredName(stem+"-tier"), "early", []string{"early"})
		lib.Technology(declaredName(stem), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			CostBy: &CostChoices{
				Setting:  tier,
				Choices:  []CostChoice{{Value: "early", Sources: []string{packlessSource}}},
				Fallback: UnitSpec{Count: 7, Seconds: 8, Packs: []Pack{{Name: "automation-science-pack", Amount: 2}}},
			},
		})
	}

	// THE CLAMPED PACK NOTE: two declared packs whose ladders land on one name,
	// each at the ceiling, so the SUM is a number no author wrote. It is the
	// only pack amount validateUnit does not already hold to 65535, and the
	// note it leaves is clampedItemNote BARE: mergePack writes no destruction
	// sentence, because a research costs no assembling machine anything.
	for i, describe := range []bool{false, true} {
		lib.Technology(declaredName("clamped-pack-tech-"+strconv.Itoa(i)), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			Unit: &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{
				{Name: pack, Amount: maxItemAmount},
				{Name: absentPack, Amount: maxItemAmount, Fallbacks: []string{pack}},
			}},
		})
	}

	// The copied unit the DROPPED-PACK note reads names one pack the game has
	// and one it does not; the PACKLESS-SOURCE one names only the pack it
	// does not.
	w := baseWorld().
		withItem(item).
		withFluid(fluid).
		withTool(pack).
		withTech(fixtureTech{name: dropSource, unit: unitOf(200, 30, absentPack, "automation-science-pack")}).
		withTech(fixtureTech{name: packlessSource, unit: unitOf(7, 8, absentPack)})

	// The two texts the player typed and the library cannot use. Everything
	// else the settings answer is left absent, which is the ordinary
	// unreadable-setting arm and composes nothing.
	for i := 0; i < 2; i++ {
		w = w.withSetting(fixturePrefix+declaredName("fallback-recipe-"+strconv.Itoa(i)+"-setting"),
			Str("1 "+existingName("nothing-is-named-this")))
		w = w.withSetting(fixturePrefix+declaredName("fallback-tech-"+strconv.Itoa(i)+"-packs"),
			Str("1 "+existingName("nothing-is-named-this")))
	}
	return lib, w
}

// describedProse is the with-a-Description arm of every pair above, and the
// empty string is the without arm. One helper rather than an if at every site,
// so the pairs read as pairs.
func describedProse(describe bool, prose string) string {
	if !describe {
		return ""
	}
	return prose
}

// THE SPLIT ITSELF, WRITTEN OUT BY HAND, which is the one place it is.
//
// Every transcript in the suite composes its expected parameters through
// chunkedParams, so a change to the budget moves one golden rather than thirty.
// That helper asks the chunker, so it cannot catch the chunker being wrong;
// this is what does. The three arms are the three the chunker has: a text
// inside the budget, a text that ends its chunks after a space, and a run with
// no space in it at all, where the cut backs off the middle of a UTF-8
// character.
func TestTheChunkerSplitsOnSpacesWithinTheBudget(t *testing.T) {
	e := "\u00e9" // two bytes, so a blind cut at the budget lands inside it

	for _, c := range []struct {
		what string
		in   string
		want []string
	}{
		{
			what: "a text inside the budget is one chunk and is not touched",
			in:   "Forged from plate.",
			want: []string{"Forged from plate."},
		},
		{
			what: "the empty string is one empty chunk",
			in:   "",
			want: []string{""},
		},
		{
			what: "exactly the budget is still one chunk",
			in:   strings.Repeat("z", 180),
			want: []string{strings.Repeat("z", 180)},
		},
		{
			what: "one byte past the budget splits, and the space ENDS the first chunk",
			in:   strings.Repeat("z", 175) + " abcde",
			want: []string{strings.Repeat("z", 175) + " ", "abcde"},
		},
		{
			what: "a run with no space is cut at the budget",
			in:   strings.Repeat("z", 181),
			want: []string{strings.Repeat("z", 180), "z"},
		},
		{
			what: "a cut that would land inside a UTF-8 character backs off to its first byte",
			in:   strings.Repeat("z", 179) + e + "abc",
			want: []string{strings.Repeat("z", 179), e + "abc"},
		},
		{
			what: "the note a recipe's ingredient text falls back with",
			in:   fallbackNote("steelworks-rivet-ingredients", true),
			want: []string{
				"The stored value of steelworks-rivet-ingredients could not be used," +
					" so this mod's own choice applies instead. The reason is in the log." +
					" Changing a recipe empties an assembling ",
				"machine's input slots of anything the new list does not use.",
			},
		},
	} {
		got := chunkLocalised(c.in)
		if len(got) != len(c.want) {
			t.Errorf("%s:\n got %d chunks: %q\nwant %d chunks: %q", c.what, len(got), got, len(c.want), c.want)
			continue
		}
		for i := range got {
			if got[i] != c.want[i] {
				t.Errorf("%s, chunk %d:\n got: %q\nwant: %q", c.what, i, got[i], c.want[i])
			}
		}
		// The two properties the assembly rests on, re-asked on every case:
		// the pieces are the input byte for byte, and none of them is over
		// the budget.
		if joined := strings.Join(got, ""); joined != c.in {
			t.Errorf("%s: the chunks do not concatenate to the input:\n got: %q\nwant: %q", c.what, joined, c.in)
		}
		for i, p := range got {
			if len(p) > localisedChunkBudget {
				t.Errorf("%s: chunk %d is %d bytes, over the budget of %d", c.what, i, len(p), localisedChunkBudget)
			}
		}
	}
}
