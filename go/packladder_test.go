package fkrecipes

import (
	"strings"
	"testing"
)

// packlessRefusal is the ONE sentence a technology left with no science pack
// earns, composed here so the ten tests that assert it cannot drift apart from
// each other, exactly as noteIn composes the tooltip note.
//
// IT NAMES THE NAMES, which is the whole of what the sentence gained: an author
// reading it is one whose ladders all missed, and the rungs they wrote are the
// one thing that says which mod set this is.
func packlessRefusal(tech string, tried ...string) string {
	return "fkrecipes: the technology " + tech +
		" has no science pack the game has; research takes at least one, and none of " +
		strings.Join(tried, ", ") + " is a science pack here"
}

// THE SCIENCE PACK LADDER. A pack is somebody else's prototype: base's own, an
// overhaul's, or one a modpack renamed. Before this ladder a Unit naming a pack
// the install lacked refused the load with the consumer's name on it, while an
// INGREDIENT in the same position was dropped with a line saying so. These hold
// the two halves of the answer to that: the ladder resolves or drops, and a
// cost left with nothing is refused rather than emitted as a free research.

// toolProbeWorld records every ToolExists question, so a test can assert that a
// question was NOT asked. That is the only way to hold up "the fallback is
// probed only when it is used": the plan is accepted either way, and what
// changes is which questions the World was put to.
type toolProbeWorld struct {
	*fixtureWorld
	asked []string
}

func (w *toolProbeWorld) ToolExists(name string) bool {
	w.asked = append(w.asked, name)
	return w.fixtureWorld.ToolExists(name)
}

func (w *toolProbeWorld) wasAsked(name string) bool {
	for _, n := range w.asked {
		if n == name {
			return true
		}
	}
	return false
}

// The first rung the game has is the one that is emitted, and a ladder that
// resolves says nothing: a log line for every walked rung would bury the drops
// that matter under the ones that are ordinary.
func TestPackLadderTakesTheFirstRungPresent(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs: []Pack{
			{Name: "military-science-pack", Amount: 2, Fallbacks: []string{"chemical-science-pack", "logistic-science-pack"}},
		},
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["chemical-science-pack", 2]]}}`,
	})
}

// THE LADDER ASKS ToolExists AND NOT ItemExists. MEASURED: a research unit
// priced in a plain item refuses the load with "Invalid research unit
// (iron-plate). Research unit(s) can only be tool type items at the moment", so
// an item that is not a tool is not a rung, and a ladder that walked past a
// present tool to reach an item would emit that refusal on the author's behalf.
func TestPackLadderWalksPastAnItemThatIsNotATool(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs:   []Pack{{Name: "iron-plate", Amount: 1, Fallbacks: []string{"automation-science-pack"}}},
	}})

	// iron-plate is an ITEM in this world and never a tool.
	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// A pack no rung resolves is DROPPED with a line in the same shape an
// ingredient drop uses, and the rest of the cost is emitted: a modpack without
// military science still gets the research, priced in what it has.
func TestPackLadderDropsWhatNoRungResolves(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs: []Pack{
			{Name: "automation-science-pack", Amount: 1},
			{Name: "military-science-pack", Amount: 3, Fallbacks: []string{"space-science-pack"}},
			{Name: "logistic-science-pack", Amount: 2},
		},
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axes: none of military-science-pack, space-science-pack is present, so the science pack is dropped`,
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 2]]}}`,
	})
}

// A pack with no ladder at all drops the same way: the single name is the whole
// candidate list, and the line reads as one.
func TestPackWithNoFallbacksDropsWithItsOneName(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs: []Pack{
			{Name: "automation-science-pack", Amount: 1},
			{Name: "military-science-pack", Amount: 3},
		},
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axes: none of military-science-pack is present, so the science pack is dropped`,
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// TWO PACK LADDERS CAN LAND ON ONE TOOL, and a unit that named it twice is
// the same duplicate-ingredient load failure a recipe gets. The amounts add,
// in the position of the first occurrence, and the emitted form is the SHORT
// TUPLE rather than the recipe's dict.
func TestPackLaddersThatLandOnOnePackMerge(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs: []Pack{
			{Name: "automation-science-pack", Amount: 1},
			{Name: "logistic-science-pack", Amount: 4},
			{Name: "military-science-pack", Amount: 2, Fallbacks: []string{"automation-science-pack"}},
		},
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axes: automation-science-pack is in the list twice after the fallbacks, so the amounts are added: 1 plus 2 is 3`,
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 3], ["logistic-science-pack", 4]]}}`,
	})

	// A MERGED PACK IS HELD TO THE ITEM CEILING, and that is the engine's own
	// rule for a unit ingredient rather than an analogy drawn from a recipe.
	// Each of these is legal on its own; their sum is the one pack amount no
	// author wrote, so it is CAPPED with a line and a note rather than refused:
	// which rung the second ladder landed on is the mod set's answer.
	//
	// THE NOTE CARRIES NO DESTRUCTION SENTENCE. Repricing a research empties
	// no assembling machine, which is the same scoping the ERROR lines take.
	over := New()
	over.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs: []Pack{
			{Name: "automation-science-pack", Amount: 40000},
			{Name: "military-science-pack", Amount: 30000, Fallbacks: []string{"automation-science-pack"}},
		},
	}})

	ops, err = over.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axes: automation-science-pack is in the list twice after the fallbacks, so the amounts are added: 40000 plus 30000 is 70000`,
		`log fkrecipes: steel-axes: automation-science-pack is in the list twice after the fallbacks, and 40000 plus 30000 is above the item ceiling of 65535, so it is capped there`,
		`extend {type="technology", name="steelworks-steel-axes", ` +
			`localised_description=["", "Two ingredients resolved onto automation-science-pack and the total was above what one slot holds, so it was capped at 65535. The reason is in the log."], ` +
			`unit={count=50, time=15, ingredients=[["automation-science-pack", 65535]]}}`,
	})
}

// A DECLARED PACK CARRIES THE SAME 16 BITS AS AN ITEM INGREDIENT, and that is
// MEASURED rather than argued. Factorio 2.0.77, build 84539, mac-arm64, steam,
// on a technology whose unit ingredients carry one pack:
//
//	65535           loads, and dumps as written
//	65536           Error while loading technology prototype "..." (technology):
//	2147483648      Value (<n>) outside of range. The data type allows values
//	9007199254740992  from 0 to 65535 in property tree at
//	                ROOT.technology.<name>.unit.ingredients[0][1]
//
// each of the three exit 1 with no dump. A declared pack above the ceiling used
// to be emitted and fail the whole load in the player's game, naming the
// consumer's mod for a number its author wrote, which is exactly the defect the
// ingredient ceiling closed on a recipe.
func TestADeclaredPackAboveTheEnginesCeilingIsRefused(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs:   []Pack{{Name: "automation-science-pack", Amount: 70000}},
	}})

	_, err := lib.PlanData(baseWorld())
	if err == nil {
		t.Fatal("a declared pack above the engine's ceiling was accepted")
	}
	want := "fkrecipes: the technology steel-axes takes 70000 of automation-science-pack, " +
		"and a science pack amount goes up to 65535"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}

	// The boundary itself is legal, and it is the number a merge is allowed to
	// land on.
	at := New()
	at.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs:   []Pack{{Name: "automation-science-pack", Amount: 65535}},
	}})
	ops, err := at.PlanData(baseWorld())
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 65535]]}}`,
	})
}

