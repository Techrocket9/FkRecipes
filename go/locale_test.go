package fkrecipes

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

// steelworksSettings declares the settings the example guests declare, in the
// same order and with the same values, because testdata/locale/example.cfg is
// the example's own locale file and both halves have to produce one report
// over it.
//
// THE RECIPES AND THE TECHNOLOGIES ARE HERE FOR ONE REASON: the checker asks
// which dropdowns have a text setting beside them, and it asks the BINDINGS. A
// dropdown that does has its preset list composed onto its description, so that
// description stops being optional, and a fixture with the settings alone would
// report the three of them as needing nothing. Nothing else about them is read
// here, which is why the items are the two a recipe needs a result for and the
// ingredient lists are as short as they go.
func steelworksSettings() *Lib {
	lib := New()
	rivet := lib.Item("steel-rivet", ItemSpec{})
	chain := lib.Item("steel-chain", ItemSpec{})

	hardened := lib.BoolSetting("hardened-tools", true)
	lib.IntSetting("rivet-batch", 4, Between(1, 20))
	lib.DoubleSetting("forging-time", 3, NumericSpec{HasMax: true, Max: 120})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water",
		[]string{"water", "oil"})
	lib.BoolSetting("bonus-research", true)
	tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "projectile",
		[]string{"projectile", "military"})
	lib.DoubleSetting("tempering-hold", 1.5, NumericSpec{HasMin: true, Min: 0.5})
	rivetIngredients := lib.IngredientsSetting("rivet-ingredients",
		[]Ingredient{IngredientNamed(1, "iron-plate")})
	quenchIngredients := lib.IngredientsSetting("quench-ingredients",
		[]Ingredient{IngredientNamed(2, "steel-plate")})
	chainLinks := lib.DropdownSettingNeedingLocale("chain-links", "short",
		[]string{"short", "long"})
	chainIngredients := lib.IngredientsSetting("chain-ingredients",
		[]Ingredient{IngredientOf(rivet, 4)})
	tipsPacks := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	tipsCount := lib.IntSetting("tips-count", 0, Between(0, 100000))
	tipsSeconds := lib.IntSetting("tips-seconds", 0, Between(0, 600))
	chainPacks := lib.PacksSetting("chain-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	chainCount := lib.IntSetting("chain-count", 20, Between(1, 100000))
	chainSeconds := lib.IntSetting("chain-seconds", 10, Between(1, 600))
	// The scaffolding line, under the legacy names the example declares: one
	// dropdown, two recipes and a technology, with Describes on the first
	// recipe.
	scaffoldTier := lib.LegacyDropdownSettingNeedingLocale("steelworks-scaffold-tier",
		"light", []string{"light", "heavy"}, "za")
	scaffoldParts := lib.LegacyIngredientsSetting("steelworks-scaffold-parts",
		[]Ingredient{IngredientNamed(2, "iron-plate")}, "ya")
	scaffoldPacks := lib.LegacyPacksSetting("steelworks-scaffold-packs",
		[]Pack{{Name: "automation-science-pack", Amount: 1}}, "zc")
	scaffoldCount := lib.LegacyIntSetting("steelworks-scaffold-count", 0, Between(0, 100000), "zd")
	scaffoldSeconds := lib.LegacyIntSetting("steelworks-scaffold-seconds", 0, Between(0, 600), "ze")

	lib.Recipe(rivet, RecipeSpec{Name: "steel-rivet", IngredientsFrom: rivetIngredients})
	lib.Recipe(rivet, RecipeSpec{
		Name:     "hardened-steel-plate-quenching",
		Category: "crafting-with-fluid",
		IngredientsBy: &IngredientChoices{
			Setting: medium,
			Choices: []IngredientChoice{
				{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
				{Value: "oil", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
			},
		},
		IngredientsFrom: quenchIngredients,
	})
	lib.Recipe(chain, RecipeSpec{
		Name: "steel-chain",
		IngredientsBy: &IngredientChoices{
			Setting: chainLinks,
			Choices: []IngredientChoice{
				{Value: "short", Ingredients: []Ingredient{IngredientOf(rivet, 4)}},
				{Value: "long", Ingredients: []Ingredient{IngredientOf(rivet, 8)}},
			},
		},
		IngredientsFrom: chainIngredients,
	})
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: []CostChoice{
				{Value: "projectile", Display: "as much as the seventh projectile damage level, or the overhaul pack's own hardening", Sources: []string{"physical-projectile-damage-7"}},
				{Value: "military", Sources: []string{"military-4"}},
			},
			Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
		},
		CostFrom: &CustomCost{Packs: tipsPacks, Count: tipsCount, Seconds: tipsSeconds},
	})
	// No dropdown, so no description is composed onto one and the checker marks
	// nothing for it. It is here because the example declares it and its three
	// settings would otherwise sit in the fixture with nothing reading them.
	lib.Technology("chain-forging", TechSpec{
		CostFrom: &CustomCost{Packs: chainPacks, Count: chainCount, Seconds: chainSeconds},
		After:    "steel-processing",
	})
	bracket := lib.Recipe(rivet, RecipeSpec{
		Name: "scaffold-bracket",
		IngredientsBy: &IngredientChoices{
			Setting: scaffoldTier,
			Choices: []IngredientChoice{
				{Value: "light", Ingredients: []Ingredient{IngredientNamed(2, "iron-plate")}},
				{Value: "heavy", Ingredients: []Ingredient{IngredientNamed(4, "steel-plate")}},
			},
			Describes: true,
		},
		IngredientsFrom: scaffoldParts,
	})
	tie := lib.Recipe(chain, RecipeSpec{
		Name: "scaffold-tie",
		IngredientsBy: &IngredientChoices{
			Setting: scaffoldTier,
			Choices: []IngredientChoice{
				{Value: "light", Ingredients: []Ingredient{IngredientOf(rivet, 2)}},
				{Value: "heavy", Ingredients: []Ingredient{IngredientOf(rivet, 4)}},
			},
		},
	})
	lib.Technology("scaffold-raising", TechSpec{
		CostBy: &CostChoices{
			Setting: scaffoldTier,
			Choices: []CostChoice{
				{Value: "light", Sources: []string{"logistics-2"}},
				{Value: "heavy", Sources: []string{"logistics-3"}},
			},
			Fallback: UnitSpec{Count: 100, Seconds: 15, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
		},
		CostFrom: &CustomCost{Packs: scaffoldPacks, Count: scaffoldCount, Seconds: scaffoldSeconds},
		Unlocks:  []RecipeRef{bracket, tie},
	})
	lib.DescribeSetting(hardened, "Adds the hardened steel line, its scaffolding and the research that unlocks them.")
	lib.DescribeSetting(scaffoldParts, "What one scaffold bracket is made of while this is not on default.\nLight scaffolding is the cheap bill; heavy is the one that holds a roof up.")
	lib.DescribeSetting(scaffoldTier, "Which bill the scaffolding line is built to, and which technology pays for raising it.")
	return lib
}

