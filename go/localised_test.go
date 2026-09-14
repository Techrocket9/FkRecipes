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
// the notes one at a time would pass the day another is added and would
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
	if described != 30 {
		t.Fatalf("the fixture emitted %d localised_description fields, not the 30 it declares;"+
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
// THIRTY DESCRIPTIONS, and the count is asserted above:
//
//	an item with a display name and a description;
//	a recipe carrying the FALLBACK note, with a Description and without;
//	a recipe carrying the clamped ITEM note, with and without;
//	a recipe carrying the clamped FLUID note, with and without;
//	a technology carrying the FALLBACK note, with and without;
//	a technology carrying the DROPPED-PACK note, with and without;
//	a technology carrying the PACKLESS-SOURCE note, with and without;
//	a technology carrying the UNPRICED-SOURCE note, with and without;
//	a technology carrying the PACKLESS note, with and without;
//	a technology carrying the UNREADABLE-SOURCE note, with and without;
//	a technology carrying the UNREADABLE-COPY note, with and without;
//	a technology carrying the CYCLE-PREREQUISITE note, with and without;
//	a technology carrying the CYCLE-SPLICE note, with and without;
//	a technology carrying the clamped PACK note, with and without;
//	a recipe and a technology at the longest name whose composed
//	  [<kind>-description] key FITS the element ceiling, both undescribed;
//	and an item whose Description alone is long enough to NEST.
//
// THE LAST TWO ROWS ADDED ARE THE ONES THE COUNT ALONE COULD NOT HAVE CAUGHT.
// unpricedSourceNote and unreadableCopyNote were composed by the library and
// walked by nothing here, which made the claim over this file ("every
// composition this library can write onto a data prototype") false while every
// assertion in it passed. A composition missing from the fixture is invisible
// to the count, because the count is of what the fixture emits; the only guard
// is that the fixture is extended in the same commit as the composer. The note
// SET is now held mechanically by TestEveryNoteCallSiteIsAccountedFor, and a
// composer that appears there and not in this list is the next thing to add.
//
// THE PAIRS ARE PAIRS BECAUSE THE COMPOSITION IS WHAT IS WALKED, not the note:
// a note beside an author's Description and a note alone are two different
// localised strings, and only one of them can nest. That is why a further note
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
	unreadableSource := existingName("electronics")
	// A source the fixture World is never given, which is what the
	// unpriced-source arm needs: every rung of the tier's ladder absent.
	absentSource := existingName("nothing-carries-this-cost")

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

	// THE UNPRICED-SOURCE NOTE: a tier whose every source is absent from this
	// game, so nothing was copied at all and the technology is priced by the
	// author's own declared fallback with no prerequisite. The Fallback names a
	// pack the game HAS and one amount, so nothing worse than this takes the
	// slot: packlessAt and mergePack are both offered the slot first, by
	// construction, and the two rows here would be measuring one of those
	// sentences instead if either fired.
	for i, describe := range []bool{false, true} {
		stem := "unpriced-tech-" + strconv.Itoa(i)
		tier := lib.DropdownSettingNeedingLocale(declaredName(stem+"-tier"), "early", []string{"early"})
		lib.Technology(declaredName(stem), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			CostBy: &CostChoices{
				Setting:  tier,
				Choices:  []CostChoice{{Value: "early", Sources: []string{absentSource}}},
				Fallback: UnitSpec{Count: 7, Seconds: 8, Packs: []Pack{{Name: pack, Amount: 2}}},
			},
		})
	}

	// THE UNREADABLE-COPY NOTE: the unreadable source again, this time behind a
	// bare CostOf with nothing declared to fall back to, so the unit is emitted
	// with an empty ingredient list and the sentence says what could not be
	// read rather than what the game does not have.
	for i, describe := range []bool{false, true} {
		lib.Technology(declaredName("unreadable-copy-tech-"+strconv.Itoa(i)), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			CostOf:      unreadableSource,
		})
	}

	// THE PACKLESS NOTE: a hand-rolled unit whose only pack the game does not
	// have, so the technology is emitted with an empty ingredient list and the
	// tooltip says the research completes for free.
	for i, describe := range []bool{false, true} {
		lib.Technology(declaredName("packless-unit-tech-"+strconv.Itoa(i)), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			Unit:        &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{{Name: absentPack, Amount: 1}}},
		})
	}

	// THE UNREADABLE-SOURCE NOTE: a tier whose chosen source carries a pack
	// list in neither engine form, so the author's own declared cost applies.
	for i, describe := range []bool{false, true} {
		stem := "unreadable-tech-" + strconv.Itoa(i)
		tier := lib.DropdownSettingNeedingLocale(declaredName(stem+"-tier"), "early", []string{"early"})
		lib.Technology(declaredName(stem), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			CostBy: &CostChoices{
				Setting:  tier,
				Choices:  []CostChoice{{Value: "early", Sources: []string{unreadableSource}}},
				Fallback: UnitSpec{Count: 7, Seconds: 8, Packs: []Pack{{Name: pack, Amount: 2}}},
			},
		})
	}

	// THE CYCLE-PREREQUISITE NOTE: a technology anchored After a technology the
	// game has already been made to require it, so the plan's own prerequisite
	// closes the ring and is dropped.
	for i, describe := range []bool{false, true} {
		lib.Technology(declaredName("cycle-prereq-tech-"+strconv.Itoa(i)), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			Unit:        &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{{Name: pack, Amount: 1}}},
			After:       ringAnchor(i),
		})
	}

	// THE CYCLE-SPLICE NOTE: an InsertBetween whose splice closes the ring,
	// because the anchor it hangs off already leads back to the technology it
	// is spliced into.
	for i, describe := range []bool{false, true} {
		lib.Technology(declaredName("cycle-splice-tech-"+strconv.Itoa(i)), TechSpec{
			Description: describedProse(describe, fixtureProse(400)),
			Unit:        &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{{Name: pack, Amount: 1}}},
			After:       spliceAnchor(i),
			Before:      spliceTarget(i),
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

	// THE COMPOSED DESCRIPTION KEY AT EXACTLY THE ELEMENT CEILING, one of each
	// kind. Every other row here is named at fixtureNameBytes, where the
	// composed key is over the ceiling and descriptionRef DROPS it, so without
	// these two the walk would measure the drop twice and a composed key never.
	// The names are the longest whose key fits, which makes the key element the
	// walk reads exactly localisedElementCeiling bytes: see
	// descriptionKeyCeiling for the arithmetic and for why the case is
	// reachable at all.
	keyedRecipe := padName("keyed-recipe", descriptionKeyCeiling("recipe")-len(fixturePrefix))
	keyedResult := lib.Item(declaredName("keyed-recipe-item"), ItemSpec{})
	keyedParts := lib.IngredientsSetting(declaredName("keyed-recipe-setting"),
		[]Ingredient{IngredientNamed(1, item)})
	lib.Recipe(keyedResult, RecipeSpec{Name: keyedRecipe, IngredientsFrom: keyedParts})
	lib.Technology(padName("keyed-tech", descriptionKeyCeiling("technology")-len(fixturePrefix)), TechSpec{
		Unit: &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{{Name: absentPack, Amount: 1}}},
	})

	// The copied unit the DROPPED-PACK note reads names one pack the game has
	// and one it does not; the PACKLESS-SOURCE one names only the pack it
	// does not.
	w := baseWorld().
		withItem(item).
		withFluid(fluid).
		withTool(pack).
		withTech(fixtureTech{name: dropSource, unit: unitOf(200, 30, absentPack, "automation-science-pack")}).
		withTech(fixtureTech{name: packlessSource, unit: unitOf(7, 8, absentPack)}).
		// A unit whose ingredients array holds an entry in NEITHER engine form.
		withTech(fixtureTech{name: unreadableSource, unit: Obj(
			kv("count", Num(7)),
			kv("ingredients", Arr(Str(pack))),
			kv("time", Num(8)),
		)})

	// THE TWO RINGS, one per pair, each closed through an existing technology
	// that the fixture makes require the name this plan is about to emit.
	// TWO SEPARATE ANCHORS PER PAIR, because the walk drops ONE edge per ring
	// and a shared anchor would make the two rows one ring.
	for i := 0; i < 2; i++ {
		w = w.withTech(fixtureTech{
			name:    ringAnchor(i),
			prereqs: []string{fixturePrefix + declaredName("cycle-prereq-tech-"+strconv.Itoa(i))},
			unit:    unitOf(10, 15, pack),
		})
		w = w.withTech(fixtureTech{
			name: spliceTarget(i),
			unit: unitOf(10, 15, pack),
		})
		w = w.withTech(fixtureTech{
			name:    spliceAnchor(i),
			prereqs: []string{spliceTarget(i)},
			unit:    unitOf(10, 15, pack),
		})
	}

	// The two texts the player typed and the library cannot use. Everything
	// else the settings answer is left absent, which is the ordinary
	// unreadable-setting arm and composes nothing.
	for i := 0; i < 2; i++ {
		w = w.withSetting(fixturePrefix+declaredName("fallback-recipe-"+strconv.Itoa(i)+"-setting"),
			Str("1 "+existingName("nothing-is-named-this")))
		w = w.withSetting(fixturePrefix+declaredName("fallback-tech-"+strconv.Itoa(i)+"-packs"),
			Str("1 "+existingName("nothing-is-named-this")))
	}
	w = w.withSetting(fixturePrefix+declaredName("keyed-recipe-setting"),
		Str("1 "+existingName("nothing-is-named-this")))
	return lib, w
}