// A UNIT THAT NAMES ONE PACK TWICE IS THE AUTHOR'S BUG, not a mod set's, and
// it is refused rather than added up: the merge exists for a ladder that
// collapsed onto a name the list already carries, and a list that named it
// twice in the declaration never had a fallback in it.
func TestAUnitNamingOnePackTwiceIsRefused(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs: []Pack{
			{Name: "automation-science-pack", Amount: 1},
			{Name: "logistic-science-pack", Amount: 4},
			{Name: "automation-science-pack", Amount: 2},
		},
	}})

	_, err := lib.PlanData(baseWorld())
	if err == nil {
		t.Fatal("a unit naming one pack twice was accepted")
	}
	want := "fkrecipes: the technology steel-axes names automation-science-pack twice; each science pack is taken once"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}

	// THE SAME RULE ON A COSTBY FALLBACK, because a fallback the engine would
	// refuse is not a fallback: one validateUnit answers for both.
	fb := New()
	tier := fb.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap"})
	fb.Technology("steel-axes", TechSpec{CostBy: &CostChoices{
		Setting: tier,
		Choices: []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
		Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{
			{Name: "automation-science-pack", Amount: 1},
			{Name: "automation-science-pack", Amount: 2},
		}},
	}})

	_, err = fb.PlanData(baseWorld())
	if err == nil {
		t.Fatal("a CostBy fallback naming one pack twice was accepted")
	}
	if err.Error() != want {
		t.Errorf("fallback\n got: %s\nwant: %s", err.Error(), want)
	}
}

