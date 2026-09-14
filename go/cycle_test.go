package fkrecipes

import (
	"strings"
	"testing"
)

// The engine's own answer to a cycle is eight words with no name and no path.
// These are the tests that earn the library's answer.

func axePlan() *Lib {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{Icon: "__steelworks__/graphics/icons/steel-axe.png"})
	rec := lib.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(4, "steel-plate")}})
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "steel-processing", Unlocks: []RecipeRef{rec}})
	return lib
}

func TestCycleDirect(t *testing.T) {
	// Somebody's overhaul made two belt technologies require each other.
	w := baseWorld().withPrereqs("logistics-2", "logistics", "logistics-3")

	_, err := axePlan().PlanData(w)
	if err == nil {
		t.Fatal("the plan was accepted, want a cycle refusal")
	}
	want := "fkrecipes: a prerequisite cycle: logistics-2 -> logistics-3 -> logistics-2"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}

func TestCycleTransitiveThroughExistingEdges(t *testing.T) {
	// Three hops, none of them ours, and the path names all of them.
	w := baseWorld().withPrereqs("logistics", "logistics-3")

	_, err := axePlan().PlanData(w)
	if err == nil {
		t.Fatal("the plan was accepted, want a cycle refusal")
	}
	want := "fkrecipes: a prerequisite cycle: logistics -> logistics-3 -> logistics-2 -> logistics"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}

// The splice is the interesting one: the tree is sound until the plan's own
// rewrite closes the ring, which is exactly what no other tool catches.
//
// AND THE SPLICE IS WHAT IS DROPPED, not the load. The ring holds an edge this
// plan made, so the game keeps playing: steel-processing is left with the
// prerequisite list it already had, the new technology is still emitted and
// still requires logistics-3, and its own tooltip says what it lost.
func TestCycleCreatedByInsertBetweenDropsTheSplice(t *testing.T) {
	// logistics-3 -> logistics-2 -> steel-processing, and nothing points back:
	// a sound tree.
	w := baseWorld().withPrereqs("logistics-2", "logistics", "steel-processing")

	lib := New()
	// Splicing between logistics-3 and steel-processing makes
	// steel-processing require the new technology, which requires logistics-3,
	// which already leads back to steel-processing.
	lib.Technology("steel-axes", TechSpec{CostOf: "electronics", After: "logistics-3", Before: "steel-processing"})

	ops, err := lib.PlanData(w)
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axes: steel-processing does not require logistics-3, so the new technology is appended to its prerequisites`,
		`log fkrecipes: ERROR: steel-axes: making it a prerequisite of steel-processing would loop ` +
			`this game's technology tree (logistics-2 -> steel-processing -> steelworks-steel-axes -> ` +
			`logistics-3 -> logistics-2), so the splice is dropped`,
		`extend {type="technology", name="steelworks-steel-axes", ` +
			`localised_description=["", ` + descriptionRefIn("technology", "steelworks-steel-axes") + `, "Making this research a prerequisite of steel-processing would loop ` +
			`this game's technology tree, so it was left out of it. The reason is in the log."], ` +
			`prerequisites=["logistics-3"], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}`,
	})
}

// The overlay must not invent a cycle out of a healthy splice: the edge the
// rewrite REMOVES is gone from the walk, not just the ones it adds.
func TestNoCycleForAHealthySplice(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", Before: "logistics-3"})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}`,
		`set technology.logistics-3.prerequisites = ["steelworks-steel-axes"]`,
	})
}

// The blind spot the overwrite refusal closes: a planned name that already
// exists put a SECOND node of that name in the overlay, the incoming edges
// bound to the stale one, and the walk stepped past the ring.
func TestOverwriteRefusalClosesTheCycleBlindSpot(t *testing.T) {
	// data.raw already carries a steelworks-widgetry that steel-processing
	// requires; the plan declares widgetry and splices it in front of
	// steel-processing, which closes a ring through the stale node.
	w := baseWorld().
		withTech(fixtureTech{name: "steelworks-widgetry", prereqs: []string{"logistics-3"}, unit: unitOf(50, 15, "automation-science-pack")}).
		withPrereqs("steel-processing", "steelworks-widgetry")

	lib := New()
	lib.Technology("widgetry", TechSpec{CostOf: "electronics", After: "logistics-3", Before: "steel-processing"})

	_, err := lib.PlanData(w)
	if err == nil {
		t.Fatal("the plan was accepted, want an overwrite refusal")
	}
	want := "fkrecipes: the technology steelworks-widgetry already exists in data.raw; this plan would overwrite it"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}

