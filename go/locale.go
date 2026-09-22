package fkrecipes

import (
	"strconv"
	"strings"
)

// localeFindingCap is how many problems a report names before it stops. A
// generated or badly encoded file can produce one finding per line, and a
// thousand sentences help nobody read the first. The cycle path is capped for
// the same reason and in the same shape.
const localeFindingCap = 100

// localeAdvisoryCap is the same ceiling over the ADVISORIES, which are a
// report of their own: see CheckLocaleAdvisories. Two caps rather than one is
// what keeps the promise that an advisory can never displace a finding, and the
// separate accessor is what keeps it from being one.
const localeAdvisoryCap = 100

// CheckLocale reads a mod's .cfg and reports what a player would not be able
// to read, in both directions. It returns one sentence per problem and an
// empty slice for a clean file.
//
// A MISSING LOCALE KEY IS A FIELD REPORT WAITING TO HAPPEN. The shape is
// BetterBeltBalancer's, which learned it the hard way: a live session in 2026
// produced `Unknown key: "entity-name.bbb-linked-belt"` out of the engine's own
// "X is in the way" message, and the fix was four lines of .cfg nobody had
// thought to write. A dropdown is the same defect with more ways to reach it,
// because a string setting renders each VALUE from its own
// [string-mod-setting] entry and there is no fallback: the player stands in
// the settings menu reading `Unknown key: "string-mod-setting.mymod-x-y"`.
//
// IT CHECKS BOTH DIRECTIONS. A missing entry is a value the player cannot
// read. An ORPHAN entry is a value that used to exist, which means an option
// was renamed and one of the two places was not.
//
// IT RUNS ON THE HOST, and it has to: --dump-data does not read locale, and
// nothing headless opens a settings menu. This is the only place the check can
// live, which is why it is a library function rather than a stage-time
// refusal.
//
// THE MOD NAME IS A PARAMETER, AND THAT IS A DRIFT RISK WORTH STATING. Every
// other prefix in this library is derived from fkdata.ModName at emit, where
// it cannot disagree with the packaged mod. A host test has no fkdata to ask,
// so the consumer's test passes the name their fklua.toml packages under, and
// a wrong name is a wrong prefix for every key at once: the result is every
// setting reported missing and every entry reported orphaned, which is loud
// rather than subtle.
//
// WHAT IT POLICES is the mod-prefix namespace plus this plan's own declared
// names, legacy names included. In [string-mod-setting] only keys under one of
// this plan's dropdown settings are considered, so another setting's values
// are not this checker's business; a legacy dropdown is policed under the name
// it actually carries, so a stale value key beneath it IS caught. In
// [mod-setting-name] and [mod-setting-description] the orphan arm fires only
// on keys CARRYING THE MOD PREFIX, so an entry that matches no declared
// setting and carries no prefix is invisible here: a renamed legacy setting's
// leftover entry goes unreported, and so does a setting the consumer wrote by
// hand under a name of its own. That arm cannot be widened on a GUESS, because
// a stale legacy name and a deliberately hand-rolled one are the same string
// to this function. CheckLocaleWith is the version that widens it on a FACT:
// told what the mod declares elsewhere, it polices those two sections against
// the complete set of the mod's setting names instead of against the prefix.
//
// A PROTOTYPE'S OWN DESCRIPTION KEY IS NEVER ASKED FOR, and that is a third
// class beside REQUIRED and ADVISORY rather than an omission. The data stage
// composes recipe-description.<emitted name> and technology-description.<name>
// onto a prototype carrying a note with no declared Description, and both are
// inside the mod's prefix, which by the rule above would make them REQUIRED.
// They must not be: they are the AUTHOR'S OWN OPTIONAL entry, referenced
// precisely so an author who wrote one keeps it, and requiring them would make
// every consumer owe a description for every recipe and technology they emit in
// a report docs/migration.md tells them to assert is EMPTY. They are not
// advisory either, because an advisory says the key is somebody else's and
// these are the consumer's own. This function walks SETTINGS and never recipes
// or technologies, so the silence is structural: there is no exclusion to
// forget. See descriptionRef.
//
// A DESCRIPTION IS OPTIONAL HERE, AND THAT IS A DELIBERATE DIVERGENCE from
// BetterBeltBalancer, which requires one. The engine's failure mode for a
// missing description is a lost tooltip, not an `Unknown key` render in the
// player's face, so it is not the defect this tripwire exists for. A
// description that names nothing is still reported: a renamed setting leaves
// one behind exactly as it leaves a name behind.
func (l *Lib) CheckLocale(modName string, cfg string) []string {
	return l.checkLocale(modName, cfg, nil, false)
}