// A UNIT THAT LOSES EVERY PACK IS REFUSED, and this is the other side of the
// drop: the engine LOADS a unit with an empty ingredient list (measured), so
// nothing downstream would complain and the player would get a research that
// completes instantly. The refusal names the technology and no rung, because
// the drop lines above it already named every rung that was tried.
func TestUnitWithEveryPackDroppedIsRefused(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs: []Pack{
			{Name: "military-science-pack", Amount: 1},
			{Name: "space-science-pack", Amount: 1, Fallbacks: []string{"metallurgic-science-pack"}},
		},
	}})

	ops, err := lib.PlanData(baseWorld())
	if err == nil {
		t.Fatal("a research priced in nothing the game has was accepted")
	}
	want := packlessRefusal("steel-axes", "military-science-pack", "space-science-pack", "metallurgic-science-pack")
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// THE ALL-DROPPED REFUSAL NAMES THE FIRST TECHNOLOGY IN DECLARATION ORDER.
//
// Two technologies lose every pack here, and one sentence has to come out. The
// names are chosen so that declaration order and alphabetical order disagree:
// a walk that sorted, or that iterated a map, would name aaa-first about half
// the time or every time, and either would be a refusal whose text depends on
// something the author cannot see.
func TestTheAllDroppedRefusalNamesTheFirstTechnologyDeclared(t *testing.T) {
	lib := New()
	packless := &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs:   []Pack{{Name: "military-science-pack", Amount: 1}},
	}
	lib.Technology("bbb-second", TechSpec{Unit: packless})
	lib.Technology("aaa-first", TechSpec{Unit: packless})

	ops, err := lib.PlanData(baseWorld())
	if err == nil {
		t.Fatal("two researches priced in nothing the game has were accepted")
	}
	want := packlessRefusal("bbb-second", "military-science-pack")
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// wrongThreeWaysAfterResolution is a plan that is wrong in all three of the
// ways the post-resolution checks answer for, so the order between them is the
// only thing that decides which sentence comes out:
//
//   - its recipe reads a crafting time from a setting whose declared default is
//     at or below the engine floor,
//   - its technology is priced in a pack the game does not have, so every pack
//     drops,
//   - and the World it is planned against carries a prerequisite ring.
//
// THE CRAFTING TIME IS THE PARAMETER, and it has to be a DECLARED default
// rather than a setting's answer. A value the player's setting answers falls
// back to the declared default with a log line, so the only world the
// crafting-time check still answers for is a default the settings stage would
// have refused; the other two problems are the plan's and stay fixed.
func wrongThreeWaysAfterResolution(craftTimeDefault float64) *Lib {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	from := lib.DoubleSetting("axe-craft-time", craftTimeDefault, NumericSpec{})
	lib.Recipe(axe, RecipeSpec{
		CraftTimeFrom: from,
		Ingredients:   []Ingredient{IngredientNamed(4, "steel-plate")},
	})
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs:   []Pack{{Name: "military-science-pack", Amount: 1}},
	}})
	return lib
}

// worldWithAPrerequisiteRing is that plan's World with a ring in somebody
// else's technologies: logistics-2 requires logistics-3, and logistics-3
// already requires logistics-2.
func worldWithAPrerequisiteRing() *fixtureWorld {
	return baseWorld().withPrereqs("logistics-2", "logistics", "logistics-3")
}

// THE ORDER OF THE POST-RESOLUTION CHECKS IS PART OF THE CONTRACT, because a
// plan can be wrong in more than one of these ways at once and one sentence
// has to come out. The design record fixes it so the two languages answer the
// same declaration the same way: crafting times, then packs, then the cycle
// walk. These two witnesses are what hold that order still, because without
// them the pack check could be moved anywhere between resolution and emit and
// every existing test would stay green.
func TestTheCraftingTimeSentenceBeatsTheDroppedPacks(t *testing.T) {
	// A declared default at the floor, and a setting whose answer is worse
	// still: the answer falls back onto the default and the default is what
	// the check refuses.
	w := worldWithAPrerequisiteRing().withSetting("steelworks-axe-craft-time", Num(0))

	ops, err := wrongThreeWaysAfterResolution(0.001).PlanData(w)
	if err == nil {
		t.Fatal("a plan wrong three ways over was accepted")
	}
	want := withFallbackFact("fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, "+
		"whose declared default is at or below the engine floor (energy_required can't be <= 0.001)",
		"steelworks-axe-craft-time")
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// And the packs beat the ring, which is the other side of the same order and
// the half a check moved one line further down would break.
func TestTheDroppedPacksSentenceBeatsThePrerequisiteRing(t *testing.T) {
	// The same plan and the same ring, with a crafting-time default the engine
	// takes and the setting left unanswered, so 2.5 stands.
	ops, err := wrongThreeWaysAfterResolution(2.5).PlanData(worldWithAPrerequisiteRing())
	if err == nil {
		t.Fatal("a plan with an unpayable cost and a prerequisite ring was accepted")
	}
	want := packlessRefusal("steel-axes", "military-science-pack")
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// The ring really is there, in both worlds above: a witness that the pack
// sentence beats a cycle is worth nothing if the World it was planned against
// has no cycle in it. The plan here is priced in a pack the game HAS, so the
// pack check passes and the walk is what answers.
func TestTheRingInThatWorldIsReal(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   50,
		Seconds: 15,
		Packs:   []Pack{{Name: "automation-science-pack", Amount: 1}},
	}})

	_, err := lib.PlanData(worldWithAPrerequisiteRing())
	if err == nil {
		t.Fatal("that World accepted a plan; it carries no ring")
	}
	want := "fkrecipes: a prerequisite cycle: logistics-2 -> logistics-3 -> logistics-2"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}