func ringTechName(i int) string {
	digits := []byte{byte('0' + i/100%10), byte('0' + i/10%10), byte('0' + i%10)}
	return "conveyor-tier-" + string(digits)
}

// A ring of technologies, sorted by name, each requiring the next.
func ringWorld(n int) *fixtureWorld {
	w := &fixtureWorld{
		modName: "steelworks",
		items:   []string{"automation-science-pack"},
		// A science pack is a TOOL as well as an item, and the pack ladder asks
		// the tool question: a fixture that named it only as an item would drop
		// the pack and refuse the cost before the cycle walk ever ran.
		tools: []string{"automation-science-pack"},
	}
	for i := 0; i < n; i++ {
		w.techs = append(w.techs, fixtureTech{
			name:    ringTechName(i),
			prereqs: []string{ringTechName((i + 1) % n)},
			unit:    unitOf(10, 5, "automation-science-pack"),
		})
	}
	return w
}

// A pathological tree can ring thousands of technologies together. The
// refusal names the first hundred and says how many it did not name, so the
// message stays something a person can read.
func TestCyclePathIsCapped(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{Count: 50, Seconds: 15, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}}})

	_, err := lib.PlanData(ringWorld(150))
	if err == nil {
		t.Fatal("the plan was accepted, want a cycle refusal")
	}

	named := make([]string, 0, cyclePathCap)
	for i := 0; i < cyclePathCap; i++ {
		named = append(named, ringTechName(i))
	}
	// 150 technologies on the stack plus the one that closes the ring, less
	// the hundred the message names.
	want := "fkrecipes: a prerequisite cycle: " + strings.Join(named, " -> ") + " -> (and 51 more before it closes)"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}

// THE WALK IS A LOOP AND NOT ONE PASS, because dropping one edge can leave a
// second ring standing. Two technologies, two rings, two drops, and the plan
// loads: termination is by construction, since every pass drops one edge this
// plan made and the plan has finitely many.
func TestTwoRingsAreBothResolved(t *testing.T) {
	lib := New()
	lib.Technology("aaa", TechSpec{CostOf: "electronics", After: "logistics-2"})
	lib.Technology("bbb", TechSpec{CostOf: "electronics", After: "logistics-3"})
	w := baseWorld().
		withPrereqs("logistics-2", "steelworks-aaa").
		withPrereqs("logistics-3", "steelworks-bbb")

	ops, err := lib.PlanData(w)
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: aaa: requiring logistics-2 would loop this game's technology tree ` +
			`(logistics-2 -> steelworks-aaa -> logistics-2), so the prerequisite is dropped`,
		`log fkrecipes: ERROR: bbb: requiring logistics-3 would loop this game's technology tree ` +
			`(logistics-3 -> steelworks-bbb -> logistics-3), so the prerequisite is dropped`,
		`extend {type="technology", name="steelworks-aaa", ` +
			`localised_description=["", ` + descriptionRefIn("technology", "steelworks-aaa") + `, "Requiring logistics-2 would loop this game's technology tree, ` +
			`so this research was left without that prerequisite. The reason is in the log."], ` +
			`unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}`,
		`extend {type="technology", name="steelworks-bbb", ` +
			`localised_description=["", ` + descriptionRefIn("technology", "steelworks-bbb") + `, "Requiring logistics-3 would loop this game's technology tree, ` +
			`so this research was left without that prerequisite. The reason is in the log."], ` +
			`unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}`,
	})
}