func readTestdata(t *testing.T, path string) string {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("the fixture is the thing under test and it is not there: %v", err)
	}
	return string(b)
}

// THE CROSS-LANGUAGE PIN. Both halves run this same plan over these same bytes
// and must produce this same file, so the two checkers are held to one output
// with no toolchain and no packaged mod in the way.
func TestCheckLocaleMatchesTheGolden(t *testing.T) {
	cfg := readTestdata(t, "../testdata/locale/example.cfg")
	golden := readTestdata(t, "../testdata/locale/findings.golden")

	got := strings.Join(steelworksSettings().CheckLocale("fkrecipes-example", cfg), "\n") + "\n"
	if got != golden {
		t.Errorf("the findings do not match the golden\n got:\n%s\nwant:\n%s", got, golden)
	}
}

// A file with every entry the plan needs and nothing it does not: a name for
// each of the seventeen settings, a description for each text setting, each
// research number and each dropdown a preset list is composed onto, and an
// entry for every dropdown value. The descriptions the engine treats as
// optional are left out on purpose, so this file is the MINIMUM rather than a
// copy of the example's.
func TestCheckLocaleAcceptsACompleteFile(t *testing.T) {
	cfg := `[mod-setting-name]
fkrecipes-example-hardened-tools=Hardened tools
fkrecipes-example-rivet-batch=Rivets per batch
fkrecipes-example-forging-time=Forging time
fkrecipes-example-quench-medium=Quenching medium
fkrecipes-example-bonus-research=Bonus research
fkrecipes-example-tips-research-tier=Tool tip research cost
fkrecipes-example-tempering-hold=Tempering hold
fkrecipes-example-rivet-ingredients=Steel rivet ingredients
fkrecipes-example-quench-ingredients=Custom quenching ingredients
fkrecipes-example-chain-links=Chain links
fkrecipes-example-chain-ingredients=Custom steel chain ingredients
fkrecipes-example-tips-packs=Tool tip science packs
fkrecipes-example-tips-count=Tool tip research count
fkrecipes-example-tips-seconds=Tool tip research seconds
fkrecipes-example-chain-packs=Chain forging science packs
fkrecipes-example-chain-count=Chain forging research count
fkrecipes-example-chain-seconds=Chain forging research seconds
steelworks-scaffold-tier=Scaffolding tier
steelworks-scaffold-parts=Custom scaffold bracket parts
steelworks-scaffold-packs=Scaffold raising science packs
steelworks-scaffold-count=Scaffold raising research count
steelworks-scaffold-seconds=Scaffold raising research seconds

[mod-setting-description]
fkrecipes-example-quench-medium=What the hot plate is dropped into.
fkrecipes-example-tips-research-tier=What a level of hardened tool tips costs.
fkrecipes-example-rivet-ingredients=Amount, then name, commas between.
fkrecipes-example-quench-ingredients=Amount, then name, commas between.
fkrecipes-example-chain-links=How much rivet a length of chain takes.
fkrecipes-example-chain-ingredients=Amount, then name, commas between.
fkrecipes-example-tips-packs=Amount, then pack name, commas between.
fkrecipes-example-tips-count=How many units of research a level takes.
fkrecipes-example-tips-seconds=Seconds per unit of research.
fkrecipes-example-chain-packs=Amount, then pack name, commas between.
fkrecipes-example-chain-count=How many units of research chain forging takes.
fkrecipes-example-chain-seconds=Seconds per unit of chain forging research.
steelworks-scaffold-packs=The science packs scaffold raising takes.
steelworks-scaffold-count=How many units of scaffold raising research.
steelworks-scaffold-seconds=Seconds per unit of scaffold raising research.

[string-mod-setting]
fkrecipes-example-quench-medium-water=Water
fkrecipes-example-quench-medium-oil=Oil
fkrecipes-example-tips-research-tier-projectile=As projectile damage
fkrecipes-example-tips-research-tier-military=As military research
fkrecipes-example-chain-links-short=Short links
fkrecipes-example-chain-links-long=Long links
steelworks-scaffold-tier-light=Light scaffolding
steelworks-scaffold-tier-heavy=Heavy scaffolding
`
	// A COMPLETE FILE PRODUCES NOTHING, which is the contract CheckLocale
	// keeps and the one a consumer's own suite is told to assert. The
	// advisories are not findings and are not here: they are a report of their
	// own, and TestCheckLocaleAdvisories is what pins them.
	if findings := steelworksSettings().CheckLocale("fkrecipes-example", cfg); len(findings) != 0 {
		t.Errorf("a complete file produced findings:\n%s", strings.Join(findings, "\n"))
	}
}