// The three existing technologies each ring in the fixture is closed through.
// They are functions rather than constants because each pair needs its OWN ring:
// see the loop that builds them.
func ringAnchor(i int) string   { return existingName("ring-anchor-" + strconv.Itoa(i)) }
func spliceAnchor(i int) string { return existingName("splice-anchor-" + strconv.Itoa(i)) }
func spliceTarget(i int) string { return existingName("splice-target-" + strconv.Itoa(i)) }

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
					" so the game loaded as though that setting had been left alone. The reason is in the log." +
					" Changing a recipe ",
				"empties an assembling machine's input slots of anything the new list does not use.",
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

// ---------------------------------------------------------------------------
// THE AUTHOR'S OWN DESCRIPTION SURVIVES A NOTE.
//
// A prototype's own localised_description field WINS OVER the
// [recipe-description] or [technology-description] entry a .cfg defines, so a
// note emitted as {"", "<note>"} DISPLACED the description of every author who
// wrote one the ordinary Factorio way. descriptionRef is the answer: the note
// opens with the author's own key behind an empty alternative, so the engine
// renders their sentence and a newline where they wrote one and nothing where
// they did not.
// ---------------------------------------------------------------------------

// descriptionRefRendered is descriptionRef's wrapper as renderValue prints it.
func descriptionRefRendered(kind, name string) string {
	return `["?", ["", ["` + kind + `-description.` + name + `"], "` + "\n" + `"], ""]`
}

