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
func TestCycleCreatedByInsertBetween(t *testing.T) {
	// logistics-3 -> logistics-2 -> steel-processing, and nothing points back:
	// a sound tree.
	w := baseWorld().withPrereqs("logistics-2", "logistics", "steel-processing")

	lib := New()
	// Splicing between logistics-3 and steel-processing makes
	// steel-processing require the new technology, which requires logistics-3,
	// which already leads back to steel-processing.
	lib.Technology("steel-axes", TechSpec{CostOf: "electronics", After: "logistics-3", Before: "steel-processing"})

	_, err := lib.PlanData(w)
	if err == nil {
		t.Fatal("the plan was accepted, want a cycle refusal")
	}
	want := "fkrecipes: a prerequisite cycle: logistics-2 -> steel-processing -> steelworks-steel-axes -> logistics-3 -> logistics-2"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
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