// equalLines is slice equality on report lines, which is all any caller here
// wants of it.
func equalLines(got, want []string) bool {
	if len(got) != len(want) {
		return false
	}
	for i := range got {
		if got[i] != want[i] {
			return false
		}
	}
	return true
}

// THE OUT-OF-PREFIX KEY IS AN ADVISORY AND THE IN-PREFIX ONE IS REQUIRED, which
// is the whole distinction this rule turns on.
//
// FACTORIO'S LOCALE NAMESPACE IS FLAT AND SHARED. Defining
// technology-name.military-4 in this mod's .cfg sets the displayed name of
// BASE's technology for every mod in the game, and this checker's own collision
// scan exists for exactly that hazard, so requiring the key would be requiring
// what the same checker flags. The note names the key, says what the tooltip
// shows where the game does not define it, and does not ask for an entry.
//
// AN ADVISORY IS NOT IN A FINDINGS REPORT AT ALL, and that is the property this
// test exists for. docs/migration.md tells a consumer to run CheckLocaleWith
// from their own suite and that it should be CLEAN; an advisory in that return
// would be a permanent red test over a key the consumer is told NOT to define.
// So the accessor carries them and neither CheckLocale nor CheckLocaleWith
// does, over a file that names nothing and over a file that names everything.
//
// IT READS NO .cfg, which is why the accessor takes none.
func TestCheckLocaleAdvisories(t *testing.T) {
	got := steelworksSettings().CheckLocaleAdvisories("fkrecipes-example")
	if !equalLines(got, advisoryNotes) {
		t.Errorf("the advisories are:\n%s", strings.Join(got, "\n"))
	}

	complete, err := os.ReadFile(filepath.Join("..", "testdata", "locale", "example.cfg"))
	if err != nil {
		t.Fatalf("reading the fixture cfg: %v", err)
	}
	for _, c := range []struct{ name, cfg string }{
		{"an empty file", ""},
		{"the fixture", string(complete)},
	} {
		reports := []struct {
			call string
			got  []string
		}{
			{"CheckLocale", steelworksSettings().CheckLocale("fkrecipes-example", c.cfg)},
			{"CheckLocaleWith", steelworksSettings().CheckLocaleWith("fkrecipes-example", c.cfg, nil)},
		}
		for _, r := range reports {
			for i, f := range r.got {
				if strings.HasPrefix(f, "note: ") {
					t.Errorf("%s over %s carries an advisory at %d: %s", r.call, c.name, i, f)
				}
			}
		}
	}
}

// THE ADVISORY IS ABOUT THE PLAN AND NOT ABOUT THE FILE, so a .cfg that DEFINES
// both of the game's keys, which is the squat the last clause warns about, gets
// the same two sentences. The accessor takes no cfg at all, which is what makes
// that unarguable; this holds the findings side of it, where a squatting file
// could have produced an orphan or a note and produces neither.
func TestCheckLocaleAdvisoriesDoNotDependOnTheFile(t *testing.T) {
	squatting := `[technology-name]
physical-projectile-damage-7=Projectile damage 7
military-4=Military 4
`
	if got := steelworksSettings().CheckLocaleAdvisories("fkrecipes-example"); !equalLines(got, advisoryNotes) {
		t.Errorf("the advisories moved:\n%s", strings.Join(got, "\n"))
	}
	for _, f := range steelworksSettings().CheckLocale("fkrecipes-example", squatting) {
		if strings.HasPrefix(f, "note: ") {
			t.Errorf("a squatting file put an advisory in the findings: %s", f)
		}
	}
}

// ONE DROPDOWN NAMING ONE KEY TWICE SAYS THE SENTENCE ONCE. A cost ladder may
// put the same first source under two tiers, which is an ordinary declaration
// and not a mistake, and the advisory is per (dropdown, key): without the
// dedupe the report repeats itself, and a consumer reading the same sentence
// twice learns nothing the second time.
//
// THE WITNESS IS THE COUNT AND THE ORDER TOGETHER: three tiers, two of them on
// the same source, produce two sentences, the repeated one at its FIRST
// position.
func TestCheckLocaleAdvisoriesSayARepeatedKeyOnce(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tier", "a", []string{"a", "b", "c"})
	packs := lib.PacksSetting("packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("count", 0, Between(0, 100000))
	seconds := lib.IntSetting("seconds", 0, Between(0, 600))
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: []CostChoice{
				{Value: "a", Sources: []string{"logistics"}},
				{Value: "b", Sources: []string{"military-4"}},
				{Value: "c", Sources: []string{"logistics"}},
			},
			Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
		},
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})

	want := []string{
		gameKeyAdvisory("mymod-tier", "technology-name.logistics", "logistics"),
		gameKeyAdvisory("mymod-tier", "technology-name.military-4", "military-4"),
	}
	if got := lib.CheckLocaleAdvisories("mymod"); !equalLines(got, want) {
		t.Errorf("the advisories are:\n%s\nwant:\n%s", strings.Join(got, "\n"), strings.Join(want, "\n"))
	}
}