// noteFixture is one plan reaching all four of appendLocalised's cases at once:
// a recipe and a technology each carrying a note, one of each WITH a declared
// Description and one WITHOUT, plus an item that carries no note at all.
//
// THE TWO KINDS ARE BOTH HERE BECAUSE THE SECTION IS THE PROTOTYPE'S OWN. A
// composer that typed one kind in as a constant would satisfy a fixture holding
// only recipes, so the assertions below name recipe-description on a recipe and
// technology-description on a technology and would go red one at a time.
func noteFixture() (*Lib, *fixtureWorld) {
	lib := New()

	// A recipe whose stored ingredient text the language refuses: a note, and
	// no Description of its own.
	bare := lib.Item("bare-rivet", ItemSpec{})
	bareParts := lib.IngredientsSetting("bare-ingredients", []Ingredient{IngredientNamed(1, "iron-plate")})
	lib.Recipe(bare, RecipeSpec{Name: "bare-forging", IngredientsFrom: bareParts})

	// The same recipe WITH a Description, which is the case that must not
	// compose a key: the author's literal already takes the entry's place.
	described := lib.Item("described-rivet", ItemSpec{})
	describedParts := lib.IngredientsSetting("described-ingredients", []Ingredient{IngredientNamed(1, "iron-plate")})
	lib.Recipe(described, RecipeSpec{
		Name:            "described-forging",
		Description:     "Forged from plate.",
		IngredientsFrom: describedParts,
	})

	// A technology the game has no science pack for: a note, and no
	// Description; and its described twin.
	lib.Technology("bare-riveting", TechSpec{
		Unit: &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{{Name: "space-science-pack", Amount: 1}}},
	})
	lib.Technology("described-riveting", TechSpec{
		Description: "Teaches riveting.",
		Unit:        &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{{Name: "space-science-pack", Amount: 1}}},
	})

	// And the two prototypes NOTHING fell back on, which is what proves the
	// unchanged cases are unchanged.
	quiet := lib.Item("quiet-plate", ItemSpec{Description: "An ordinary plate."})
	lib.Recipe(quiet, RecipeSpec{Name: "quiet-forging", Ingredients: []Ingredient{IngredientNamed(1, "iron-plate")}})
	lib.Technology("quiet-research", TechSpec{
		Unit: &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
	})

	w := baseWorld().
		withSetting("steelworks-bare-ingredients", Str("1 unobtanium")).
		withSetting("steelworks-described-ingredients", Str("1 unobtanium"))
	return lib, w
}

