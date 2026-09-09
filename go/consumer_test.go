package fkrecipes

import "testing"

// The surfaces the BetterBeltBalancer pilot was blocked on: prototype names a
// migrating mod keeps, the prototype fields it needs, and a recipe whose
// result it did not declare.

// ---------------------------------------------------------------------------
// Legacy prototypes.
// ---------------------------------------------------------------------------

// A NAME A SAVE ALREADY REFERENCES IS EMITTED VERBATIM. The pilot's own item
// is the case: a hand-rolled entity names it, and the engine's answer to a
// renamed one is an assignID abort rather than a warning.
func TestLegacyPrototypesKeepTheirNames(t *testing.T) {
	lib := New()
	part := lib.LegacyItem("bbb-balancer-part", ItemSpec{StackSize: 50})
	lib.LegacyRecipe(part, "bbb-balancer-part", RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
	})
	lib.LegacyTechnology("bbb-balancer", TechSpec{CostOf: "logistics-2"})
	// A generated declaration beside them still takes the prefix.
	gen := lib.Item("hardened-steel-plate", ItemSpec{})
	lib.Recipe(gen, RecipeSpec{Ingredients: []Ingredient{IngredientOf(part, 2)}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="bbb-balancer-part", stack_size=50}`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="bbb-balancer-part", enabled=true, ingredients=[{type="item", name="steel-plate", amount=1}], results=[{type="item", name="bbb-balancer-part", amount=1}]}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="bbb-balancer-part", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
		`extend {type="technology", name="bbb-balancer", unit=` + logistics2Unit + `}`,
	})
}

// A LEGACY HANDLE IS AN ORDINARY HANDLE, which is what lets a mod migrate one
// prototype at a time: the generated half references the legacy half and the
// splices reach both.
//
// AND IT MAKES ITSELF, which is why the first line of this transcript is the
// self-product line: the recipe's one ingredient is a handle to its own result
// item. That is a legal shape the base game ships and it stays accepted; the
// line is the signal, and it is asserted here rather than trimmed away because
// this plan is the smallest one in the suite that reaches it by accident.
func TestLegacyAndGeneratedPrototypesInterlock(t *testing.T) {
	lib := New()
	part := lib.LegacyItem("bbb-balancer-part", ItemSpec{})
	rec := lib.LegacyRecipe(part, "bbb-balancer-part", RecipeSpec{
		Ingredients: []Ingredient{IngredientOf(part, 1)},
	})
	first := lib.LegacyTechnology("bbb-balancer", TechSpec{
		CostOf:  "logistics-2",
		After:   "steel-processing",
		Unlocks: []RecipeRef{rec},
	})
	// A GENERATED technology anchored on a LEGACY one by handle.
	lib.Technology("balancer-2", TechSpec{CostOf: "logistics-3", AfterTech: first})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: bbb-balancer-part: bbb-balancer-part is in the list and is also what this recipe makes, so nothing can craft the first one unless something else produces it`,
		`extend {type="item", name="bbb-balancer-part", stack_size=50}`,
		`extend {type="recipe", name="bbb-balancer-part", enabled=false, ingredients=[{type="item", name="bbb-balancer-part", amount=1}], results=[{type="item", name="bbb-balancer-part", amount=1}]}`,
		`extend {type="technology", name="bbb-balancer", prerequisites=["steel-processing"], unit=` + logistics2Unit + `, effects=[{type="unlock-recipe", recipe="bbb-balancer-part"}]}`,
		`extend {type="technology", name="steelworks-balancer-2", prerequisites=["bbb-balancer"], unit=` + logistics3Unit + `}`,
	})
}