// THE ADVISORIES HAVE A CAP OF THEIR OWN, with the shape the findings cap has
// and a budget no finding can spend: one dropdown with more distinctly-sourced
// choices than the cap takes is all it needs, and a plan can carry one.
func TestCheckLocaleAdvisoriesAreCapped(t *testing.T) {
	report := func(n int) []string {
		values := make([]string, 0, n)
		choices := make([]CostChoice, 0, n)
		for i := 0; i < n; i++ {
			v := "t" + strconv.Itoa(i)
			values = append(values, v)
			choices = append(choices, CostChoice{Value: v, Sources: []string{"src" + strconv.Itoa(i)}})
		}
		lib := New()
		tier := lib.DropdownSettingNeedingLocale("tier", values[0], values)
		packs := lib.PacksSetting("packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
		count := lib.IntSetting("count", 0, Between(0, 100000))
		seconds := lib.IntSetting("seconds", 0, Between(0, 600))
		lib.Technology("hardened-tips", TechSpec{
			CostBy: &CostChoices{
				Setting:  tier,
				Choices:  choices,
				Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
			},
			CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
		})
		return lib.CheckLocaleAdvisories("mymod")
	}

	got := report(localeAdvisoryCap + 50)
	if len(got) != localeAdvisoryCap+1 {
		t.Fatalf("got %d advisories, want the cap plus one closing line", len(got))
	}
	if got[localeAdvisoryCap] != "(and 50 more advisories)" {
		t.Errorf("the closing line is %q", got[localeAdvisoryCap])
	}

	// The boundary: exactly one advisory past the cap reads as one.
	got = report(localeAdvisoryCap + 1)
	if len(got) != localeAdvisoryCap+1 {
		t.Fatalf("got %d advisories at the boundary", len(got))
	}
	if got[localeAdvisoryCap] != "(and 1 more advisory)" {
		t.Errorf("the closing line at the boundary is %q", got[localeAdvisoryCap])
	}

	// One below it is not capped at all, so no closing line is added.
	got = report(localeAdvisoryCap)
	if len(got) != localeAdvisoryCap {
		t.Fatalf("got %d advisories just below the cap", len(got))
	}
	if strings.HasPrefix(got[localeAdvisoryCap-1], "(and ") {
		t.Errorf("a report at the cap gained a closing line: %q", got[localeAdvisoryCap-1])
	}
}

// everyNameMissing is what a file that names nothing produces over this plan:
// one finding per setting in DECLARATION order, and under each dropdown a
// preset list is composed onto the description it is composed onto, then the
// dropdown's values. It is written once because the cases below are about one
// finding each and would otherwise be seventeen settings of scenery apiece.
var everyNameMissing = []string{
	"the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
	"the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
	"the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
	"the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
	"the dropdown setting fkrecipes-example-quench-medium has no [mod-setting-description] entry, and the library composes its preset list onto that entry",
	"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
	"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
	"the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
	"the setting fkrecipes-example-tips-research-tier has no [mod-setting-name] entry",
	"the dropdown setting fkrecipes-example-tips-research-tier has no [mod-setting-description] entry, and the library composes its preset list onto that entry",
	"the dropdown setting fkrecipes-example-tips-research-tier has no [string-mod-setting] entry for its value projectile",
	"the dropdown setting fkrecipes-example-tips-research-tier has no [string-mod-setting] entry for its value military",
	"the setting fkrecipes-example-tempering-hold has no [mod-setting-name] entry",
	"the setting fkrecipes-example-rivet-ingredients has no [mod-setting-name] entry",
	"the setting fkrecipes-example-rivet-ingredients has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
	"the setting fkrecipes-example-quench-ingredients has no [mod-setting-name] entry",
	"the setting fkrecipes-example-quench-ingredients has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
	"the setting fkrecipes-example-chain-links has no [mod-setting-name] entry",
	"the dropdown setting fkrecipes-example-chain-links has no [mod-setting-description] entry, and the library composes its preset list onto that entry",
	"the dropdown setting fkrecipes-example-chain-links has no [string-mod-setting] entry for its value short",
	"the dropdown setting fkrecipes-example-chain-links has no [string-mod-setting] entry for its value long",
	"the setting fkrecipes-example-chain-ingredients has no [mod-setting-name] entry",
	"the setting fkrecipes-example-chain-ingredients has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
	"the setting fkrecipes-example-tips-packs has no [mod-setting-name] entry",
	"the setting fkrecipes-example-tips-packs has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
	"the setting fkrecipes-example-tips-count has no [mod-setting-name] entry",
	"the setting fkrecipes-example-tips-count has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
	"the setting fkrecipes-example-tips-seconds has no [mod-setting-name] entry",
	"the setting fkrecipes-example-tips-seconds has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
	"the setting fkrecipes-example-chain-packs has no [mod-setting-name] entry",
	"the setting fkrecipes-example-chain-packs has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
	"the setting fkrecipes-example-chain-count has no [mod-setting-name] entry",
	"the setting fkrecipes-example-chain-count has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
	"the setting fkrecipes-example-chain-seconds has no [mod-setting-name] entry",
	"the setting fkrecipes-example-chain-seconds has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
	// The scaffolding line, in declaration order behind the rest. Its
	// ingredient text needs a NAME entry and no description entry, because
	// the plan describes that one inline.
	"the setting steelworks-scaffold-tier has no [mod-setting-name] entry",
	// AND NO DESCRIPTION FINDING FOR IT, because the plan describes that
	// dropdown inline. Its VALUE entries are still required, which is the
	// narrow half of that rule: an inline description is the row's own text
	// and says nothing about what a player reads inside the list.
	"the dropdown setting steelworks-scaffold-tier has no [string-mod-setting] entry for its value light",
	"the dropdown setting steelworks-scaffold-tier has no [string-mod-setting] entry for its value heavy",
	"the setting steelworks-scaffold-parts has no [mod-setting-name] entry",
	"the setting steelworks-scaffold-packs has no [mod-setting-name] entry",
	"the setting steelworks-scaffold-packs has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
	"the setting steelworks-scaffold-count has no [mod-setting-name] entry",
	"the setting steelworks-scaffold-count has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
	"the setting steelworks-scaffold-seconds has no [mod-setting-name] entry",
	"the setting steelworks-scaffold-seconds has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
}

// advisoryNotes is what this plan's cost dropdown composes out of the GAME's
// namespace, in composition order: one note per out-of-prefix key its presets
// reference. They are ADVISORIES rather than findings, so they are in no
// findings report at all, and no .cfg this file can write makes them appear or
// go away. PINNED HERE RATHER THAN IN A GOLDEN FILE, because the two halves
// carry the same two literal sentences and a third copy on disk would be a
// third place to forget.
var advisoryNotes = []string{
	// ONE NOTE AND NOT TWO: the projectile choice carries a Display, so its
	// line names no technology-name key and there is nothing to advise about.
	"note: the dropdown setting fkrecipes-example-tips-research-tier composes the game's own key technology-name.military-4, which this plan does not own; where the game does not define it the tooltip shows military-4 instead, and defining it here would rename it for every mod",
}

// alsoFinding is everyNameMissing with more findings after it, copied so a
// case cannot write into the shared slice.
func alsoFinding(tail ...string) []string {
	out := make([]string, 0, len(everyNameMissing)+len(tail))
	out = append(out, everyNameMissing...)
	return append(out, tail...)
}

func TestCheckLocaleFindings(t *testing.T) {
	cases := []struct {
		name string
		cfg  string
		want []string
	}{
		{
			name: "a setting with no name entry",
			cfg: `[mod-setting-name]
fkrecipes-example-hardened-tools=Hardened tools
`,
			// The one setting the file names is the one finding that goes
			// away; everything after it is the ordinary report.
			want: everyNameMissing[1:],
		},
		{
			// Present but blank renders as nothing, which is the defect the
			// player reports, so it counts as missing.
			name: "an entry that is present but empty",
			cfg: `[mod-setting-name]
fkrecipes-example-hardened-tools=
`,
			want: everyNameMissing,
		},
		{
			name: "a description entry matching nothing",
			cfg: `[mod-setting-description]
fkrecipes-example-scrap-recovery=Left behind by a rename.
`,
			want: alsoFinding("the [mod-setting-description] entry fkrecipes-example-scrap-recovery matches no setting this plan declares"),
		},
		{
			// Another mod's string setting shares the section and is none of
			// this checker's business.
			name: "a value entry under a setting this plan does not own",
			cfg: `[string-mod-setting]
someothermod-belt-tier-express=Express
`,
			want: everyNameMissing,
		},
		{
			name: "a line that is neither a section nor an entry",
			cfg: `[mod-setting-name]
this line has no equals sign
`,
			// The parse finding comes first, before anything about the
			// settings: the reader is told the file is malformed before they
			// are told what it is missing.
			want: append([]string{"the locale line this line has no equals sign is neither a section nor an entry"},
				everyNameMissing...),
		},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got := steelworksSettings().CheckLocale("fkrecipes-example", c.cfg)
			assertLines(t, got, c.want)
		})
	}
}