// CheckLocaleWith is CheckLocale told what this mod declares OUTSIDE this
// library, which is what lets the name and description orphan scan be
// COMPLETE rather than prefix-shaped.
//
// handRolled is the consumer's whole set of settings declared elsewhere: the
// ones written straight into an fk_settings hook beside Emit, under whatever
// names they carry. Given that list, this function knows every setting name
// the mod has, so an entry under [mod-setting-name] or
// [mod-setting-description] matching no declared, legacy or hand-rolled name
// is an orphan REGARDLESS OF PREFIX. That closes both halves of the gap the
// prefix rule leaves: a renamed legacy setting's leftover entry carries no
// prefix and is now caught, and a hand-rolled setting that happens to carry
// the mod prefix is no longer reported as an orphan for existing.
//
// THE LIST SUPPRESSES ORPHANS; IT DOES NOT CREATE OBLIGATIONS. A name in it
// is not reported missing, because this library knows the name and nothing
// else: whether that setting is a dropdown needing per-value entries, or a
// runtime-global one, or a bool, is the consumer's business. The value
// direction is unchanged for the same reason, so another mod's string setting
// in the same file is still left alone.
//
// AN EMPTY LIST IS NOT THE PLAIN CALL. It is the assertion that this mod
// declares nothing outside this library, so every [mod-setting-name] and
// [mod-setting-description] entry in the file must match a declared or legacy
// setting and anything else is an orphan, another mod's entry in the same file
// included. That is the strictest reading available and it is the right one
// for a mod that declares everything here; use CheckLocale when you have not
// enumerated the rest, because it is the call that assumes nothing.
//
// A name in the list that this plan also declares is a contradiction rather
// than a fact about the file, and is reported FIRST in the checker's own
// voice: the list is by definition what this plan does not declare, so one of
// the two is wrong and no orphan verdict over that name would mean anything.
func (l *Lib) CheckLocaleWith(modName string, cfg string, handRolled []string) []string {
	return l.checkLocale(modName, cfg, handRolled, true)
}