func TestLegacyPrototypeRefusals(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			// The names differ as declared and collide as EMITTED, which is
			// the namespace the engine keeps.
			name: "a legacy item name colliding with a generated one",
			build: func(l *Lib) {
				l.Item("hardened-steel-plate", ItemSpec{})
				l.LegacyItem("steelworks-hardened-steel-plate", ItemSpec{})
			},
			want: "fkrecipes: two items share the name steelworks-hardened-steel-plate; the second would overwrite the first",
		},
		{
			name: "a legacy technology name colliding with a generated one",
			build: func(l *Lib) {
				l.Technology("balancer", TechSpec{CostOf: "logistics-2"})
				l.LegacyTechnology("steelworks-balancer", TechSpec{CostOf: "logistics-2"})
			},
			want: "fkrecipes: two technologies share the name steelworks-balancer; the second would overwrite the first",
		},
		{
			// A legacy name is checked against data.raw under the name it
			// really carries, not under a prefix it never gets.
			name: "a legacy item that already exists in data.raw",
			build: func(l *Lib) {
				l.LegacyItem("steel-plate", ItemSpec{})
			},
			want: "fkrecipes: the item steel-plate already exists in data.raw; this plan would overwrite it",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			_, err := lib.PlanData(baseWorld())
			if err == nil {
				t.Fatalf("the plan was accepted, want %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// Order, PlaceResult and Extra.
// ---------------------------------------------------------------------------

// All four fields the pilot measured DROPPED from the dump, emitted.
func TestFieldSlotsReachThePrototypes(t *testing.T) {
	lib := New()
	part := lib.Item("balancer-part", ItemSpec{
		Order:       "z[balancer]",
		PlaceResult: "steel-chest",
		Extra:       []KV{kv("weight", Num(100))},
	})
	lib.Recipe(part, RecipeSpec{
		Order:       "z[balancer]-a",
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		Extra:       []KV{kv("allow_productivity", Bool(true))},
	})
	lib.Technology("balancer", TechSpec{
		CostOf: "logistics-2",
		Order:  "c-b-z",
		Extra:  []KV{kv("upgrade", Bool(false))},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-balancer-part", stack_size=50, order="z[balancer]", place_result="steel-chest", weight=100}`,
		`extend {type="recipe", name="steelworks-balancer-part", enabled=true, ingredients=[{type="item", name="steel-plate", amount=1}], results=[{type="item", name="steelworks-balancer-part", amount=1}], order="z[balancer]-a", allow_productivity=true}`,
		`extend {type="technology", name="steelworks-balancer", unit=` + logistics2Unit + `, order="c-b-z", upgrade=false}`,
	})
}

// A place_result naming an entity the WORLD gained at test time, so the probe
// is a real lookup rather than anything that could be special-cased on the
// base fixture's one row. This is the pilot's own shape: the entity is the
// mod's own hand-rolled neighbour, present because the mod declares it beside
// the Emit call.
func TestPlaceResultResolvesAgainstTheWorld(t *testing.T) {
	lib := New()
	lib.Item("balancer-part", ItemSpec{PlaceResult: "bbb-balancer-1-to-2"})

	w := baseWorld().withEntity("bbb-balancer-1-to-2")
	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-balancer-part", stack_size=50, place_result="bbb-balancer-1-to-2"}`,
	})
}

// ---------------------------------------------------------------------------
// A recipe whose result this plan does not declare.
// ---------------------------------------------------------------------------

// The migration case: a recipe moves onto the library before its item does, or
// the recipe produces somebody else's item outright.
func TestRecipeProducingAnExistingItem(t *testing.T) {
	lib := New()
	lib.Recipe(ItemRef{}, RecipeSpec{
		Name:        "steel-plate-recycling",
		ResultNamed: "steel-plate",
		ResultCount: 2,
		Ingredients: []Ingredient{IngredientNamed(1, "iron-plate")},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="recipe", name="steelworks-steel-plate-recycling", enabled=true, ingredients=[{type="item", name="iron-plate", amount=1}], results=[{type="item", name="steel-plate", amount=2}]}`,
	})
}

func TestConsumerSurfaceRefusals(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			// The engine's own answer to this is an assignID ABORT naming the
			// item, which is what the pilot's scratch clone hit.
			name: "a place_result that does not exist",
			build: func(l *Lib) {
				l.Item("balancer-part", ItemSpec{PlaceResult: "bbb-balancer-1-to-2"})
			},
			want: "fkrecipes: the item balancer-part names a place_result bbb-balancer-1-to-2 that does not exist",
		},
		{
			name: "an Extra key this library emits itself",
			build: func(l *Lib) {
				it := l.Item("balancer-part", ItemSpec{})
				l.Recipe(it, RecipeSpec{Extra: []KV{kv("ingredients", Arr())}})
			},
			want: "fkrecipes: the recipe balancer-part sets ingredients through Extra, which this library emits",
		},
		{
			name: "an Extra key set twice",
			build: func(l *Lib) {
				l.Item("balancer-part", ItemSpec{Extra: []KV{
					kv("weight", Num(1)), kv("weight", Num(2)),
				}})
			},
			want: "fkrecipes: the item balancer-part sets weight through Extra twice",
		},
		{
			name: "an Extra key with no name",
			build: func(l *Lib) {
				l.Technology("balancer", TechSpec{CostOf: "logistics-2", Extra: []KV{kv("", Num(1))}})
			},
			want: "fkrecipes: the technology balancer sets a field through Extra with an empty name",
		},
		{
			// THE PROBE ASKS THE WORLD, and the World is data.raw before this
			// plan runs, so the plan's own item is not there yet. Refusing is
			// right; refusing with "does not exist" would send the consumer
			// looking for a missing prototype they can see two lines up.
			name: "a ResultNamed item this plan declares",
			build: func(l *Lib) {
				l.Item("balancer-part", ItemSpec{})
				l.Recipe(ItemRef{}, RecipeSpec{Name: "assembly", ResultNamed: "steelworks-balancer-part"})
			},
			want: "fkrecipes: the recipe assembly produces steelworks-balancer-part through ResultNamed, which this plan declares; use the item's handle instead",
		},
		{
			// The same, under a LEGACY item's verbatim name: the comparison is
			// on emitted names, so both declaration shapes are covered.
			name: "a ResultNamed item this plan declares under a legacy name",
			build: func(l *Lib) {
				l.LegacyItem("bbb-balancer-part", ItemSpec{})
				l.Recipe(ItemRef{}, RecipeSpec{Name: "assembly", ResultNamed: "bbb-balancer-part"})
			},
			want: "fkrecipes: the recipe assembly produces bbb-balancer-part through ResultNamed, which this plan declares; use the item's handle instead",
		},
		{
			name: "a ResultNamed item that does not exist",
			build: func(l *Lib) {
				l.Recipe(ItemRef{}, RecipeSpec{Name: "smelting", ResultNamed: "tungsten-plate"})
			},
			want: "fkrecipes: the recipe smelting produces tungsten-plate, which does not exist",
		},
		{
			name: "a ResultNamed recipe with no name to inherit",
			build: func(l *Lib) {
				l.Recipe(ItemRef{}, RecipeSpec{ResultNamed: "steel-plate"})
			},
			want: "fkrecipes: a recipe producing an existing item was declared with no name; there is no declared item to take one from",
		},
		{
			name: "both a result handle and ResultNamed",
			build: func(l *Lib) {
				it := l.Item("balancer-part", ItemSpec{})
				l.Recipe(it, RecipeSpec{ResultNamed: "steel-plate"})
			},
			want: "fkrecipes: the recipe balancer-part names both a result item and ResultNamed; pick one",
		},
		{
			// The pre-existing refusal, unchanged: neither a handle nor a name
			// is still a recipe with no result.
			name: "neither a result handle nor a name",
			build: func(l *Lib) {
				l.Recipe(ItemRef{}, RecipeSpec{Name: "smelting"})
			},
			want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanData(baseWorld())
			if err == nil {
				t.Fatalf("the plan was accepted with %d ops, want %q", len(ops), c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// The narrowed settings seam, and the exported locale reader.
// ---------------------------------------------------------------------------

// justAName is what a consumer's settings test now has to write: ONE method,
// where the full World is ten. That this type satisfies PlanSettings is the
// whole assertion, and it is made at compile time.
type justAName struct{}

func (justAName) ModName() string { return "better-belt-balancer" }

func TestPlanSettingsTakesOnlyAName(t *testing.T) {
	lib := New()
	lib.LegacyBoolSetting("bbb-multi-edge-parts", false, "a")

	ops, err := lib.PlanSettings(justAName{})
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="bool-setting", name="bbb-multi-edge-parts", setting_type="startup", default_value=false, order="a"}`,
	})
}