// <setting>-<value> is a flat namespace: two settings whose names are prefixes
// of one another can produce one key for two values, and the engine keeps
// whichever came last. Nothing else catches that.
func TestCheckLocaleFindsCollidingValueKeys(t *testing.T) {
	lib := New()
	lib.DropdownSettingNeedingLocale("quench", "medium-oil", []string{"medium-oil"})
	lib.DropdownSettingNeedingLocale("quench-medium", "oil", []string{"oil"})

	cfg := `[mod-setting-name]
fkrecipes-example-quench=Quench
fkrecipes-example-quench-medium=Quenching medium

[string-mod-setting]
fkrecipes-example-quench-medium-oil=Oil
`
	assertLines(t, lib.CheckLocale("fkrecipes-example", cfg), []string{
		"the dropdown values fkrecipes-example-quench/medium-oil and fkrecipes-example-quench-medium/oil both produce the [string-mod-setting] key fkrecipes-example-quench-medium-oil",
	})
}

// A wrong mod name is a wrong prefix for every key at once, which is the loud
// failure the doc comment promises rather than a subtle one.
func TestCheckLocaleWithTheWrongModNameFailsEverything(t *testing.T) {
	cfg := readTestdata(t, "../testdata/locale/example.cfg")
	findings := steelworksSettings().CheckLocale("steelworks", cfg)

	if len(findings) < 6 {
		t.Fatalf("a wrong mod name produced only %d findings", len(findings))
	}
	for _, f := range findings[:6] {
		if !strings.Contains(f, "steelworks-") {
			t.Errorf("finding does not name the wrong prefix: %s", f)
		}
	}
}