// checkLocale is both entry points. complete says whether handRolled is the
// authoritative rest of the mod's settings; without it the name and
// description orphan rule can only be the mod prefix.
func (l *Lib) checkLocale(modName string, cfg string, handRolled []string, complete bool) []string {
	prefix := modName + "-"

	// (0) THE LIBRARY'S OWN LINES, AHEAD OF EVERY FINDING ABOUT THE FILE. This
	// is the one rule here that is not about the author's .cfg at all, so it
	// goes first and it goes once. See checkComposedTextLines.
	findings := l.checkComposedTextLines(prefix)
	sections, parsed := parseLocale(cfg)
	findings = append(findings, parsed...)

	// The contradiction first, before anything reads the list as truth.
	if complete {
		for _, n := range handRolled {
			if l.declaresSetting(prefix, n) {
				findings = append(findings, "the hand-rolled name "+localeShow(n)+
					" is also a setting this plan declares; the list names only settings declared outside this library")
			}
		}
	}

	// (1) and (2): what the player cannot read, in DECLARATION order, and a
	// setting's own name before the description it needs and the values it
	// offers.
	composed := l.dropdownsWithComposedDescription()
	numbers := l.researchNumberSettings()
	for i, s := range l.settings {
		full := s.emittedName(prefix)
		if !localeHas(sections, "mod-setting-name", full) {
			findings = append(findings, "the setting "+full+" has no [mod-setting-name] entry"+
				elsewhere(sections, "mod-setting-name", full))
		}
		// A TEXT SETTING'S DESCRIPTION IS REQUIRED, and it is the one place a
		// description is. Everywhere else a missing one costs a tooltip; here
		// it costs the player the whole tooltip, because the library composes
		// the declared list, the format and its length limit, the field that
		// decides while this one says the reserved word, the fallback, and on
		// every field that is the one to disclose it the ladder, onto that
		// entry: an absent one loses all of them along with whatever the
		// consumer meant to say. What the consumer's own entry is FOR is
		// therefore what the setting is, not how to fill it in, and the
		// sentence says so.
		//
		// NO elsewhere HINT ON THESE TWO. That hint names another section
		// holding the same key, and a setting with a perfectly good
		// [mod-setting-name] entry would then be told its description "sits
		// under [mod-setting-name]" every single time. The hint earns its keep
		// where the key appears in ONE settings section and the reader cannot
		// see why it is missing; here it would fire on nearly every finding.
		// A SETTING THE PLAN DESCRIBES INLINE NEEDS NO ENTRY AT ALL, of any
		// of the three kinds whose entry is otherwise required: the engine
		// shows the prototype's own field and the entry is never read. It is
		// reported in the orphan walk below instead, as dead text, which is
		// what it is.
		if s.described {
			continue
		}
		if s.kind.isText() {
			if !localeHas(sections, "mod-setting-description", full) {
				findings = append(findings, "the setting "+full+
					" has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for;"+
					" the library composes the format, the limits and the fallback onto it")
			}
			continue
		}
		// A RESEARCH NUMBER'S DESCRIPTION IS REQUIRED FOR THE SAME REASON a
		// text setting's is: the library composes the range and what 0 means
		// onto that entry, and an absent one loses both along with whatever the
		// consumer meant to say about what the number is for.
		if numbers[i].bound {
			if !localeHas(sections, "mod-setting-description", full) {
				findings = append(findings, "the setting "+full+
					" has no [mod-setting-description] entry, and a research number needs one to say what the number is for;"+
					" the library composes the range onto it")
			}
			continue
		}
		if s.kind != settingDropdown {
			continue
		}
		// A dropdown the library composes anything onto has its description
		// stop being optional, and WHICH SENTENCE says so is the shape: a
		// dropdown with a text setting beside it loses a preset list, and a
		// bare ingredient dropdown loses the one line about the ladder. Saying
		// "its preset list" of the second would name a list that dropdown
		// composes nothing of.
		if composed[i] != composesNothing && !localeHas(sections, "mod-setting-description", full) {
			lost := "the library composes its preset list onto that entry"
			if composed[i] == composesLadderOnly {
				lost = "the library composes onto that entry the line saying what a name this game does not have costs the list"
			}
			findings = append(findings, "the dropdown setting "+full+
				" has no [mod-setting-description] entry, and "+lost)
		}
		for _, v := range s.values {
			key := full + "-" + v
			if !localeHas(sections, "string-mod-setting", key) {
				findings = append(findings,
					"the dropdown setting "+full+" has no [string-mod-setting] entry for its value "+v+
						elsewhere(sections, "string-mod-setting", key))
			}
		}
	}

	// (3): entries that match nothing, in FILE order.
	for i := range sections {
		sec := &sections[i]
		for _, e := range sec.entries {
			switch sec.name {
			case "mod-setting-name", "mod-setting-description":
				// COMPLETE-LIST policing when the caller supplied the rest of
				// the mod's settings, prefix-shaped policing when it did not.
				// The prefix gate is not a rule anybody wants; it is the only
				// one available when the set of names is unknown.
				owned := l.declaresSetting(prefix, e.key) || nameListed(handRolled, e.key)
				scanned := complete || strings.HasPrefix(e.key, prefix)
				if scanned && !owned {
					findings = append(findings, "the ["+localeShow(sec.name)+"] entry "+
						localeShow(e.key)+" matches no setting this plan declares")
					continue
				}
				// DEAD TEXT, AND A FINDING RATHER THAN AN ADVISORY. The entry
				// is inside the mod's own prefix, somebody will edit it
				// expecting it to render, and the engine renders the
				// prototype's own localised_description over it. An advisory
				// is for a key OUTSIDE the prefix that must not be defined;
				// this is one inside it that is defined and does nothing.
				if sec.name == "mod-setting-description" && l.describesInline(prefix, e.key) {
					findings = append(findings, "the [mod-setting-description] entry "+localeShow(e.key)+
						" is never shown; the plan describes that setting inline and the engine shows the plan's description instead")
				}
			case "string-mod-setting":
				if l.policesValueKey(prefix, e.key) && !l.declaresValue(prefix, e.key) {
					findings = append(findings, "the [string-mod-setting] entry "+localeShow(e.key)+
						" matches no dropdown value this plan declares")
				}
			}
		}
	}

	// (4): <setting>-<value> is a FLAT namespace, so two settings whose names
	// are prefixes of one another can produce one key for two values, and the
	// engine keeps whichever came last. Nothing catches that but this.
	type producedKey struct{ key, owner string }
	var produced []producedKey
	for _, s := range l.settings {
		if s.kind != settingDropdown {
			continue
		}
		full := s.emittedName(prefix)
		for _, v := range s.values {
			key := full + "-" + v
			owner := full + "/" + v
			for _, earlier := range produced {
				if earlier.key == key {
					findings = append(findings, "the dropdown values "+earlier.owner+" and "+owner+
						" both produce the [string-mod-setting] key "+key)
				}
			}
			produced = append(produced, producedKey{key: key, owner: owner})
		}
	}

	if len(findings) > localeFindingCap {
		rest := len(findings) - localeFindingCap
		// A singular arm, because "(and 1 more findings)" is the kind of
		// sentence that makes a reader doubt the rest of the report.
		tail := "(and " + strconv.Itoa(rest) + " more findings)"
		if rest == 1 {
			tail = "(and 1 more finding)"
		}
		findings = append(findings[:localeFindingCap], tail)
	}
	return findings
}