// describedProtos is every prototype a plan emits, by emitted name.
func describedProtos(t *testing.T, lib *Lib, w World) map[string]Value {
	t.Helper()
	ops, err := lib.PlanData(w)
	assertNoError(t, err)
	out := map[string]Value{}
	for _, op := range ops {
		if op.Kind != OpExtend {
			continue
		}
		name, ok := field(op.Proto, "name")
		if !ok {
			continue
		}
		out[name.Str] = op.Proto
	}
	return out
}

func descriptionOf(t *testing.T, protos map[string]Value, name string) string {
	t.Helper()
	proto, ok := protos[name]
	if !ok {
		t.Fatalf("the plan emitted no prototype named %s, so nothing below is about it", name)
	}
	desc, ok := field(proto, "localised_description")
	if !ok {
		return ""
	}
	return renderValue(desc)
}

// A NOTE WITH NO DECLARED DESCRIPTION REFERENCES THE AUTHOR'S OWN ENTRY, and
// the section is the PROTOTYPE'S: a recipe composes recipe-description and a
// technology technology-description.
func TestANoteWithNoDescriptionComposesThePrototypesOwnKey(t *testing.T) {
	lib, w := noteFixture()
	protos := describedProtos(t, lib, w)

	for _, c := range []struct{ proto, kind, note string }{
		{
			proto: "steelworks-bare-forging",
			kind:  "recipe",
			note:  fallbackNote("steelworks-bare-ingredients", true),
		},
		{
			proto: "steelworks-bare-riveting",
			kind:  "technology",
			note:  packlessNote(),
		},
	} {
		want := `["", ` + descriptionRefRendered(c.kind, c.proto) + `, ` + chunkedParams(c.note) + `]`
		if got := descriptionOf(t, protos, c.proto); got != want {
			t.Errorf("%s:\n got: %s\nwant: %s", c.proto, got, want)
		}
	}
}

// A DECLARED DESCRIPTION BESIDE A NOTE COMPOSES NO KEY AT ALL, which is what
// says the two cases did not get crossed. The author put their description in
// the plan, so that literal IS their description.
func TestADeclaredDescriptionBesideANoteComposesNoKey(t *testing.T) {
	lib, w := noteFixture()
	protos := describedProtos(t, lib, w)

	for _, c := range []struct{ proto, description, note string }{
		{
			proto:       "steelworks-described-forging",
			description: "Forged from plate.",
			note:        fallbackNote("steelworks-described-ingredients", true),
		},
		{
			proto:       "steelworks-described-riveting",
			description: "Teaches riveting.",
			note:        packlessNote(),
		},
	} {
		want := `["", "` + c.description + `", ` + chunkedParams("\n"+c.note) + `]`
		got := descriptionOf(t, protos, c.proto)
		if got != want {
			t.Errorf("%s:\n got: %s\nwant: %s", c.proto, got, want)
		}
		if strings.Contains(got, "-description.") {
			t.Errorf("%s composed a locale key beside the author's own literal: %s", c.proto, got)
		}
	}
}