// A unit that DECLARES no packs is refused too, and by a DIFFERENT sentence:
// this one is about what the author wrote, not about what the game turned out
// to have, and the two have different answers. The engine loads such a unit
// (measured), so nobody downstream would say anything and the player would get
// a research that completes the moment it is started.
func TestUnitDeclaredWithNoPacksIsRefused(t *testing.T) {
	t.Run("a hand-rolled unit", func(t *testing.T) {
		lib := New()
		lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{Count: 50, Seconds: 15}})

		ops, err := lib.PlanData(baseWorld())
		if err == nil {
			t.Fatal("a research priced in nothing at all was accepted")
		}
		want := "fkrecipes: the technology steel-axes declares no science pack; research takes at least one"
		if err.Error() != want {
			t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
		}
		if ops != nil {
			t.Errorf("a refused plan still produced %d ops", len(ops))
		}
	})

	// THE FALLBACK IS REFUSED WITHOUT BEING REACHED, which is what places this
	// rule at plan validation rather than after resolution: steel-processing
	// answers, so the fallback is the cost nothing uses, and its numbers are
	// still the author's to get right. The pack ladder is the opposite case and
	// is probed only when the fallback applies.
	t.Run("a fallback nothing reaches", func(t *testing.T) {
		lib := New()
		tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
		lib.Technology("hardened-tips", TechSpec{
			CostBy: &CostChoices{
				Setting:  tier,
				Choices:  []CostChoice{{Value: "logistics", Sources: []string{"steel-processing"}}},
				Fallback: UnitSpec{Count: 60, Seconds: 30},
			},
		})

		ops, err := lib.PlanData(baseWorld())
		if err == nil {
			t.Fatal("a fallback priced in nothing at all was accepted")
		}
		want := "fkrecipes: the technology hardened-tips declares no science pack; research takes at least one"
		if err.Error() != want {
			t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
		}
		if ops != nil {
			t.Errorf("a refused plan still produced %d ops", len(ops))
		}
	})
}

func fallbackTierPlan(fallbackPack string, sources []string) *Lib {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: []CostChoice{{Value: "logistics", Sources: sources}},
			Fallback: UnitSpec{
				Count:   60,
				Seconds: 30,
				Packs:   []Pack{{Name: fallbackPack, Amount: 1}},
			},
		},
	})
	return lib
}

// THE FALLBACK IS PROBED ONLY WHEN IT IS USED, which is the pilot's finding
// answered: a cost that never applies used to have its science pack presence
// checked anyway, so a mod could not name a pack from an optional dependency in
// a fallback without every install without that dependency refusing to load.
//
// The witness is the QUESTION, not the outcome: the plan is accepted either
// way, and what this holds up is that the World was never asked about a pack in
// a cost nothing reaches.
func TestFallbackPacksAreProbedOnlyWhenTheFallbackApplies(t *testing.T) {
	t.Run("a source answers, so the fallback is never asked about", func(t *testing.T) {
		w := &toolProbeWorld{fixtureWorld: baseWorld()}
		// steel-processing carries a unit, so the ladder settles there and the
		// fallback is unreachable. Its pack is one this world does not have.
		ops, err := fallbackTierPlan("space-science-pack", []string{"steel-processing"}).PlanData(w)
		assertNoError(t, err)

		assertLines(t, transcript(ops), []string{
			`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
			`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["steel-processing"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}`,
		})
		if w.wasAsked("space-science-pack") {
			t.Errorf("the World was asked about a pack in a fallback nothing reaches; questions asked: %v", w.asked)
		}
	})

	t.Run("no source answers, so the fallback is resolved", func(t *testing.T) {
		w := &toolProbeWorld{fixtureWorld: baseWorld()}
		// No sources at all, so the fallback is what applies and its pack IS
		// probed. It resolves here, which is what keeps this arm about the
		// probe rather than about the refusal.
		ops, err := fallbackTierPlan("logistic-science-pack", nil).PlanData(w)
		assertNoError(t, err)

		assertLines(t, transcript(ops), []string{
			`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
			`log fkrecipes: hardened-tips: no source for the logistics cost carries a unit, so the fallback cost applies and the technology has no prerequisite`,
			`extend {type="technology", name="steelworks-hardened-tips", unit={count=60, time=30, ingredients=[["logistic-science-pack", 1]]}}`,
		})
		if !w.wasAsked("logistic-science-pack") {
			t.Errorf("the World was never asked about the pack of the fallback that applied; questions asked: %v", w.asked)
		}
	})
}