// CheckLocaleAdvisories is what this library has to SAY about a consumer's
// locale file without asking anything of it: one note per locale key the
// composition references from OUTSIDE the mod's own prefix, in composition
// order. It is INFORMATIONAL, and a test suite must not fail on it.
//
// IT IS NOT PART OF CheckLocale, AND THAT IS THE POINT. An empty return from
// CheckLocale means a clean file, and a consumer's suite is told in
// docs/migration.md to assert exactly that; an advisory in that return would
// be a permanent red test over a thing the consumer is told NOT to fix, which
// is the defect this separation closes. It does not depend on the .cfg at all,
// so it takes no cfg: it is a property of the PLAN.
//
// WHAT IT COVERS is today exactly technology-name.<source>, from a cost
// dropdown's preset lines. Factorio's locale namespace is FLAT and shared:
// defining technology-name.logistics-2 in this mod's own .cfg sets the
// displayed name of BASE's technology for every mod in the game. Requiring the
// key would therefore be requiring exactly the hazard CheckLocale's own
// collision scan exists to catch, so the sentence names the key, says what the
// tooltip shows where the game does not define it, and pointedly does not tell
// the consumer to define it.
//
// A MISSING KEY IS NOT A DEFECT. localeRef wraps every composed reference in
// the engine's alternatives form, so an undefined technology-name key degrades
// to the raw internal name and the tooltip survives whole; before that wrapper
// it cost the consumer the entire tooltip, silently.
//
// IT HAS A CAP OF ITS OWN, localeAdvisoryCap, with the same closing line
// CheckLocale's cap uses. Two reports, two budgets, and neither can crowd out
// the other.
func (l *Lib) CheckLocaleAdvisories(modName string) []string {
	advisories := l.composedGameKeyAdvisories(modName + "-")
	if len(advisories) > localeAdvisoryCap {
		rest := len(advisories) - localeAdvisoryCap
		// A singular arm, for the reason the findings cap has one.
		tail := "(and " + strconv.Itoa(rest) + " more advisories)"
		if rest == 1 {
			tail = "(and 1 more advisory)"
		}
		advisories = append(advisories[:localeAdvisoryCap], tail)
	}
	return advisories
}

// composedGameKeyAdvisories is CheckLocaleAdvisories' uncapped walk: one
// sentence per locale key the composition references from outside this mod's
// prefix, which today is exactly technology-name.<source>.
//
// THE ORDER IS THE COMPOSITION'S, setting by setting in declaration order and
// choice by choice within one, and it steps past exactly what the composition
// steps past, because it asks the same function: composedDropdownPresets is
// where a dropdown's presets are chosen, Describes and all, and it is the only
// spelling of that. One dropdown naming one key twice says the same sentence
// twice, so it is said once.
//
// THE ORDER MOVED FROM TECHNOLOGY ORDER TO SETTING ORDER WITH THAT, and the two
// differ only on a plan with two cost dropdowns declared in one order and
// described by technologies declared in another. Setting order is the right one
// of the two, because what each advisory is about is a setting.
func (l *Lib) composedGameKeyAdvisories(prefix string) []string {
	var out []string
	var seen []string
	for i, s := range l.settings {
		p := l.composedDropdownPresets(i + 1)
		if p == nil || p.kind != presetsCost {
			continue
		}
		full := s.emittedName(prefix)
		for _, choice := range p.cost {
			// A CHOICE CARRYING Display NAMES NO KEY AT ALL, so there is
			// nothing to advise about: the composed tail is the author's own
			// literal and the technology-name reference is not composed for
			// that line. Taking the advisory away is the second thing the
			// override is for.
			if len(choice.Sources) == 0 || choice.Display != "" {
				continue
			}
			line := gameKeyAdvisory(full, "technology-name."+choice.Sources[0], choice.Sources[0])
			if nameListed(seen, line) {
				continue
			}
			seen = append(seen, line)
			out = append(out, line)
		}
	}
	return out
}

// gameKeyAdvisory is the advisory's one sentence, and it is one function in
// each half so that the two cannot drift apart a word at a time.
func gameKeyAdvisory(full, key, raw string) string {
	return "note: the dropdown setting " + full + " composes the game's own key " + key +
		", which this plan does not own; where the game does not define it the tooltip shows " + raw +
		" instead, and defining it here would rename it for every mod"
}

