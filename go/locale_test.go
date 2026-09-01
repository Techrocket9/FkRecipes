package fkrecipes

import (
	"os"
	"strconv"
	"strings"
	"testing"
)

// steelworksSettings declares the settings the example guests declare, in the
// same order and with the same values. The checker reads settings and nothing
// else, so the example's items, recipes and technologies are not repeated
// here; what has to match is every name a locale key is built from.
func steelworksSettings() *Lib {
	lib := New()
	lib.BoolSetting("hardened-tools", true)
	lib.IntSetting("rivet-batch", 4, Between(1, 20))
	lib.DoubleSetting("forging-time", 3, NumericSpec{HasMax: true, Max: 120})
	lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
	lib.BoolSetting("bonus-research", true)
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

// A file with every entry the plan needs and nothing it does not.
func TestCheckLocaleAcceptsACompleteFile(t *testing.T) {
	cfg := `[mod-setting-name]
fkrecipes-example-hardened-tools=Hardened tools
fkrecipes-example-rivet-batch=Rivets per batch
fkrecipes-example-forging-time=Forging time
fkrecipes-example-quench-medium=Quenching medium
fkrecipes-example-bonus-research=Bonus research

[mod-setting-description]
fkrecipes-example-forging-time=Seconds to quench and temper one plate.

[string-mod-setting]
fkrecipes-example-quench-medium-water=Water
fkrecipes-example-quench-medium-oil=Oil
`
	if findings := steelworksSettings().CheckLocale("fkrecipes-example", cfg); len(findings) != 0 {
		t.Errorf("a complete file produced findings:\n%s", strings.Join(findings, "\n"))
	}
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
			want: []string{
				"the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
				"the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
				"the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
				"the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
			},
		},
		{
			// Present but blank renders as nothing, which is the defect the
			// player reports, so it counts as missing.
			name: "an entry that is present but empty",
			cfg: `[mod-setting-name]
fkrecipes-example-hardened-tools=
`,
			want: []string{
				"the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
				"the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
				"the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
				"the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
				"the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
			},
		},
		{
			name: "a description entry matching nothing",
			cfg: `[mod-setting-description]
fkrecipes-example-scrap-recovery=Left behind by a rename.
`,
			want: []string{
				"the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
				"the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
				"the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
				"the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
				"the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
				"the [mod-setting-description] entry fkrecipes-example-scrap-recovery matches no setting this plan declares",
			},
		},
		{
			// Another mod's string setting shares the section and is none of
			// this checker's business.
			name: "a value entry under a setting this plan does not own",
			cfg: `[string-mod-setting]
someothermod-belt-tier-express=Express
`,
			want: []string{
				"the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
				"the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
				"the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
				"the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
				"the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
			},
		},
		{
			name: "a line that is neither a section nor an entry",
			cfg: `[mod-setting-name]
this line has no equals sign
`,
			want: []string{
				"the locale line this line has no equals sign is neither a section nor an entry",
				"the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
				"the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
				"the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
				"the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
				"the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
				"the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
			},
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
func TestCheckLocaleCapsItsFindings(t *testing.T) {
	// 5 settings with no name and 2 dropdown values with no entry come before
	// the orphans, so the orphan count sets how far past the cap a report
	// lands.
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
	if got[len(got)-1] != "(and 57 more findings)" {
		t.Errorf("the closing line is %q", got[len(got)-1])
	}

	// The boundary: exactly one finding past the cap reads as one.
	got = reportWithOrphans(94)
	if len(got) != localeFindingCap+1 {
		t.Fatalf("got %d findings at the boundary", len(got))
	}
	if got[len(got)-1] != "(and 1 more finding)" {
		t.Errorf("the closing line is %q", got[len(got)-1])
	}

	// One below it is not capped at all, so no closing line is added.
	got = reportWithOrphans(93)
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