// A key redefined to blank is EXACTLY how a blank sneaks into a file, and the
// engine keeps the last one. Both findings have to fire: the duplicate that
// explains it, and the unreadable entry the player would actually meet.
func TestCheckLocaleReadsTheLastOfADuplicate(t *testing.T) {
	cases := []struct {
		name  string
		cfg   string
		about string
		want  []string
	}{
		{
			name:  "a setting name redefined to blank",
			about: "hardened-tools",
			cfg: `[mod-setting-name]
fkrecipes-example-hardened-tools=Hardened tools
fkrecipes-example-hardened-tools=
`,
			want: []string{
				"the [mod-setting-name] entry fkrecipes-example-hardened-tools is defined twice; the engine keeps the last one",
				"the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
			},
		},
		{
			name:  "a dropdown value redefined to blank",
			about: "water",
			cfg: `[string-mod-setting]
fkrecipes-example-quench-medium-water=Water
fkrecipes-example-quench-medium-water=
`,
			want: []string{
				"the [string-mod-setting] entry fkrecipes-example-quench-medium-water is defined twice; the engine keeps the last one",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
			},
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got := steelworksSettings().CheckLocale("fkrecipes-example", c.cfg)
			// Only the two findings this shape is about; the other settings
			// are missing for the ordinary reason and are not the point.
			var kept []string
			for _, f := range got {
				if strings.Contains(f, c.about) {
					kept = append(kept, f)
				}
			}
			assertLines(t, kept, c.want)
		})
	}
}

// The mis-cased section header: the entry is right there in the file, and a
// bare "has no entry" sends the reader looking for something they can see.
//
// The hint is SCOPED to sections that call themselves settings sections. A
// content section that happens to hold a colliding key is not an explanation
// for anything, and offering it as one would be worse than saying nothing.
func TestCheckLocaleNamesTheSectionAMissingKeySitsUnder(t *testing.T) {
	cases := []struct {
		name    string
		section string
		want    string
	}{
		{
			name:    "the mis-cased header",
			section: "Mod-Setting-Name",
			want:    "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry, though one sits under [Mod-Setting-Name]",
		},
		{
			name:    "the plural typo",
			section: "mod-settings-name",
			want:    "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry, though one sits under [mod-settings-name]",
		},
		{
			name:    "a name entry filed under the description section",
			section: "mod-setting-description",
			want:    "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry, though one sits under [mod-setting-description]",
		},
		{
			// A content section is none of this checker's business, and an
			// item sharing a name with a setting is a coincidence rather than
			// an explanation.
			name:    "a content section holding a colliding key",
			section: "item-name",
			want:    "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			cfg := "[" + c.section + "]\nfkrecipes-example-hardened-tools=Hardened tools\n"
			got := steelworksSettings().CheckLocale("fkrecipes-example", cfg)
			if len(got) == 0 || got[0] != c.want {
				t.Errorf("\n got: %v\nwant: %s", got, c.want)
			}
		})
	}
}

// A byte order mark is reported ONCE and then stripped. How the engine treats
// one has not been measured, so it is never blessed silently; leaving it in
// would make the first key unreadable and bury the encoding mistake under
// findings about entries that are fine.
func TestCheckLocaleReportsAByteOrderMarkOnce(t *testing.T) {
	cfg := "\ufeff[mod-setting-name]\nfkrecipes-example-hardened-tools=Hardened tools\n"
	got := steelworksSettings().CheckLocale("fkrecipes-example", cfg)

	if len(got) == 0 || got[0] != "the file begins with a byte order mark" {
		t.Fatalf("the byte order mark was not reported first: %v", got)
	}
	for _, f := range got[1:] {
		if strings.Contains(f, "hardened-tools") {
			t.Errorf("the mark cascaded into a finding about a good entry: %s", f)
		}
	}
}

func TestCheckLocaleReportsMalformedShapes(t *testing.T) {
	cfg := `[]
[mod-setting-name]
=Hardened tools
fkrecipes-example-orphan =Trailing space in the key
`
	got := steelworksSettings().CheckLocale("fkrecipes-example", cfg)
	want := []string{
		"the locale section header [] names no section",
		"the locale line =Hardened tools has no key before its =",
	}
	for i, w := range want {
		if i >= len(got) || got[i] != w {
			t.Errorf("finding %d\n got: %v\nwant: %s", i, got, w)
		}
	}
	// The whitespace in a key is invisible in prose, so it is quoted.
	found := false
	for _, f := range got {
		if f == `the [mod-setting-name] entry "fkrecipes-example-orphan " matches no setting this plan declares` {
			found = true
		}
	}
	if !found {
		t.Errorf("the key with trailing whitespace was not quoted: %v", got)
	}
}