// checkComposedTextLines is the DRIFT GUARD over what this library composes
// onto a text setting's description: the ladder line where that field carries
// one, the format line, the switch line and the fallback line, each reported by
// name when the composition stops carrying it.
//
// IT IS NOT AN AUTHOR FINDING, and that is why it is worded and placed the way
// it is. The consumer writes the [mod-setting-description] entry and this
// library writes everything under it, so a report here says the library shipped
// a description missing a line it owes; nothing the consumer can type puts one
// back. The checker is where it lives because the composition has no other
// reader that runs on a host: a settings-stage refusal would be a load failure
// over a tooltip, and the engine's own dump proves the shape only where
// somebody runs an engine.
//
// ONCE PER REPORT, NOT ONCE PER SETTING, and the sentence names the library
// rather than a setting. What it inspects does not vary with the setting in any
// way the rule reads: it asks for four lines at most, of which the fallback
// line is a constant, the ladder line and the format line are constants the
// KIND chooses between and the switch line is one of two sentences the
// declaration chooses between; all four arrive from the caller that built the
// composition, beside the flag saying whether the ladder line is one of them at
// all, and the only per-setting part of the composition, the consumer's own
// key, is not what it looks at. Run inside the per-setting loop it turned ONE library
// defect into one finding per text setting, five of them on the example guest,
// and at fifty text settings the sentences alone would fill localeFindingCap and
// push every author finding out of the report.
//
// FIRST IN THE REPORT, WHICH IS ALSO HOW IT SURVIVES THE CAP. The cap keeps the
// first localeFindingCap findings and replaces the tail with a count, so a
// finding emitted ahead of the parse findings and of every rule about the
// author's file cannot be dropped by a file that produces a thousand of its
// own. That is the cheaper of the two ways to keep it: a cap exemption would
// have to be carried through the truncation in both halves, and this is one
// ordering decision instead.
//
// NOTHING TO GUARD WITHOUT A TEXT SETTING. A plan that declares none composes
// no text description, so there is no shipped description for a line to have
// gone missing from, and a report about one would name a defect that plan
// cannot carry. The composition is built from the FIRST text setting in
// declaration order, so what is inspected is a description this plan really
// emits.
//
// THE LIST IS LEFT OUT OF THE COMPOSITION, and that is the one deviation from
// "check what is emitted". Rendering the declared list is the only part of
// textDescription that reaches the language, and it is also the only part that
// dereferences an item handle. The checker validates nothing, exactly as the
// rest of it validates nothing, so a plan the planners would refuse must not
// panic here: with the list left out neither the language nor l.items is
// touched, and the lines under test are the ones this function can see.
// What the rendered list itself says is the settings stage's business and the
// corpus's.
func (l *Lib) checkComposedTextLines(prefix string) []string {
	desc, switchLine, ingredients, ladder, ok := l.guardedTextDescription(prefix)
	if !ok {
		return nil
	}
	return composedTextLinesMissing(desc, switchLine, ingredients, ladder)
}

// guardedTextDescription is WHICH composition the guard inspects, split out
// from the rule so that the choice is visible to a test on its own: the first
// text setting's in declaration order, with the declared list left out, and no
// composition at all when the plan declares no text setting.
//
// THE SWITCH LINE, THE KIND AND THE LADDER FLAG COME BACK BESIDE THE
// COMPOSITION because they are the three inputs the composition is not a
// constant in. The switch line names the option above or below when a dropdown
// is bound to the same declaration and the mod's own list when none is; the
// kind decides TWO lines, the ladder line's whole vocabulary (a pack and a
// research that takes fewer of them, against a name and a shorter craft) and
// whether the format line names the word none, which only an ingredient list
// takes; and the ladder flag decides whether the ladder line is there to look
// for at all, which is the same question textCarriesLadderLine answers for the
// composition. The rule cannot recompute any of them without the
// declaration, so the caller that built the composition hands over what it
// built it with, and what the guard then answers is whether textDescription put
// those lines into the table it returned.
//
// THE FLAG IS WHAT KEEPS THE GUARD FROM ASKING FOR A LINE THAT IS NOT OWED.
// Beside an ingredient dropdown the ladder is disclosed on that dropdown
// instead, so a guard that always looked for it would report a defect on every
// plan the customizer was designed for; one that never looked for it would stop
// watching the compositions that do carry it, which is every other shape.
func (l *Lib) guardedTextDescription(prefix string) (desc Value, switchLine string, ingredients, ladder, ok bool) {
	for i, s := range l.settings {
		if s.kind.isText() {
			line := l.textSwitchLine(i)
			ing := s.kind == settingIngredients
			lad := l.textCarriesLadderLine(i)
			return textDescription(l.settingDescriptionHead(i, s.emittedName(prefix)), "", line, ing, lad), line, ing, lad, true
		}
	}
	return Value{}, "", false, false, false
}