// The fallback's drop line comes AFTER the line saying why the fallback
// applies, because that is the order the planner decided them in and the log
// stream is the two halves' comparison surface.
func TestFallbackPackDropsInLogOrder(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: []CostChoice{{Value: "logistics"}},
			Fallback: UnitSpec{
				Count:   60,
				Seconds: 30,
				Packs: []Pack{
					{Name: "automation-science-pack", Amount: 1},
					{Name: "military-science-pack", Amount: 2, Fallbacks: []string{"space-science-pack"}},
				},
			},
		},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
		`log fkrecipes: hardened-tips: no source for the logistics cost carries a unit, so the fallback cost applies and the technology has no prerequisite`,
		`log fkrecipes: hardened-tips: none of military-science-pack, space-science-pack is present, so the science pack is dropped`,
		`extend {type="technology", name="steelworks-hardened-tips", unit={count=60, time=30, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// A pack's ladder is DEEP COPIED at declaration, like every other slice a
// consumer hands in: a caller reusing one buffer across two technologies must
// not find the first one rewritten by the second.
func TestPackLaddersDoNotAliasCallerSlices(t *testing.T) {
	lib := New()
	rungs := []string{"military-science-pack", "automation-science-pack"}
	packs := []Pack{{Name: "space-science-pack", Amount: 1, Fallbacks: rungs}}
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{Count: 50, Seconds: 15, Packs: packs}})
	// The caller reuses the buffer, both the row and the ladder inside it.
	rungs[1] = "logistic-science-pack"
	packs[0].Name = "chemical-science-pack"

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// A DECLARED CHOICES LIST IS A SNAPSHOT, so a caller reusing its struct after
// the declaration cannot rewrite the plan. The dropdown's option list is
// exactly what the author wrote and this library adds nothing to it, so what
// has to survive the copy is the author's own values.
func TestDeclaredChoicesAreSnapshotted(t *testing.T) {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "dry", []string{"dry", "wet"})
	choices := &IngredientChoices{
		Setting: medium,
		Choices: []IngredientChoice{{Value: "dry"}, {Value: "wet"}},
	}
	lib.Recipe(plate, RecipeSpec{IngredientsBy: choices})

	tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
	cost := &CostChoices{
		Setting:  tier,
		Choices:  []CostChoice{{Value: "logistics", Sources: []string{"steel-processing"}}},
		Fallback: UnitSpec{Count: 60, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
	}
	lib.Technology("hardened-tips", TechSpec{CostBy: cost})

	// The declaration took a copy, so the caller's later edit cannot reach it.
	choices.Choices[0].Value = "rewritten"
	cost.Choices[0].Value = "rewritten"

	if got := lib.recipes[0].spec.IngredientsBy.Choices[0].Value; got != "dry" {
		t.Errorf("IngredientChoices.Choices[0].Value is %q, want dry", got)
	}
	if got := lib.techs[0].spec.CostBy.Choices[0].Value; got != "logistics" {
		t.Errorf("CostChoices.Choices[0].Value is %q, want logistics", got)
	}

	// AND THE PLAN IS STILL THE ONE THE AUTHOR DECLARED: a rewritten value
	// would no longer cover the dropdown's allowed values and would be refused.
	if _, err := lib.PlanData(baseWorld()); err != nil {
		t.Errorf("a snapshotted plan was refused: %s", err)
	}
}

// ---------------------------------------------------------------------------
// A COPIED RESEARCH UNIT'S PACKS, which are the ones no ladder ever guarded.
// ---------------------------------------------------------------------------

// A COPIED UNIT USED TO BE HANDED TO THE ENGINE UNFILTERED, and the engine is
// not forgiving about it. MEASURED on 2.0.77 build 84539, headless: a pack a
// modpack demoted from tool to item refuses the whole load with
// `Invalid research unit (iron-plate). Research unit(s) can only be tool type
// items at the moment.`, and a name the game does not have at all fails
// earlier and more coarsely, with `Error in assignID: item with name 'water'
// does not exist.` Neither names this mod, this setting or the technology in
// the second case, and both fire on the DEFAULT setting. So the copy is
// filtered through the same ToolExists probe the ladder uses.
func TestACopiedUnitDropsAPackTheGameDoesNotHave(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "logistics-2"})

	ops, err := lib.PlanData(baseWorld().withoutTool("logistic-science-pack"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axes: logistic-science-pack is not a science pack this game has, so it is left out of the logistics-2 cost`,
		`extend {type="technology", name="steelworks-steel-axes", ` +
			`localised_description=["", "This game has no logistic-science-pack, so this research was priced without it. The reason is in the log."], ` +
			`unit={count=200, ingredients=[["automation-science-pack", 1]], time=30}}`,
	})
}

// THE LONG FORM TOO, because a unit this library copies is somebody else's
// declaration and the engine takes both spellings. Only the NAME is read; the
// entry itself is what is kept, so an amount, a quality and any field no
// version of this library has heard of ride through the filter untouched.
func TestACopiedUnitInTheLongIngredientFormIsFiltered(t *testing.T) {
	long := Obj(
		kv("count", Num(10)),
		kv("ingredients", Arr(
			Obj(kv("name", Str("military-science-pack")), kv("amount", Num(3))),
			Obj(kv("name", Str("automation-science-pack")), kv("amount", Num(2)), kv("quality", Str("legendary"))),
		)),
		kv("time", Num(15)),
	)

	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})

	ops, err := lib.PlanData(baseWorld().withUnit("steel-processing", long))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axes: military-science-pack is not a science pack this game has, so it is left out of the steel-processing cost`,
		`extend {type="technology", name="steelworks-steel-axes", ` +
			`localised_description=["", "This game has no military-science-pack, so this research was priced without it. The reason is in the log."], ` +
			`unit={count=10, ingredients=[{name="automation-science-pack", amount=2, quality="legendary"}], time=15}}`,
	})
}