// A PROTOTYPE WITH NO NOTE IS WHAT IT ALWAYS WAS, byte for byte, declared
// description or not. A golden taken before this change must not move for a
// load nothing fell back on.
func TestAPrototypeWithNoNoteIsUnchanged(t *testing.T) {
	lib, w := noteFixture()
	protos := describedProtos(t, lib, w)

	// A declared description with no note stays the two-element literal.
	if got, want := descriptionOf(t, protos, "steelworks-quiet-plate"), `["", "An ordinary plate."]`; got != want {
		t.Errorf("an item with a description and no note:\n got: %s\nwant: %s", got, want)
	}
	// And neither with a note nor a description emits the field at all, so the
	// engine resolves the author's own entry exactly as it always did.
	for _, name := range []string{"steelworks-quiet-forging", "steelworks-quiet-research"} {
		proto, ok := protos[name]
		if !ok {
			t.Fatalf("the plan emitted no prototype named %s", name)
		}
		if _, ok := field(proto, "localised_description"); ok {
			t.Errorf("%s emitted a localised_description with neither a description nor a note: %s",
				name, descriptionOf(t, protos, name))
		}
	}
}

// descriptionKeyCeiling is the longest EMITTED prototype name of a kind whose
// composed [<kind>-description] key is exactly localisedElementCeiling bytes.
//
// THE ARITHMETIC IS THE WHOLE POINT OF THIS TEST. A key is ONE element by
// definition and cannot be chunked, the engine polices the key slot at 200
// bytes like every other element, and the engine's own prototype-name ceiling
// is 200 bytes with nothing shorter refused anywhere in this library. So
// `technology-description.` at 23 bytes over a 200-byte name is a 223-byte
// element the engine refuses: the case is REACHABLE, and descriptionRef drops
// the key form above the length below rather than composing a load failure.
func descriptionKeyCeiling(kind string) int {
	return localisedElementCeiling - len(kind+"-description.")
}

// THE KEY FORM IS COMPOSED UP TO THE ELEMENT CEILING AND DROPPED ABOVE IT, one
// byte either side, on both kinds.
func TestTheDescriptionKeyIsDroppedWhereItWouldNotFit(t *testing.T) {
	for _, kind := range []string{"recipe", "technology"} {
		fits := descriptionKeyCeiling(kind)
		for _, c := range []struct {
			what  string
			bytes int
			want  bool
		}{
			{what: "the longest name whose key fits", bytes: fits, want: true},
			{what: "one byte more", bytes: fits + 1, want: false},
		} {
			name := padName("q", c.bytes)
			ref, ok := descriptionRef(kind, name)
			if ok != c.want {
				t.Errorf("%s, %s: a %d-byte name composed=%v, want %v",
					kind, c.what, c.bytes, ok, c.want)
				continue
			}
			if !ok {
				continue
			}
			key := ref.Arr[1].Arr[1].Arr[0].Str
			if len(key) != localisedElementCeiling {
				t.Errorf("%s, %s: the key is %d bytes, want exactly the ceiling of %d",
					kind, c.what, len(key), localisedElementCeiling)
			}
		}
	}
	// AND THE FIXTURE PLAN'S OWN NAMES ARE ABOVE IT, which is what the element
	// walk above measures: at fixtureNameBytes every composed key would be over
	// the ceiling, so the walk sees the drop rather than a refusal.
	for _, kind := range []string{"recipe", "technology"} {
		if fixtureNameBytes <= descriptionKeyCeiling(kind) {
			t.Errorf("a %d-byte %s name composes a key that fits, so worstCasePlan no longer"+
				" reaches the drop arm at all", fixtureNameBytes, kind)
		}
	}
}