// composedTextLinesMissing is the guard's rule over one composition.
//
// THE DESCRIPTION IS A PARAMETER so that a test can hand it the composition
// with one line taken out of it, which is the only way to see the finding
// without editing the source: nothing a consumer can declare produces a
// composition missing a line.
//
// LADDER IS THE COMPOSITION'S OWN ANSWER AND NOT A SECOND RULE. It arrives
// beside the description from the caller that built it, so the guard asks for
// exactly the lines that composition put in; deriving it here from the switch
// line's wording would be a second spelling of textCarriesLadderLine, and the
// two could then disagree about which shape they are looking at. The switch
// line cannot answer it in any case: it says a dropdown decides without saying
// which kind of dropdown, and the kind is the whole of the rule.
func composedTextLinesMissing(desc Value, switchLine string, ingredients, ladder bool) []string {
	var out []string
	// THE ORDER IS THE COMPOSITION'S OWN, so a description that lost more than
	// one of them reports them in the order a reader would have met them, and
	// the ladder line is asked for FIRST or not at all, which is where it sits.
	want := make([]struct{ line, missing string }, 0, 4)
	if ladder {
		want = append(want, struct{ line, missing string }{
			textLadderLine(ingredients), "no line about a name in the list this game does not have"})
	}
	want = append(want,
		struct{ line, missing string }{textFormatLine(ingredients), "no line about the format and the length limit"},
		struct{ line, missing string }{switchLine, "no line about which field decides while the text says default"},
		struct{ line, missing string }{textFallbackLine, "no line about what happens to a text this mod cannot use"},
	)
	for _, w := range want {
		if !localisedCarries(desc, w.line) {
			out = append(out, "the library composes "+w.missing+
				" onto a text setting's description; a text setting's description carries one,"+
				" so this is a defect in fkrecipes and not in this locale file")
		}
	}
	return out
}

// localisedCarries reports whether a composed localised string holds this exact
// string as one of its parameters, at any depth.
//
// DEPTH BECAUSE THE QUESTION IS "DOES THE PLAYER READ IT", NOT "WHERE". The
// composition it is handed is flat past the consumer's own key, which is itself
// a nested table: textDescription is at most six parameters and never reaches
// localisedGroup's nesting rule, and a dropdown's composition is never handed
// here at all. A top-level scan would therefore be a claim about the shape of
// the composition rather than about the lines, and it would go quietly wrong
// the day a line moves into a group. This asks only what the guard needs.
func localisedCarries(v Value, want string) bool {
	if v.Kind == KindStr {
		return v.Str == want
	}
	if v.Kind != KindArr {
		return false
	}
	for _, item := range v.Arr {
		if localisedCarries(item, want) {
			return true
		}
	}
	return false
}

// elsewhere names the section a missing key actually turned up in, when that
// section plausibly meant to be a settings section.
//
// This is the mis-cased or typo'd section header, which is otherwise reported
// as a plain absence and sends the reader looking for an entry they can see
// with their own eyes. THE HINT IS SCOPED, and the rule is the word "setting":
// all three sections this checker polices carry it, so a section that holds
// the key and calls itself a setting section plausibly meant to be one, while
// a content section never did. A real locale file carries [item-name],
// [entity-name] and [technology-name], and an item that happens to share a
// name with a setting must not be offered as the explanation for a missing
// setting entry.
//
// The comparison is ASCII-only on purpose: Go's Unicode lowercasing and Rust's
// are not the same function on every input, and this decision has to be the
// same in both halves.
func elsewhere(sections []localeSect, want, key string) string {
	for i := range sections {
		if sections[i].name == want {
			continue
		}
		if !strings.Contains(asciiLower(sections[i].name), "setting") {
			continue
		}
		if _, ok := sections[i].find(key); ok {
			return ", though one sits under [" + localeShow(sections[i].name) + "]"
		}
	}
	return ""
}

func asciiLower(s string) string {
	b := []byte(s)
	for i, c := range b {
		if c >= 'A' && c <= 'Z' {
			b[i] = c + ('a' - 'A')
		}
	}
	return string(b)
}

// nameListed reports whether the consumer named this setting as one of its
// own. Compared verbatim: a hand-rolled name is whatever the mod ships, and
// deriving anything from it is the guessing this parameter exists to replace.
func nameListed(handRolled []string, key string) bool {
	for _, n := range handRolled {
		if n == key {
			return true
		}
	}
	return false
}