// A PACK NAME THIS HALF CANNOT PUT TO THE WORLD IS KEPT UNASKED, and this is
// the ONE DELIBERATE DIVERGENCE-AVOIDANCE in the filter.
//
// Another mod's science pack can be named with bytes that are not UTF-8; fkdata
// hands them over unchanged and a copied unit has always crossed byte-exact.
// Rust's Value has a Bytes variant those arrive in and its tool_exists takes a
// &str, so that half CANNOT ask about such a name; this half's Value holds a Go
// string, which can carry any bytes at all, so it COULD ask and deliberately
// does not. Without askableName this half would ask, get false, drop the pack
// and write a drop line the other half never writes, and the mirror would part
// over a name neither half can spell.
//
// SO THE PROPERTY IS BOTH HALVES OF IT: the entry survives with its bytes
// intact, and no line is written about it. The Rust twin is
// a_copied_unit_carries_a_pack_name_that_is_not_text.
func TestACopiedUnitKeepsAPackNameThatIsNotText(t *testing.T) {
	const pack = "othermod-p\xffck"
	unit := Obj(
		kv("count", Num(200)),
		kv("ingredients", Arr(Arr(Str(pack), Num(1)))),
		kv("time", Num(30)),
	)

	lib := New()
	lib.Technology("hardened-tips", TechSpec{CostOf: "steel-processing"})

	ops, err := lib.PlanData(baseWorld().withUnit("steel-processing", unit))
	assertNoError(t, err)

	// NOT ONE LINE, which is the half a rendering cannot show: a dropped pack
	// writes a drop line, and a pack this half declined to ask about writes
	// nothing at all.
	for _, op := range ops {
		if op.Kind == OpLog {
			t.Errorf("a pack name this half cannot ask about wrote a line: %s", op.Line)
		}
	}

	// AND THE BYTES ARE THE PROPERTY, not the rendering: the op stream is
	// walked and the name compared byte for byte, because a transcript could
	// agree with itself while the value carried something else.
	found := 0
	for _, op := range ops {
		if op.Kind != OpExtend {
			continue
		}
		u, ok := field(op.Proto, "unit")
		if !ok {
			t.Fatal("the technology carries no unit")
		}
		ings, ok := field(u, "ingredients")
		if !ok || ings.Kind != KindArr {
			t.Fatal("the unit carries no ingredient array")
		}
		for _, e := range ings.Arr {
			if e.Kind != KindArr || len(e.Arr) < 1 || e.Arr[0].Kind != KindStr {
				t.Fatalf("the copied entry is not the short tuple form any more: %v", e.Kind)
			}
			if e.Arr[0].Str != pack {
				t.Errorf("\n got: %q\nwant: %q", e.Arr[0].Str, pack)
			}
			found++
		}
	}
	if found != 1 {
		t.Errorf("the copied unit emitted %d pack entries, want 1", found)
	}
}