// The parser's output, exported so a consumer can ask questions this library
// cannot: their own entity's locale entry, a translation file's key set.
func TestLocaleEntriesReadsTheFile(t *testing.T) {
	cfg := `[mod-setting-name]
bbb-recipe-cost=Recipe cost

[entity-name]
bbb-balancer-1-to-2=Balancer
`
	got := LocaleEntries(cfg)
	want := []LocaleEntry{
		{Section: "mod-setting-name", Key: "bbb-recipe-cost", Value: "Recipe cost"},
		{Section: "entity-name", Key: "bbb-balancer-1-to-2", Value: "Balancer"},
	}
	if len(got) != len(want) {
		t.Fatalf("got %d entries, want %d: %+v", len(got), len(want), got)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Errorf("entry %d\n got: %+v\nwant: %+v", i, got[i], want[i])
		}
	}
}

// EXTRA IS SNAPSHOTTED VALUES AND ALL, which is what keeps a plan a plan.
//
// A Value carries Arr and Map as SLICE HEADERS, so copying the KV backbone
// alone leaves a Pair holding a container pointing at the caller's backing
// array: the declaration would still be rewritable after it was made. Every
// other spec slice in this library holds flat structs and is safe from this;
// Extra is the exception, and is exactly where nested values live.
//
// The shape below is the one a consumer actually writes. Building the array
// inline hides the hazard; building it first and passing it second is what a
// list assembled from a loop or a helper looks like.
func TestExtraDoesNotAliasTheCallerValues(t *testing.T) {
	// A container in each arm, and one nested inside the other, because the
	// copy is recursive and a one-level copy would pass a one-level test.
	cases := []struct {
		name  string
		build func(*Lib, []Value, []KV)
		want  string
	}{
		{
			name: "on an item",
			build: func(l *Lib, flags []Value, fields []KV) {
				l.Item("balancer-part", ItemSpec{Extra: []KV{
					Pair("flags", Arr(flags...)),
					Pair("meta", Obj(fields...)),
				}})
			},
			want: `extend {type="item", name="steelworks-balancer-part", stack_size=50, flags=["hidden", ["deep"]], meta={tier="a", nested={inner="b"}}}`,
		},
		{
			name: "on a recipe",
			build: func(l *Lib, flags []Value, fields []KV) {
				it := l.Item("balancer-part", ItemSpec{})
				l.Recipe(it, RecipeSpec{Extra: []KV{
					Pair("flags", Arr(flags...)),
					Pair("meta", Obj(fields...)),
				}})
			},
			want: `extend {type="recipe", name="steelworks-balancer-part", enabled=true, ingredients=[], results=[{type="item", name="steelworks-balancer-part", amount=1}], flags=["hidden", ["deep"]], meta={tier="a", nested={inner="b"}}}`,
		},
		{
			name: "on a technology",
			build: func(l *Lib, flags []Value, fields []KV) {
				l.Technology("balancer", TechSpec{CostOf: "logistics-2", Extra: []KV{
					Pair("flags", Arr(flags...)),
					Pair("meta", Obj(fields...)),
				}})
			},
			want: `extend {type="technology", name="steelworks-balancer", unit=` + logistics2Unit + `, flags=["hidden", ["deep"]], meta={tier="a", nested={inner="b"}}}`,
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()

			inner := []Value{Str("deep")}
			flags := []Value{Str("hidden"), Arr(inner...)}
			innerFields := []KV{Pair("inner", Str("b"))}
			fields := []KV{Pair("tier", Str("a")), Pair("nested", Obj(innerFields...))}

			c.build(lib, flags, fields)

			// Every backing array the caller still holds, rewritten AFTER the
			// declaration was made.
			flags[0] = Str("something-else")
			inner[0] = Str("rewritten")
			fields[0] = Pair("tier", Str("z"))
			innerFields[0] = Pair("inner", Str("rewritten"))

			ops, err := lib.PlanData(baseWorld())
			assertNoError(t, err)

			lines := transcript(ops)
			got := lines[len(lines)-1]
			if got != c.want {
				t.Errorf("the plan followed the caller's edits\n got: %s\nwant: %s", got, c.want)
			}
		})
	}
}