// dropdownComposition is what a dropdown setting's description is composed of,
// which decides both WHETHER its [mod-setting-description] entry is required
// and WHICH sentence says so.
type dropdownComposition int

const (
	// composesNothing is a dropdown this library writes no description for at
	// all: a plain dropdown nothing binds, or one a validator stepped past.
	composesNothing dropdownComposition = iota
	// composesLadderOnly is a bare INGREDIENT dropdown: no text setting beside
	// it, so no language to render a preset in and no switch line, and the
	// ladder line alone. See settingDescriptions.
	composesLadderOnly
	// composesPresetLines is a dropdown with a text setting beside it, whose
	// description carries its presets written out.
	composesPresetLines
)

// dropdownsWithComposedDescription says, per setting, what the library composes
// onto that dropdown's description.
//
// AN INGREDIENT DROPDOWN IS ON THE LIST WHETHER OR NOT A TEXT SETTING SITS
// BESIDE IT. With one beside it the composition is the preset lines, the ladder
// line and the switch line; with none it is the ladder line
// alone, which is still a composition and still loses the consumer's own
// sentence when the entry is absent. That is a NEW OBLIGATION on a consumer
// shipping a bare ingredient dropdown and docs/migration.md names it.
//
// IT ASKS composedDropdownPresets RATHER THAN WALKING THE PLAN ITSELF, so the
// obligation is about the description this library really emits: which
// declaration describes a shared dropdown, Describes and all, is decided in
// that one function and read here.
func (l *Lib) dropdownsWithComposedDescription() []dropdownComposition {
	marks := make([]dropdownComposition, len(l.settings))
	for i := range l.settings {
		p := l.composedDropdownPresets(i + 1)
		switch {
		case p == nil:
		case p.kind == presetsIngredients && p.text == 0:
			marks[i] = composesLadderOnly
		default:
			marks[i] = composesPresetLines
		}
	}
	return marks
}

// describesInline reports whether this plan wrote the description of the
// setting emitted under this key, which is what makes a [mod-setting-description]
// entry for it dead text.
func (l *Lib) describesInline(prefix, key string) bool {
	for _, s := range l.settings {
		if s.emittedName(prefix) == key {
			return s.described
		}
	}
	return false
}

func (l *Lib) declaresSetting(prefix, key string) bool {
	for _, s := range l.settings {
		if s.emittedName(prefix) == key {
			return true
		}
	}
	return false
}

// policesValueKey is BetterBeltBalancer's rule: a key belongs to this checker
// only when it sits under one of THIS plan's dropdown settings. An entry for
// somebody else's string setting is not this function's business.
func (l *Lib) policesValueKey(prefix, key string) bool {
	for _, s := range l.settings {
		if s.kind == settingDropdown && strings.HasPrefix(key, s.emittedName(prefix)+"-") {
			return true
		}
	}
	return false
}

func (l *Lib) declaresValue(prefix, key string) bool {
	for _, s := range l.settings {
		if s.kind != settingDropdown {
			continue
		}
		for _, v := range s.values {
			if s.emittedName(prefix)+"-"+v == key {
				return true
			}
		}
	}
	return false
}

type localeEntry struct {
	key   string
	value string
}

type localeSect struct {
	name    string
	entries []localeEntry
	// sorted holds indexes into entries, ordered by key, so a lookup is a
	// binary search rather than a scan. A file with tens of thousands of
	// entries is a real shape (one generated file per language), and the scan
	// it replaces was quadratic in the number of entries.
	sorted []int
}

// find binary-searches the section for a key. The first result is the position
// in sorted, which is where an insert belongs when the second is false.
func (s *localeSect) find(key string) (int, bool) {
	lo, hi := 0, len(s.sorted)
	for lo < hi {
		mid := int(uint(lo+hi) >> 1)
		if s.entries[s.sorted[mid]].key < key {
			lo = mid + 1
			continue
		}
		hi = mid
	}
	if lo < len(s.sorted) && s.entries[s.sorted[lo]].key == key {
		return lo, true
	}
	return lo, false
}

// localeHas reports an entry a player would actually read. An entry that is
// present but blank renders as nothing, which is the same defect as an absent
// one, so it counts as missing: BetterBeltBalancer's rule.
func localeHas(sections []localeSect, section, key string) bool {
	for i := range sections {
		if sections[i].name != section {
			continue
		}
		pos, ok := sections[i].find(key)
		if !ok {
			return false
		}
		return strings.TrimSpace(sections[i].entries[sections[i].sorted[pos]].value) != ""
	}
	return false
}

