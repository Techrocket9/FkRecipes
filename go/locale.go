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
	sections, findings := parseLocale(cfg)

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
	// setting's own name before the values it offers.
	for _, s := range l.settings {
		full := s.emittedName(prefix)
		if !localeHas(sections, "mod-setting-name", full) {
			findings = append(findings, "the setting "+full+" has no [mod-setting-name] entry"+
				elsewhere(sections, "mod-setting-name", full))
		}
		if s.kind != settingDropdown {
			continue
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