// A generated or badly encoded file produces one finding per line, and a
// thousand sentences help nobody read the first.
//
// THE ADVISORIES ARE COUNTED SEPARATELY AND ARE NOT HERE AT ALL: they are a
// report of their own with a cap of their own, so a file with a thousand
// orphans loses none of its findings to a note and none of its notes to a
// finding. TestCheckLocaleAdvisoriesAreCapped is the other budget.
func TestCheckLocaleCapsItsFindings(t *testing.T) {
	// everyNameMissing comes first, so its length is what the orphan count is
	// measured from: a report is that many findings plus one per orphan.
	reportWithOrphans := func(n int) []string {
		var b strings.Builder
		b.WriteString("[mod-setting-name]\n")
		for i := 0; i < n; i++ {
			b.WriteString("fkrecipes-example-orphan-" + strconv.Itoa(i) + "=Left behind\n")
		}
		return steelworksSettings().CheckLocale("fkrecipes-example", b.String())
	}
	got := reportWithOrphans(150)
	if len(got) != localeFindingCap+1 {
		t.Fatalf("got %d findings, want the cap plus one closing line", len(got))
	}
	if got[len(got)-1] != "(and 95 more findings)" {
		t.Errorf("the closing line is %q", got[len(got)-1])
	}

	// The boundary: exactly one finding past the cap reads as one.
	got = reportWithOrphans(localeFindingCap + 1 - len(everyNameMissing))
	if len(got) != localeFindingCap+1 {
		t.Fatalf("got %d findings at the boundary", len(got))
	}
	if got[len(got)-1] != "(and 1 more finding)" {
		t.Errorf("the closing line is %q", got[len(got)-1])
	}

	// One below it is not capped at all, so no closing line is added.
	got = reportWithOrphans(localeFindingCap - len(everyNameMissing))
	if len(got) != localeFindingCap {
		t.Fatalf("got %d findings just below the cap", len(got))
	}
	if strings.HasPrefix(got[len(got)-1], "(and ") {
		t.Errorf("an uncapped report gained a closing line: %q", got[len(got)-1])
	}
}

// ---------------------------------------------------------------------------
// CheckLocaleWith: the complete-list orphan rule.
// ---------------------------------------------------------------------------

// bbbPlan is the migration pilot's shape: two legacy dropdowns declared here,
// and a third setting the mod declares itself.
func bbbPlan() *Lib {
	lib := New()
	lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
		[]string{"vanilla", "cheap"}, "a")
	lib.LegacyDropdownSettingNeedingLocale("bbb-tech-cost", "logistics",
		[]string{"logistics", "logistics-2"}, "b")
	return lib
}

const bbbCfg = `[mod-setting-name]
bbb-recipe-cost=Recipe cost
bbb-tech-cost=Research cost
bbb-multi-edge-parts=Multi-edge parts
bbb-renamed-away=Left over from a rename

[string-mod-setting]
bbb-recipe-cost-vanilla=Vanilla
bbb-recipe-cost-cheap=Cheap
bbb-tech-cost-logistics=Logistics
bbb-tech-cost-logistics-2=Logistics 2
`

// The GAP THIS PARAMETER EXISTS FOR, stated as the difference between the two
// calls over one file. Neither the hand-rolled name nor the leftover carries
// the mod prefix, so the plain call cannot see either of them.
func TestPlainCheckLocaleCannotSeeUnprefixedEntries(t *testing.T) {
	assertLines(t, bbbPlan().CheckLocale("better-belt-balancer", bbbCfg), nil)
}

// Told what the mod declares elsewhere, the same file gives up the leftover
// and stays quiet about the hand-rolled one.
func TestCheckLocaleWithPolicesTheCompleteList(t *testing.T) {
	assertLines(t, bbbPlan().CheckLocaleWith("better-belt-balancer", bbbCfg,
		[]string{"bbb-multi-edge-parts"}), []string{
		"the [mod-setting-name] entry bbb-renamed-away matches no setting this plan declares",
	})
}

// The list is what suppresses the hand-rolled entry, so leaving it out reports
// that entry too. This is the other side of the test above: without it the
// pair could both pass on a checker that reported nothing at all.
func TestCheckLocaleWithReportsAnUnlistedHandRolledEntry(t *testing.T) {
	assertLines(t, bbbPlan().CheckLocaleWith("better-belt-balancer", bbbCfg, nil), []string{
		"the [mod-setting-name] entry bbb-multi-edge-parts matches no setting this plan declares",
		"the [mod-setting-name] entry bbb-renamed-away matches no setting this plan declares",
	})
}

// The prefix-only FALSE POSITIVE, closed: a hand-rolled setting that happens
// to carry the mod prefix is an orphan to the plain call and is not one here.
func TestCheckLocaleWithClearsAPrefixedHandRolledName(t *testing.T) {
	lib := New()
	lib.BoolSetting("hardened-tools", true)
	cfg := `[mod-setting-name]
steelworks-hardened-tools=Hardened tools
steelworks-written-by-hand=Written by hand
`
	assertLines(t, lib.CheckLocale("steelworks", cfg), []string{
		"the [mod-setting-name] entry steelworks-written-by-hand matches no setting this plan declares",
	})
	assertLines(t, lib.CheckLocaleWith("steelworks", cfg,
		[]string{"steelworks-written-by-hand"}), nil)
}