// localeShow quotes a key or section name whose whitespace would otherwise be
// invisible in the sentence. Everything else is rendered bare, so the ordinary
// finding reads as prose.
func localeShow(s string) string {
	if s != strings.TrimSpace(s) {
		return `"` + s + `"`
	}
	return s
}

// parseLocale reads Factorio's .cfg grammar, which is INI without quoting.
//
// THE SEMANTICS ARE BetterBeltBalancer's, and they are the engine's:
//
//   - a line is trimmed of surrounding whitespace before anything else;
//   - a blank line is skipped, and so is one starting with # or ; (the comment
//     markers, at the START of a line only: neither one comments out the rest
//     of an entry);
//   - [name] opens a section, and entries before any section header belong to
//     no section, which is where the engine ignores them;
//   - key=value splits at the FIRST = and the value may contain more;
//   - a key defined twice OVERWRITES: the engine keeps the last one, so the
//     last value is what the presence check reads. Redefining a key to blank
//     is exactly how a blank sneaks into a file, and it has to be visible as
//     both a duplicate AND an unreadable entry;
//   - the key is NOT trimmed around the =, which is not sloppiness: Factorio
//     takes everything before the first = as the key, so `name = X` really
//     does declare a key with a trailing space, and a checker that trimmed it
//     would pass a file the game reads differently.
//
// Anything else is a finding rather than a silent skip: a line the parser does
// not understand is a line the player does not get.
func parseLocale(cfg string) ([]localeSect, []string) {
	var sections []localeSect
	var findings []string
	current := -1

	// A BOM is REPORTED AND THEN STRIPPED. How the engine treats one here has
	// not been measured, so the checker may not silently bless it; and leaving
	// it in place would make the first key unreadable and turn one encoding
	// mistake into a cascade of findings about entries that are perfectly
	// fine.
	if strings.HasPrefix(cfg, "\ufeff") {
		findings = append(findings, "the file begins with a byte order mark")
		cfg = strings.TrimPrefix(cfg, "\ufeff")
	}

	for _, raw := range strings.Split(cfg, "\n") {
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, ";") {
			continue
		}
		if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
			name := line[1 : len(line)-1]
			if name == "" {
				// The current section is left alone rather than replaced by a
				// nameless one: the entries that follow still belong where the
				// last real header put them.
				findings = append(findings, "the locale section header [] names no section")
				continue
			}
			current = -1
			for i := range sections {
				if sections[i].name == name {
					current = i
					break
				}
			}
			if current < 0 {
				sections = append(sections, localeSect{name: name})
				current = len(sections) - 1
			}
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if !ok {
			findings = append(findings, "the locale line "+localeShow(line)+
				" is neither a section nor an entry")
			continue
		}
		if key == "" {
			findings = append(findings, "the locale line "+localeShow(line)+
				" has no key before its =")
			continue
		}
		if current < 0 {
			findings = append(findings, "the locale entry "+localeShow(key)+
				" sits before any section header")
			continue
		}

		sec := &sections[current]
		pos, dup := sec.find(key)
		if dup {
			findings = append(findings, "the ["+localeShow(sec.name)+"] entry "+localeShow(key)+
				" is defined twice; the engine keeps the last one")
			sec.entries[sec.sorted[pos]].value = value
			continue
		}
		sec.entries = append(sec.entries, localeEntry{key: key, value: value})
		sec.sorted = append(sec.sorted, 0)
		copy(sec.sorted[pos+1:], sec.sorted[pos:])
		sec.sorted[pos] = len(sec.entries) - 1
	}
	return sections, findings
}

// LocaleEntry is one key and value from a .cfg, with the section it sat under.
type LocaleEntry struct {
	Section string
	Key     string
	Value   string
}

// LocaleEntries parses a .cfg into its entries, in FILE ORDER, for a
// consumer's own assertions.
//
// CheckLocale answers the questions this library can ask, which are the ones
// about the settings it generated. A consumer has questions of their own: that
// an entity name entry exists for their hand-rolled entity, that no key is
// blank, that a translation file carries the same keys as the English one.
// Writing a second .cfg parser to ask them is the kind of duplication that
// drifts, so this is the same parser's output, exported.
//
// PARSE FINDINGS ARE NOT REPORTED HERE. A malformed line is skipped exactly as
// CheckLocale skips it; run CheckLocale for the diagnosis. This returns what
// the file says, not what is wrong with it.
func LocaleEntries(cfg string) []LocaleEntry {
	sections, _ := parseLocale(cfg)
	var out []LocaleEntry
	for i := range sections {
		for _, e := range sections[i].entries {
			out = append(out, LocaleEntry{
				Section: sections[i].name,
				Key:     e.key,
				Value:   e.value,
			})
		}
	}
	return out
}