// AN ENTRY IN NEITHER FORM IS REFUSED RATHER THAN PASSED THROUGH. A form this
// library cannot decode is not a licence to hand it to the engine: the name
// inside it might be one the game does not have, and the refusal that earns
// names neither the technology nor the property. It is the CostOf family's own
// phrase, because it is the same fact about the same value.
func TestACopiedUnitWhosePackListIsInNeitherFormIsRefused(t *testing.T) {
	odd := Obj(
		kv("count", Num(10)),
		kv("ingredients", Arr(Str("automation-science-pack"))),
		kv("time", Num(15)),
	)

	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})

	ops, err := lib.PlanData(baseWorld().withUnit("steel-processing", odd))
	if err == nil {
		t.Fatal("a copied unit whose pack list is in neither engine form was accepted")
	}
	want := "fkrecipes: steel-axes: the unit of steel-processing holds a table this library cannot copy faithfully"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}

	// AND THE LIST ITSELF IN NEITHER FORM, which is the same rule one level up:
	// an ingredients key that is not an array at all cannot be walked, so it is
	// refused rather than crossed unfiltered. Passing it through would hand the
	// engine a pack list this library never read, and the refusal that earns
	// names neither the technology nor the property.
	notAList := Obj(
		kv("count", Num(10)),
		kv("ingredients", Str("automation-science-pack")),
		kv("time", Num(15)),
	)

	ops, err = copyingSteelProcessing().PlanData(baseWorld().withUnit("steel-processing", notAList))
	if err == nil {
		t.Fatal("a copied unit whose ingredients key is not a list was accepted")
	}
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// copyingSteelProcessing is the one-technology plan the copied-unit refusals
// are read through, built fresh per world so no answer can ride from one to
// the next.
func copyingSteelProcessing() *Lib {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
	return lib
}

// A COPIED COST HAS NOTHING TO FALL BACK ON, which is the whole difference
// between it and a tier: CostOf is a bare string with no declared ladder behind
// it, so a copy that keeps no pack is refused by name rather than degraded onto
// a cost this library would have to invent. The names are the ones it tried.
func TestACopiedUnitThatLosesEveryPackIsRefusedByName(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})

	ops, err := lib.PlanData(baseWorld().withoutTool("automation-science-pack"))
	if err == nil {
		t.Fatal("a copied cost priced in nothing the game has was accepted")
	}
	want := packlessRefusal("steel-axes", "automation-science-pack")
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// A COPIED UNIT THAT NAMED NO PACK TO BEGIN WITH IS SOMEBODY ELSE'S
// DECLARATION AND IS LEFT ALONE. Nothing dropped, so nothing degraded: the
// packless rule is about what this mod set took away, not about what the source
// technology was priced in, which is the same line the tier's own packs are on.
func TestACopiedUnitThatNamedNoPackAtAllIsCopiedAsItIs(t *testing.T) {
	empty := Obj(
		kv("count", Num(10)),
		kv("ingredients", Arr()),
		kv("time", Num(15)),
	)

	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})

	ops, err := lib.PlanData(baseWorld().withUnit("steel-processing", empty))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=10, ingredients=[], time=15}}`,
	})
}

// A TIER'S CHOSEN SOURCE IS COPIED THE SAME WAY, and the drop line names the
// source the tier landed on rather than a CostOf that is not there.
func TestATierWhoseCopiedUnitDropsOnePackKeepsTheRest(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tier", "mid", []string{"mid"})
	lib.Technology("steel-axes", TechSpec{CostBy: &CostChoices{
		Setting:  tier,
		Choices:  []CostChoice{{Value: "mid", Sources: []string{"logistics-2"}}},
		Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
	}})

	ops, err := lib.PlanData(baseWorld().withoutTool("logistic-science-pack"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tier was not readable, so its default applies`,
		`log fkrecipes: steel-axes: logistic-science-pack is not a science pack this game has, so it is left out of the logistics-2 cost`,
		`extend {type="technology", name="steelworks-steel-axes", ` +
			`localised_description=["", "This game has no logistic-science-pack, so this research was priced without it. The reason is in the log."], ` +
			`prerequisites=["logistics-2"], unit={count=200, ingredients=[["automation-science-pack", 1]], time=30}}`,
	})
}

// AND A TIER THAT LOSES EVERY PACK DEGRADES INSTEAD OF REFUSING, because there
// IS a declared cost behind it: the author's own Fallback unit, resolved through
// the same ladder so its own absent rungs drop the same way.
//
// THE ERROR LINE IS NOT A PLAYER'S FALLBACK, and it carries no route to the
// settings screen: nothing was typed, so there is no field to send anybody to.
//
// THE PREREQUISITE AND THE LEVEL CAP STAY. The tier still chose this rung; only
// the price moved.
func TestATierWhoseCopiedUnitLosesEveryPackTakesTheDeclaredFallback(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tier", "early", []string{"early"})
	lib.Technology("steel-axes", TechSpec{CostBy: &CostChoices{
		Setting:  tier,
		Choices:  []CostChoice{{Value: "early", Sources: []string{"steel-processing"}}},
		Fallback: UnitSpec{Count: 7, Seconds: 8, Packs: []Pack{{Name: "chemical-science-pack", Amount: 2}}},
	}})

	ops, err := lib.PlanData(baseWorld().withoutTool("automation-science-pack"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tier was not readable, so its default applies`,
		`log fkrecipes: steel-axes: automation-science-pack is not a science pack this game has, so it is left out of the steel-processing cost`,
		`log fkrecipes: ERROR: steel-axes: the steel-processing cost names no science pack this game has, so this mod's own declared cost applies instead`,
		`extend {type="technology", name="steelworks-steel-axes", ` +
			`localised_description=["", "This game has none of the science packs the steel-processing cost names, so this mod's own declared cost applies. The reason is in the log."], ` +
			`prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["chemical-science-pack", 2]]}}`,
	})
}

