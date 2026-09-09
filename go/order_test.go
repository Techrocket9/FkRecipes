package fkrecipes

import (
	"strconv"
	"testing"
)

// OrderAfter, the order collision it makes visible, and the placement it
// refuses rather than silently missing.
//
// THE FRICTION THIS ANSWERS is a migrated mod's: a consumer holding legacy
// orders "a" and "b" could not choose where a GENERATED setting landed,
// because the two letters come from the declaration index and nothing else.
// They count DECLARATION SLOTS, the legacy declarations among them: a
// generated setting in any of the first twenty-six slots sorted between the
// two dropdowns, and the twenty-seventh declaration carries "ba" and sorts
// past the second one. The pilot declared four new settings as Legacy purely
// to place them, hand-writing prefixed names it did not need.
//
// THE PLAN THAT NEVER CALLS IT IS WITNESSED ELSEWHERE, on purpose:
// TestPlanSettingsOrderStringsRollOver in settings_test.go and
// TestLegacySettingsKeepTheirNamesAndOrders in migration_test.go were written
// before this surface existed and are unchanged by it, so they are what says
// the bare two letters still come out exactly as they did.

// The pilot's shape, whole. The two legacy dropdowns keep "a" and "b", and
// each group of generated settings sorts under the dropdown it belongs to,
// which is the placement no ordering of the declarations could reach before.
// EVERY GENERATED CONSTRUCTOR IS IN IT, one under each prefix or both, so
// dropping the capture from any one of them shows up here as an order two
// letters short.
func TestOrderAfterPlacesGeneratedSettingsUnderALegacyOrder(t *testing.T) {
	lib := New()
	lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
		[]string{"vanilla", "cheap", "custom"}, "a")
	lib.OrderAfter("a")
	parts := lib.IngredientsSetting("recipe-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	lib.BoolSetting("recipe-hint", true)
	lib.LegacyDropdownSettingNeedingLocale("bbb-tech-cost", "logistics",
		[]string{"logistics", "custom"}, "b")
	lib.OrderAfter("b")
	packs := lib.PacksSetting("tech-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("tech-count", 20, Between(1, 1000000))
	seconds := lib.DoubleSetting("tech-seconds", 15, Between(1, 3600))
	lib.DropdownSettingNeedingLocale("tech-style", "plain", []string{"plain", "fancy"})

	rivet := lib.Item("steel-rivet", ItemSpec{})
	lib.Recipe(rivet, RecipeSpec{IngredientsFrom: parts})
	lib.Technology("hardened-tips", TechSpec{
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="bbb-recipe-cost", setting_type="startup", default_value="vanilla", order="a", allowed_values=["vanilla", "cheap", "custom"]}`,
		`extend {type="string-setting", name="steelworks-recipe-ingredients", setting_type="startup",` +
			` default_value="default", order="aab", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.steelworks-recipe-ingredients"], "` + "\n" + `default: 2 steel-plate"` + wantTextTail + `]}`,
		`extend {type="bool-setting", name="steelworks-recipe-hint", setting_type="startup", default_value=true, order="aac"}`,
		`extend {type="string-setting", name="bbb-tech-cost", setting_type="startup", default_value="logistics", order="b", allowed_values=["logistics", "custom"]}`,
		`extend {type="string-setting", name="steelworks-tech-packs", setting_type="startup",` +
			` default_value="default", order="bae", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.steelworks-tech-packs"], "` + "\n" + `default: 1 automation-science-pack"` + wantTextTail + `]}`,
		`extend {type="int-setting", name="steelworks-tech-count", setting_type="startup", default_value=20, order="baf", minimum_value=1, maximum_value=1000000}`,
		`extend {type="double-setting", name="steelworks-tech-seconds", setting_type="startup", default_value=15, order="bag", minimum_value=1, maximum_value=3600}`,
		`extend {type="string-setting", name="steelworks-tech-style", setting_type="startup", default_value="plain", order="bah", allowed_values=["plain", "fancy"]}`,
	})
}

// A LEGACY SETTING KEEPS ITS OWN ORDER WHATEVER PREFIX IS IN FORCE. What this
// holds is emittedOrder's first line, which answers with the declared order
// before it reads the captured prefix at all; it says nothing about what any
// constructor stored, because the capture asks nothing about legacy and what
// a legacy declaration holds there is read by nothing. The order a migrated
// mod already ships is the whole point of the Legacy constructors, and a
// prefix in front of it would move the setting the consumer is preserving.
// Both text constructors are in it, since they share a helper with the
// generated surface, and two scalar Legacy ones with them.
func TestALegacySettingDeclaredAfterOrderAfterKeepsItsOwnOrder(t *testing.T) {
	lib := New()
	lib.OrderAfter("a")
	parts := lib.LegacyIngredientsSetting("bbb-recipe-parts",
		[]Ingredient{IngredientNamed(2, "steel-plate")}, "c")
	packs := lib.LegacyPacksSetting("bbb-tech-packs",
		[]Pack{{Name: "automation-science-pack", Amount: 1}}, "d")
	count := lib.LegacyIntSetting("bbb-tech-count", 20, Between(1, 1000), "e")
	seconds := lib.LegacyDoubleSetting("bbb-tech-seconds", 15, Between(1, 600), "f")

	rivet := lib.Item("steel-rivet", ItemSpec{})
	lib.Recipe(rivet, RecipeSpec{IngredientsFrom: parts})
	lib.Technology("hardened-tips", TechSpec{
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	for i, want := range []string{"c", "d", "e", "f"} {
		got, ok := field(ops[i].Proto, "order")
		if !ok || got.Str != want {
			t.Errorf("legacy setting %d carries the order %q, want %q", i, got.Str, want)
		}
	}
}

// An empty order names nothing to sort after, so it is refused rather than
// quietly meaning "back to the bare two letters". The refusal is the SETTINGS
// stage's: PlanData's validate runs the binding and text-setting validators
// and never validateSettings, which the last case here holds up to the light.
func TestOrderAfterRefusesAnEmptyOrder(t *testing.T) {
	want := "fkrecipes: OrderAfter was given an empty order; name the order string the generated settings should follow"
	cases := []struct {
		name  string
		build func(*Lib)
	}{
		{
			// Refused with nothing after it at all: the call is the mistake,
			// and a plan that carried it silently would start refusing the
			// day somebody added a setting under it.
			name:  "no setting follows it",
			build: func(l *Lib) { l.OrderAfter("") },
		},
		{
			name: "settings were declared before it",
			build: func(l *Lib) {
				l.BoolSetting("hardened-tools", true)
				l.OrderAfter("")
			},
		},
		{
			name: "settings follow it",
			build: func(l *Lib) {
				l.OrderAfter("")
				l.BoolSetting("hardened-tools", true)
				l.IntSetting("axe-durability", 250, Between(50, 1000))
			},
		},
		{
			// A LATER, VALID CALL DOES NOT CLEAR IT. The empty call was a
			// mistake when it was written, and the settings declared under
			// some other order afterwards do not make it one the consumer
			// meant; the plan says so rather than swallowing it.
			name: "a later call names a real order",
			build: func(l *Lib) {
				l.OrderAfter("")
				l.OrderAfter("a")
				l.BoolSetting("hardened-tools", true)
			},
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanSettings(settingsWorld())
			if err == nil {
				t.Fatalf("plan was accepted, want refusal %q", want)
			}
			if err.Error() != want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}

	// The data stage validates bindings and text settings and nothing about
	// the settings themselves, so the same plan passes there. This is the
	// stage split as a fact rather than as a comment.
	lib := New()
	lib.OrderAfter("")
	lib.BoolSetting("hardened-tools", true)
	if _, err := lib.PlanData(baseWorld()); err != nil {
		t.Errorf("the data stage refused a settings-stage mistake: %s", err)
	}
}

// A generated order landing on a legacy one is refused: two settings sharing
// an order string are placed by the engine rather than by the consumer or by
// this library, and neither of them chose it.
func TestAGeneratedOrderMayNotLandOnALegacyOne(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			// The pilot's shape with one more legacy setting in it, declared
			// AFTER the generated one it collides with: the scan covers every
			// setting rather than the ones already seen, because a legacy
			// order is carried whenever it was declared.
			name: "a legacy setting declared after the generated one it collides with",
			build: func(l *Lib) {
				l.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
					[]string{"vanilla", "cheap"}, "a")
				l.OrderAfter("a")
				parts := l.IngredientsSetting("recipe-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
				l.LegacyBoolSetting("bbb-recipe-detail", false, "aab")
				rivet := l.Item("steel-rivet", ItemSpec{})
				l.Recipe(rivet, RecipeSpec{IngredientsFrom: parts})
			},
			want: "fkrecipes: the setting recipe-ingredients would carry the order aab, which the legacy setting bbb-recipe-detail already carries; give one of them an order of its own",
		},
		{
			// NO OrderAfter ANYWHERE: the first generated setting has always
			// carried "aa", and a mod whose historic order was "aa" has
			// always tied with it silently. That plan loaded before this
			// check existed and is refused now, deliberately.
			name: "a plan that never calls OrderAfter",
			build: func(l *Lib) {
				l.BoolSetting("hardened-tools", true)
				l.LegacyBoolSetting("bbb-enabled", false, "aa")
			},
			want: "fkrecipes: the setting hardened-tools would carry the order aa, which the legacy setting bbb-enabled already carries; give one of them an order of its own",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanSettings(settingsWorld())
			if err == nil {
				t.Fatalf("plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}
}

// THE PLACEMENT EQUALITY ALONE MISSES. A legacy order that EXTENDS the named
// one sits inside the range the two letters walk, so a generated setting far
// enough along the alphabet sorts past the very setting the consumer was
// placing things under, with no collision anywhere. The two letters are
// arithmetic, so this is arithmetic too: it is a matter of how many settings
// were declared, which is exactly the kind of break that ships.
func TestAGeneratedOrderMayNotSortPastALegacyOrderUnderItsPrefix(t *testing.T) {
	// Legacy "a" and "ab" with OrderAfter("a"): the generated settings run
	// "aac", "aad", ... "aaz" and then "aba". Everything up to "aaz" sorts
	// before "ab" and is fine; "aba" is past it.
	underA := func(n int) *Lib {
		l := New()
		l.LegacyBoolSetting("bbb-recipe-cost", false, "a")
		l.LegacyBoolSetting("bbb-recipe-detail", false, "ab")
		l.OrderAfter("a")
		for i := 0; i < n; i++ {
			l.BoolSetting("toggle-"+strconv.Itoa(i), false)
		}
		return l
	}

	t.Run("twenty-four generated settings still sort before it", func(t *testing.T) {
		ops, err := underA(24).PlanSettings(settingsWorld())
		assertNoError(t, err)
		last, ok := field(ops[len(ops)-1].Proto, "order")
		if !ok || last.Str != "aaz" {
			t.Fatalf("the last generated setting carries the order %q, want aaz", last.Str)
		}
	})

	cases := []struct {
		name  string
		build func() *Lib
		want  string
	}{
		{
			// The twenty-fifth is the first one the second letter rolls over
			// for, and it is refused.
			name:  "the twenty-fifth generated setting rolls over past it",
			build: func() *Lib { return underA(25) },
			want:  "fkrecipes: the setting toggle-24 would carry the order aba and sort past the legacy setting bbb-recipe-detail at ab, which extends a; OrderAfter(a) places settings before every legacy order that extends a",
		},
		{
			// A one-letter legacy order with a two-letter one under it needs
			// no rollover at all: the FIRST generated setting is already
			// past "ba".
			name: "a two-letter legacy order under a one-letter one catches the first setting",
			build: func() *Lib {
				l := New()
				l.LegacyBoolSetting("bbb-tech-cost", false, "b")
				l.LegacyBoolSetting("bbb-tech-detail", false, "ba")
				l.OrderAfter("b")
				l.BoolSetting("tech-hint", true)
				return l
			},
			want: "fkrecipes: the setting tech-hint would carry the order bac and sort past the legacy setting bbb-tech-detail at ba, which extends b; OrderAfter(b) places settings before every legacy order that extends b",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			ops, err := c.build().PlanSettings(settingsWorld())
			if err == nil {
				t.Fatalf("plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}

	// A PLAN WITH NO OrderAfter IS NOT TOUCHED BY THIS RULE, and that is the
	// whole reason the check asks for a non-empty prefix: every legacy order
	// extends the empty string, so a rule that ignored the prefix would refuse
	// the oldest shape there is, a legacy "a" beside the generated "aa" that
	// the two letters have always produced.
	lib := New()
	lib.BoolSetting("hardened-tools", true)
	lib.LegacyBoolSetting("bbb-enabled", false, "a")
	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="aa"}`,
		`extend {type="bool-setting", name="bbb-enabled", setting_type="startup", default_value=false, order="a"}`,
	})
}

// WHICH REFUSAL ANSWERS A PLAN THAT HAS TWO. The order checks are a SECOND
// PASS, after every setting has been through its own: each refusal here names
// a setting other than the one being examined, and naming one the plan has
// not validated yet quotes a name that may not exist. Each case below pairs
// an order collision with a mistake the first pass answers, and the collision
// on its own is on the record first, so a check moved back inside the loop
// turns these red rather than merely reordering them.
func TestTheOrderChecksRunAfterEverySettingIsValidated(t *testing.T) {
	// A legacy "aa" beside a generated setting at index 0, which is the index
	// whose two letters are "aa".
	collision := func(l *Lib) {
		l.BoolSetting("hardened-tools", true)
		l.LegacyBoolSetting("bbb-enabled", false, "aa")
	}
	collisionRefusal := "fkrecipes: the setting hardened-tools would carry the order aa, which the legacy setting bbb-enabled already carries; give one of them an order of its own"
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			name:  "the collision alone",
			build: collision,
			want:  collisionRefusal,
		},
		{
			// The empty order is recorded on the PLAN rather than at a
			// declaration, so it is answered before the first pass, let alone
			// the second.
			name: "an empty order beside the collision",
			build: func(l *Lib) {
				collision(l)
				l.OrderAfter("")
			},
			want: "fkrecipes: OrderAfter was given an empty order; name the order string the generated settings should follow",
		},
		{
			// THE SENTENCE THE OLD SHAPE PRODUCED IS THE POINT: the legacy
			// setting the collision names has no name, so the refusal read
			// "which the legacy setting  already carries", with a blank in
			// the middle of it, instead of naming the empty name that is the
			// real mistake.
			name: "a legacy setting with no name at all",
			build: func(l *Lib) {
				l.BoolSetting("hardened-tools", true)
				l.LegacyBoolSetting("", false, "aa")
			},
			want: "fkrecipes: a setting was declared with an empty name",
		},
		{
			// The empty legacy order is on a THIRD setting, later than both:
			// a first pass that runs to the end reaches it, an order check
			// inside the loop never gets there.
			name: "a legacy setting with an empty order, declared later",
			build: func(l *Lib) {
				collision(l)
				l.LegacyBoolSetting("bbb-detail", false, "")
			},
			want: "fkrecipes: the legacy setting bbb-detail was declared with an empty order",
		},
		{
			// The next three put the second mistake on the COLLIDING setting
			// itself, where the old shape pre-empted it by checking the order
			// before the switch that reaches these.
			name: "a dropdown default outside its own values",
			build: func(l *Lib) {
				l.DropdownSettingNeedingLocale("smelting-style", "electric-furnace", []string{"furnace", "foundry"})
				l.LegacyBoolSetting("bbb-enabled", false, "aa")
			},
			want: "fkrecipes: the dropdown setting smelting-style defaults to electric-furnace, which is not one of its allowed values",
		},
		{
			name: "a craft-time minimum at the engine floor",
			build: func(l *Lib) {
				forging := l.DoubleSetting("axe-craft-time", 2, NumericSpec{HasMin: true, Min: 0.001})
				l.LegacyBoolSetting("bbb-enabled", false, "aa")
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: forging})
			},
			want: "fkrecipes: the setting axe-craft-time backs a crafting time but declares a minimum at or below the engine floor (energy_required can't be <= 0.001)",
		},
		{
			name: "an int default past what a double holds",
			build: func(l *Lib) {
				l.IntSetting("axe-durability", 9007199254740993, NumericSpec{})
				l.LegacyBoolSetting("bbb-enabled", false, "aa")
			},
			want: "fkrecipes: the numeric setting axe-durability declares a default a Lua double cannot hold exactly: 9007199254740993",
		},
		{
			// A duplicate emitted name is answered in the first pass too, and
			// this one was never a question of passes: a setting the engine
			// drops has no order worth discussing. The generated setting at
			// index 1 carries the order "ab", which the legacy setting
			// carries as well.
			name: "a duplicate emitted name beside a collision",
			build: func(l *Lib) {
				l.BoolSetting("hardened-tools", true)
				l.BoolSetting("hardened-tools", false)
				l.LegacyBoolSetting("bbb-enabled", false, "ab")
			},
			want: "fkrecipes: two settings share the name steelworks-hardened-tools; the engine keeps the last one silently",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanSettings(settingsWorld())
			if err == nil {
				t.Fatalf("plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}
}