// The list names what this plan does NOT declare, so a name in both is a
// contradiction and is said first, before any verdict that would rest on it.
func TestCheckLocaleWithRefusesAContradictoryList(t *testing.T) {
	assertLines(t, bbbPlan().CheckLocaleWith("better-belt-balancer", bbbCfg,
		[]string{"bbb-recipe-cost", "bbb-multi-edge-parts"}), []string{
		"the hand-rolled name bbb-recipe-cost is also a setting this plan declares; the list names only settings declared outside this library",
		"the [mod-setting-name] entry bbb-renamed-away matches no setting this plan declares",
	})
}

// THE MISSING DIRECTION IS UNCHANGED, and that is the point of the parameter
// suppressing orphans without creating obligations: this library knows a
// hand-rolled setting's NAME and nothing else, so it cannot say what entries
// that setting needs. The declared settings are still held to theirs.
func TestCheckLocaleWithLeavesTheMissingDirectionAlone(t *testing.T) {
	cfg := `[mod-setting-name]
bbb-recipe-cost=Recipe cost

[string-mod-setting]
bbb-recipe-cost-vanilla=Vanilla
bbb-recipe-cost-cheap=Cheap
`
	assertLines(t, bbbPlan().CheckLocaleWith("better-belt-balancer", cfg,
		[]string{"bbb-multi-edge-parts"}), []string{
		"the setting bbb-tech-cost has no [mod-setting-name] entry",
		"the dropdown setting bbb-tech-cost has no [string-mod-setting] entry for its value logistics",
		"the dropdown setting bbb-tech-cost has no [string-mod-setting] entry for its value logistics-2",
	})
}

// The VALUE direction is unchanged too: another mod's string setting in the
// same file is not this checker's business whether or not a complete list was
// given, because a name alone says nothing about what values a setting offers.
func TestCheckLocaleWithLeavesForeignValueKeysAlone(t *testing.T) {
	cfg := `[mod-setting-name]
bbb-recipe-cost=Recipe cost
bbb-tech-cost=Research cost
bbb-multi-edge-parts=Multi-edge parts

[string-mod-setting]
bbb-recipe-cost-vanilla=Vanilla
bbb-recipe-cost-cheap=Cheap
bbb-tech-cost-logistics=Logistics
bbb-tech-cost-logistics-2=Logistics 2
bbb-multi-edge-parts-aggressive=Aggressive
`
	assertLines(t, bbbPlan().CheckLocaleWith("better-belt-balancer", cfg,
		[]string{"bbb-multi-edge-parts"}), nil)
}

// THE COMPOSITION'S CONDITION IS ONE FUNCTION AND THIS IS WHAT HOLDS ITS
// CLAUSES. costDropdownComposesPresetLines decides, for a technology, whether
// its cost dropdown has a preset list composed onto its description, and THREE
// readers ask it: the composition in settingDescriptions, the checker's
// required-description rule through dropdownsWithComposedDescription, and the
// advisory walk. One spelling means one place to break, and this is the test
// that notices.
//
// IT IS HERE BECAUSE CONSOLIDATION ALONE DID NOT COVER THEM, which was measured
// rather than assumed: with the three spellings reduced to one, dropping
// l.validPacksSetting, l.validDropdownSetting or the namedCostSources clause
// each left `go test ./...` green, so the required-description findings do NOT
// exercise these guards and no fixture in the suite reaches them.
//
// A HANDLE FROM ANOTHER PLAN IS THE SHAPE, because it is the one an author
// actually produces and the one every validator in this library already names.
// The checker validates nothing by design, so it must SKIP such a declaration:
// it may not demand a description for a preset list nobody composes, it may not
// say anything about the game's keys in lines nobody emits, and on the dropdown
// arm it may not follow the handle at all, which would be an index of -1.
func TestACostDropdownWithAHandleFromAnotherPlanComposesNothing(t *testing.T) {
	other := New()
	foreignDropdown := other.DropdownSettingNeedingLocale("tier", "a", []string{"a", "b"})
	foreignPacks := other.PacksSetting("packs", []Pack{{Name: "automation-science-pack", Amount: 1}})

	build := func(useForeignDropdown bool) *Lib {
		lib := New()
		tier := lib.DropdownSettingNeedingLocale("tier", "a", []string{"a", "b"})
		packs := lib.PacksSetting("packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
		count := lib.IntSetting("count", 0, Between(0, 100000))
		seconds := lib.IntSetting("seconds", 0, Between(0, 600))
		if useForeignDropdown {
			tier = foreignDropdown
		} else {
			packs = foreignPacks
		}
		lib.Technology("hardened-tips", TechSpec{
			CostBy: &CostChoices{
				Setting: tier,
				Choices: []CostChoice{
					{Value: "a", Sources: []string{"logistics"}},
					{Value: "b", Sources: []string{"military-4"}},
				},
				Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
			},
			CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
		})
		return lib
	}

	for _, c := range []struct {
		name    string
		foreign bool
	}{
		{"the dropdown handle is another plan's", true},
		{"the packs handle is another plan's", false},
	} {
		t.Run(c.name, func(t *testing.T) {
			lib := build(c.foreign)
			if got := lib.CheckLocaleAdvisories("mymod"); len(got) != 0 {
				t.Errorf("a composition nobody emits produced advisories:\n%s", strings.Join(got, "\n"))
			}
			for _, f := range lib.CheckLocale("mymod", "") {
				if strings.Contains(f, "composes its preset list onto that entry") {
					t.Errorf("a preset list nobody composes was required: %s", f)
				}
			}
		})
	}
}