// A DROPPED SPLICE MUST NOT TAKE THE BASE GAME'S OWN EDGE WITH IT, which is
// the one thing InsertBetween's rewrite makes easy to get wrong: a splice
// REPLACES the technology it was inserted after, and a SECOND splice into the
// same target builds its record on a list the anchor is already out of. Taking
// the dropped name back out of that later record without putting the anchor
// back emits a prerequisite list with an edge of somebody else's tree silently
// missing from it.
//
// THE SHAPE, and every piece of it is load-bearing. automation requires
// electronics in the base tree. riveting splices BETWEEN them, so automation's
// list becomes [riveting] and the anchor electronics is out of it. plating
// splices into automation too and finds nothing to replace, so its record is
// [riveting, plating]. forging splices into electronics and requires
// automation, which is what closes the ring the first drop is about.
//
// WHAT THE WALK DOES, in order: the ring automation -> riveting -> electronics
// -> forging -> automation is found first and riveting's splice is the first
// edge in it this plan owns, so it goes and electronics goes back into BOTH
// records; the ring that is left, automation -> electronics -> forging ->
// automation, costs forging's splice; and what is emitted is plating's record,
// which must read [electronics, steelworks-plating]. That is exactly the list a
// plan with neither dropped splice in it would have produced.
func TestADroppedSpliceGivesTheAnchorBackToTheRecordsBuiltOnIt(t *testing.T) {
	lib := New()
	lib.Technology("riveting", TechSpec{CostOf: "electronics", After: "electronics", Before: "automation"})
	lib.Technology("forging", TechSpec{CostOf: "electronics", After: "automation", Before: "electronics"})
	lib.Technology("plating", TechSpec{CostOf: "electronics", After: "logistics", Before: "automation"})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`log fkrecipes: forging: electronics does not require automation, so the new technology is appended to its prerequisites`,
		`log fkrecipes: plating: automation does not require logistics, so the new technology is appended to its prerequisites`,
		`log fkrecipes: ERROR: riveting: making it a prerequisite of automation would loop this game's technology tree ` +
			`(automation -> steelworks-riveting -> electronics -> steelworks-forging -> automation), so the splice is dropped`,
		`log fkrecipes: ERROR: forging: making it a prerequisite of electronics would loop this game's technology tree ` +
			`(automation -> electronics -> steelworks-forging -> automation), so the splice is dropped`,
		`extend {type="technology", name="steelworks-riveting", ` +
			`localised_description=["", ` + descriptionRefIn("technology", "steelworks-riveting") + `, "Making this research a prerequisite of automation would loop this game's ` +
			`technology tree, so it was left out of it. The reason is in the log."], ` +
			`prerequisites=["electronics"], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}`,
		`extend {type="technology", name="steelworks-forging", ` +
			`localised_description=["", ` + descriptionRefIn("technology", "steelworks-forging") + `, "Making this research a prerequisite of electronics would loop this game's ` +
			`technology tree, so it was left out of it. The reason is in the log."], ` +
			`prerequisites=["automation"], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}`,
		`extend {type="technology", name="steelworks-plating", prerequisites=["logistics"], ` +
			`unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}`,
		// THE ASSERTION THE WHOLE TEST IS FOR. electronics is a prerequisite of
		// automation in the base game and no declaration here asked for it to
		// go; a drop that deleted the dropped name instead of substituting the
		// anchor emits ["steelworks-plating"] here.
		`set technology.automation.prerequisites = ["electronics", "steelworks-plating"]`,
	})
}

// prereqProbeWorld counts TechPrereqs questions. It is the only way to hold up
// "the World is asked once": the walk's answer is the same either way, and what
// changes is how many times the host was crossed to get it.
type prereqProbeWorld struct {
	*fixtureWorld
	asked int
}

func (w *prereqProbeWorld) TechPrereqs(name string) []string {
	w.asked++
	return w.fixtureWorld.TechPrereqs(name)
}

// THE WORLD IS ASKED ONCE PER TECHNOLOGY, HOWEVER MANY PASSES THE WALK TAKES.
// Its prerequisite lists cannot move between passes, only the plan's rewrites
// can, so re-asking is a host crossing per technology per pass for an answer
// that is already in hand. The plan below takes THREE passes (two drops and the
// clean walk that follows them), and the count must still be one per name.
func TestTheCycleWalkAsksTheWorldItsPrerequisitesOnce(t *testing.T) {
	lib := New()
	lib.Technology("riveting", TechSpec{CostOf: "electronics", After: "electronics", Before: "automation"})
	lib.Technology("forging", TechSpec{CostOf: "electronics", After: "automation", Before: "electronics"})
	lib.Technology("plating", TechSpec{CostOf: "electronics", After: "logistics", Before: "automation"})

	w := &prereqProbeWorld{fixtureWorld: baseWorld()}
	_, err := lib.PlanData(w)
	assertNoError(t, err)

	// TWO ASKED BEFORE THE WALK RUNS AT ALL, and they are currentPrereqs' own
	// reader rather than this one: resolve asks the game for automation's list
	// and for electronics' when it builds the first splice into each. The
	// SECOND splice into automation reads the record the first one left and
	// asks the game nothing, which is that reader's whole job.
	want := len(w.TechNames()) + 2
	if w.asked != want {
		t.Errorf("the World was asked for a prerequisite list %d times, want %d", w.asked, want)
	}
}