// AND WHEN THE PLAYER HAS TYPED A PACK LIST, THE TIER'S SENTENCE IS TAKEN BACK,
// because it is not true any more.
//
// THE SHAPE IS THE EXAMPLE GUEST'S OWN: a technology declaring CostBy and
// CostFrom together, on a mod set where the tier's source loses every pack. The
// tier arm writes the ERROR line and the tooltip note BEFORE the custom cost is
// read, so without the retraction the technology says "this mod's own declared
// cost applies instead" while the packs, and the count and the seconds beside
// them, are the player's. A false statement in a tooltip is worse than none.
//
// THE DROP LINE STAYS, because it is still true: that pack really is absent.
func TestATypedPackListTakesBackTheTiersPacklessSentence(t *testing.T) {
	plan := func() *Lib {
		lib := New()
		tier := lib.DropdownSettingNeedingLocale("tier", "early", []string{"early"})
		packs := lib.PacksSetting("axe-packs", []Pack{{Name: "chemical-science-pack", Amount: 2}})
		count := lib.IntSetting("axe-count", 0, Between(0, 1000))
		seconds := lib.IntSetting("axe-seconds", 0, Between(0, 600))
		lib.Technology("steel-axes", TechSpec{
			CostBy: &CostChoices{
				Setting:  tier,
				Choices:  []CostChoice{{Value: "early", Sources: []string{"steel-processing"}}},
				Fallback: UnitSpec{Count: 7, Seconds: 8, Packs: []Pack{{Name: "chemical-science-pack", Amount: 2}}},
			},
			CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
		})
		return lib
	}

	ops, err := plan().PlanData(baseWorld().withoutTool("automation-science-pack").
		withSetting("steelworks-axe-packs", Str("3 logistic-science-pack")))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tier was not readable, so its default applies`,
		`log fkrecipes: steel-axes: automation-science-pack is not a science pack this game has, so it is left out of the steel-processing cost`,
		`log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies`,
		`log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies`,
		`log fkrecipes: steelworks-steel-axes takes its research cost from steelworks-axe-packs: count 7, time 8, packs 3 logistic-science-pack; the steelworks-tier choice early supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-steel-axes", prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["logistic-science-pack", 3]]}}`,
	})

	// AND THE SAME PLAN WITH THE FIELD LEFT ALONE KEEPS BOTH, which is what
	// scopes the retraction to the case that made them false: here the mod's
	// own declared cost really is what applies.
	ops, err = plan().PlanData(baseWorld().withoutTool("automation-science-pack"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tier was not readable, so its default applies`,
		`log fkrecipes: steel-axes: automation-science-pack is not a science pack this game has, so it is left out of the steel-processing cost`,
		`log fkrecipes: ERROR: steel-axes: the steel-processing cost names no science pack this game has, so this mod's own declared cost applies instead`,
		`log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies`,
		`log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies`,
		`log fkrecipes: the setting steelworks-axe-packs was not readable, so its default applies`,
		`extend {type="technology", name="steelworks-steel-axes", ` +
			`localised_description=["", "This game has none of the science packs the steel-processing cost names, so this mod's own declared cost applies. The reason is in the log."], ` +
			`prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["chemical-science-pack", 2]]}}`,
	})
}

// AND WHEN THE DECLARED FALLBACK IS ALSO UNPAYABLE THE REFUSAL STAYS, naming
// every rung the walk asked about: the copied pack first, then the fallback's
// own ladder, in the order they were asked.
func TestATierWhoseFallbackIsAlsoUnpayableStillRefuses(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tier", "early", []string{"early"})
	lib.Technology("steel-axes", TechSpec{CostBy: &CostChoices{
		Setting: tier,
		Choices: []CostChoice{{Value: "early", Sources: []string{"steel-processing"}}},
		Fallback: UnitSpec{Count: 7, Seconds: 8, Packs: []Pack{
			{Name: "military-science-pack", Amount: 2, Fallbacks: []string{"space-science-pack"}},
		}},
	}})

	_, err := lib.PlanData(baseWorld().withoutTool("automation-science-pack"))
	if err == nil {
		t.Fatal("a tier whose fallback is also unpayable was accepted")
	}
	want := packlessRefusal("steel-axes", "automation-science-pack", "military-science-pack", "space-science-pack")
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}
