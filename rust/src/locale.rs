use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::plan::{Lib, SettingKind};

/// How many problems a report names before it stops. A generated or badly
/// encoded file can produce one finding per line, and a thousand sentences
/// help nobody read the first. The cycle path is capped for the same reason
/// and in the same shape.
const LOCALE_FINDING_CAP: usize = 100;

impl Lib {
    /// Reads a mod's .cfg and reports what a player would not be able to read,
    /// in both directions. It returns one sentence per problem and an empty
    /// vector for a clean file.
    ///
    /// A MISSING LOCALE KEY IS A FIELD REPORT WAITING TO HAPPEN. The shape is
    /// BetterBeltBalancer's, which learned it the hard way: a live session in
    /// 2026 produced `Unknown key: "entity-name.bbb-linked-belt"` out of the
    /// engine's own "X is in the way" message, and the fix was four lines of
    /// .cfg nobody had thought to write. A dropdown is the same defect with
    /// more ways to reach it, because a string setting renders each VALUE from
    /// its own `[string-mod-setting]` entry and there is no fallback: the
    /// player stands in the settings menu reading
    /// `Unknown key: "string-mod-setting.mymod-x-y"`.
    ///
    /// IT CHECKS BOTH DIRECTIONS. A missing entry is a value the player cannot
    /// read. An ORPHAN entry is a value that used to exist, which means an
    /// option was renamed and one of the two places was not.
    ///
    /// IT RUNS ON THE HOST, and it has to: `--dump-data` does not read locale,
    /// and nothing headless opens a settings menu. This is the only place the
    /// check can live, which is why it is a library function rather than a
    /// stage-time refusal.
    ///
    /// THE MOD NAME IS A PARAMETER, AND THAT IS A DRIFT RISK WORTH STATING.
    /// Every other prefix in this library is derived from `fkdata::mod_name`
    /// at emit, where it cannot disagree with the packaged mod. A host test
    /// has no fkdata to ask, so the consumer's test passes the name their
    /// fklua.toml packages under, and a wrong name is a wrong prefix for every
    /// key at once: the result is every setting reported missing and every
    /// entry reported orphaned, which is loud rather than subtle.
    ///
    /// WHAT IT POLICES is the mod-prefix namespace plus this plan's own
    /// declared names, legacy names included. In `[string-mod-setting]` only
    /// keys under one of this plan's dropdown settings are considered, so
    /// another setting's values are not this checker's business; a legacy
    /// dropdown is policed under the name it actually carries, so a stale
    /// value key beneath it IS caught. In `[mod-setting-name]` and
    /// `[mod-setting-description]` the orphan arm fires only on keys CARRYING
    /// THE MOD PREFIX, so an entry that matches no declared setting and
    /// carries no prefix is invisible here: a renamed legacy setting's
    /// leftover entry goes unreported, and so does a setting the consumer
    /// wrote by hand under a name of its own. That arm cannot be widened on a
    /// GUESS, because a stale legacy name and a deliberately hand-rolled one
    /// are the same string to this function. `check_locale_with` is the
    /// version that widens it on a FACT: told what the mod declares
    /// elsewhere, it polices those two sections against the complete set of
    /// the mod's setting names instead of against the prefix.
    ///
    /// A DESCRIPTION IS OPTIONAL HERE, AND THAT IS A DELIBERATE DIVERGENCE
    /// from BetterBeltBalancer, which requires one. The engine's failure mode
    /// for a missing description is a lost tooltip, not an `Unknown key`
    /// render in the player's face, so it is not the defect this tripwire
    /// exists for. A description that names nothing is still reported: a
    /// renamed setting leaves one behind exactly as it leaves a name behind.
    pub fn check_locale(&self, mod_name: &str, cfg: &str) -> Vec<String> {
        self.check_locale_inner(mod_name, cfg, &[], false)
    }

    /// `check_locale` told what this mod declares OUTSIDE this library, which
    /// is what lets the name and description orphan scan be COMPLETE rather
    /// than prefix-shaped.
    ///
    /// `hand_rolled` is the consumer's whole set of settings declared
    /// elsewhere: the ones written straight into an `fk_settings` hook beside
    /// `emit`, under whatever names they carry. Given that list, this function
    /// knows every setting name the mod has, so an entry under
    /// `[mod-setting-name]` or `[mod-setting-description]` matching no
    /// declared, legacy or hand-rolled name is an orphan REGARDLESS OF PREFIX.
    /// That closes both halves of the gap the prefix rule leaves: a renamed
    /// legacy setting's leftover entry carries no prefix and is now caught,
    /// and a hand-rolled setting that happens to carry the mod prefix is no
    /// longer reported as an orphan for existing.
    ///
    /// THE LIST SUPPRESSES ORPHANS; IT DOES NOT CREATE OBLIGATIONS. A name in
    /// it is not reported missing, because this library knows the name and
    /// nothing else: whether that setting is a dropdown needing per-value
    /// entries, or a runtime-global one, or a bool, is the consumer's
    /// business. The value direction is unchanged for the same reason, so
    /// another mod's string setting in the same file is still left alone.
    ///
    /// AN EMPTY LIST IS NOT THE PLAIN CALL. It is the assertion that this mod
    /// declares nothing outside this library, so every `[mod-setting-name]`
    /// and `[mod-setting-description]` entry in the file must match a declared
    /// or legacy setting and anything else is an orphan, another mod's entry
    /// in the same file included. That is the strictest reading available and
    /// it is the right one for a mod that declares everything here; use
    /// `check_locale` when you have not enumerated the rest, because it is the
    /// call that assumes nothing.
    ///
    /// A name in the list that this plan also declares is a contradiction
    /// rather than a fact about the file, and is reported FIRST in the
    /// checker's own voice: the list is by definition what this plan does not
    /// declare, so one of the two is wrong and no orphan verdict over that
    /// name would mean anything.
    pub fn check_locale_with(
        &self,
        mod_name: &str,
        cfg: &str,
        hand_rolled: &[&str],
    ) -> Vec<String> {
        self.check_locale_inner(mod_name, cfg, hand_rolled, true)
    }

    /// Both entry points. `complete` says whether `hand_rolled` is the
    /// authoritative rest of the mod's settings; without it the name and
    /// description orphan rule can only be the mod prefix.
    fn check_locale_inner(
        &self,
        mod_name: &str,
        cfg: &str,
        hand_rolled: &[&str],
        complete: bool,
    ) -> Vec<String> {
        let prefix = format!("{}-", mod_name);
        let (sections, mut findings) = parse_locale(cfg);

        // The contradiction first, before anything reads the list as truth.
        if complete {
            for n in hand_rolled {
                if self.declares_setting(&prefix, n) {
                    findings.push(format!(
                        "the hand-rolled name {} is also a setting this plan declares; the list names only settings declared outside this library",
                        locale_show(n)
                    ));
                }
            }
        }

        // (1) and (2): what the player cannot read, in DECLARATION order, and
        // a setting's own name before the values it offers.
        for s in &self.settings {
            let full = s.emitted_name(&prefix);
            if !locale_has(&sections, "mod-setting-name", &full) {
                findings.push(format!(
                    "the setting {} has no [mod-setting-name] entry{}",
                    full,
                    elsewhere(&sections, "mod-setting-name", &full)
                ));
            }
            if s.kind != SettingKind::Dropdown {
                continue;
            }
            for v in &s.values {
                let key = format!("{}-{}", full, v);
                if !locale_has(&sections, "string-mod-setting", &key) {
                    findings.push(format!(
                        "the dropdown setting {} has no [string-mod-setting] entry for its value {}{}",
                        full,
                        v,
                        elsewhere(&sections, "string-mod-setting", &key)
                    ));
                }
            }
        }

        // (3) and (5): entries that match nothing, in FILE order. A
        // description is optional, so it is never reported missing, and it is
        // orphan-checked exactly like a name: a renamed setting leaves both
        // behind.
        for sec in &sections {
            for e in &sec.entries {
                match sec.name.as_str() {
                    "mod-setting-name" | "mod-setting-description" => {
                        // COMPLETE-LIST policing when the caller supplied the
                        // rest of the mod's settings, prefix-shaped policing
                        // when it did not. The prefix gate is not a rule
                        // anybody wants; it is the only one available when the
                        // set of names is unknown.
                        let owned = self.declares_setting(&prefix, &e.key)
                            || name_listed(hand_rolled, &e.key);
                        let scanned = complete || e.key.starts_with(&prefix);
                        if scanned && !owned {
                            findings.push(format!(
                                "the [{}] entry {} matches no setting this plan declares",
                                locale_show(&sec.name),
                                locale_show(&e.key)
                            ));
                        }
                    }
                    // Left as an arm plus an if, not folded into a match
                    // guard: the Go half is a switch with the same two
                    // conditions inside, and the mirror is worth more here
                    // than one level of nesting.
                    #[allow(clippy::collapsible_match)]
                    "string-mod-setting" => {
                        if self.polices_value_key(&prefix, &e.key)
                            && !self.declares_value(&prefix, &e.key)
                        {
                            findings.push(format!(
                                "the [string-mod-setting] entry {} matches no dropdown value this plan declares",
                                locale_show(&e.key)
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }

        // (4): <setting>-<value> is a FLAT namespace, so two settings whose
        // names are prefixes of one another can produce one key for two
        // values, and the engine keeps whichever came last. Nothing catches
        // that but this.
        let mut produced: Vec<(String, String)> = Vec::new();
        for s in &self.settings {
            if s.kind != SettingKind::Dropdown {
                continue;
            }
            let full = s.emitted_name(&prefix);
            for v in &s.values {
                let key = format!("{}-{}", full, v);
                let owner = format!("{}/{}", full, v);
                for (earlier_key, earlier_owner) in &produced {
                    if *earlier_key == key {
                        findings.push(format!(
                            "the dropdown values {} and {} both produce the [string-mod-setting] key {}",
                            earlier_owner, owner, key
                        ));
                    }
                }
                produced.push((key, owner));
            }
        }

        if findings.len() > LOCALE_FINDING_CAP {
            let rest = findings.len() - LOCALE_FINDING_CAP;
            findings.truncate(LOCALE_FINDING_CAP);
            // A singular arm, because "(and 1 more findings)" is the kind of
            // sentence that makes a reader doubt the rest of the report.
            findings.push(if rest == 1 {
                String::from("(and 1 more finding)")
            } else {
                format!("(and {} more findings)", rest)
            });
        }
        findings
    }

    fn declares_setting(&self, prefix: &str, key: &str) -> bool {
        self.settings.iter().any(|s| s.emitted_name(prefix) == key)
    }

    /// BetterBeltBalancer's rule: a key belongs to this checker only when it
    /// sits under one of THIS plan's dropdown settings. An entry for somebody
    /// else's string setting is not this function's business.
    fn polices_value_key(&self, prefix: &str, key: &str) -> bool {
        self.settings.iter().any(|s| {
            s.kind == SettingKind::Dropdown
                && key.starts_with(&format!("{}-", s.emitted_name(prefix)))
        })
    }

    fn declares_value(&self, prefix: &str, key: &str) -> bool {
        for s in &self.settings {
            if s.kind != SettingKind::Dropdown {
                continue;
            }
            for v in &s.values {
                if format!("{}-{}", s.emitted_name(prefix), v) == key {
                    return true;
                }
            }
        }
        false
    }
}

struct LocaleEntry {
    key: String,
    value: String,
}

struct LocaleSect {
    name: String,
    entries: Vec<LocaleEntry>,
    /// Indexes into `entries`, ordered by key, so a lookup is a binary search
    /// rather than a scan. A file with tens of thousands of entries is a real
    /// shape (one generated file per language), and the scan it replaces was
    /// quadratic in the number of entries.
    sorted: Vec<usize>,
}

impl LocaleSect {
    /// Binary-searches the section for a key. The first result is the position
    /// in `sorted`, which is where an insert belongs when the second is false.
    fn find(&self, key: &str) -> (usize, bool) {
        let pos = self
            .sorted
            .partition_point(|&i| self.entries[i].key.as_str() < key);
        let hit = pos < self.sorted.len() && self.entries[self.sorted[pos]].key == key;
        (pos, hit)
    }
}

/// Names the section a missing key actually turned up in, when that section
/// plausibly meant to be a settings section.
///
/// This is the mis-cased or typo'd section header, which is otherwise reported
/// as a plain absence and sends the reader looking for an entry they can see
/// with their own eyes. THE HINT IS SCOPED, and the rule is the word
/// "setting": all three sections this checker polices carry it, so a section
/// that holds the key and calls itself a setting section plausibly meant to be
/// one, while a content section never did. A real locale file carries
/// [item-name], [entity-name] and [technology-name], and an item that happens
/// to share a name with a setting must not be offered as the explanation for a
/// missing setting entry.
///
/// The comparison is ASCII-only on purpose: Rust's Unicode lowercasing and
/// Go's are not the same function on every input, and this decision has to be
/// the same in both halves.
fn elsewhere(sections: &[LocaleSect], want: &str, key: &str) -> String {
    for s in sections {
        if s.name == want {
            continue;
        }
        if !s.name.to_ascii_lowercase().contains("setting") {
            continue;
        }
        if s.find(key).1 {
            return format!(", though one sits under [{}]", locale_show(&s.name));
        }
    }
    String::new()
}

/// Reports an entry a player would actually read. An entry that is present but
/// blank renders as nothing, which is the same defect as an absent one, so it
/// counts as missing: BetterBeltBalancer's rule.
fn locale_has(sections: &[LocaleSect], section: &str, key: &str) -> bool {
    for s in sections {
        if s.name != section {
            continue;
        }
        let (pos, hit) = s.find(key);
        if !hit {
            return false;
        }
        return !s.entries[s.sorted[pos]].value.trim().is_empty();
    }
    false
}

/// Quotes a key or section name whose whitespace would otherwise be invisible
/// in the sentence. Everything else is rendered bare, so the ordinary finding
/// reads as prose.
/// Whether the consumer named this setting as one of its own. Compared
/// verbatim: a hand-rolled name is whatever the mod ships, and deriving
/// anything from it is the guessing this parameter exists to replace.
fn name_listed(hand_rolled: &[&str], key: &str) -> bool {
    hand_rolled.contains(&key)
}

fn locale_show(s: &str) -> String {
    if s != s.trim() {
        return format!("\"{}\"", s);
    }
    String::from(s)
}

/// Reads Factorio's .cfg grammar, which is INI without quoting.
///
/// THE SEMANTICS ARE BetterBeltBalancer's, and they are the engine's:
///
/// - a line is trimmed of surrounding whitespace before anything else;
/// - a blank line is skipped, and so is one starting with `#` or `;` (the
///   comment markers, at the START of a line only: neither one comments out
///   the rest of an entry);
/// - `[name]` opens a section, and entries before any section header belong to
///   no section, which is where the engine ignores them;
/// - `key=value` splits at the FIRST `=` and the value may contain more;
/// - a key defined twice OVERWRITES: the engine keeps the last one, so the
///   last value is what the presence check reads. Redefining a key to blank is
///   exactly how a blank sneaks into a file, and it has to be visible as both
///   a duplicate AND an unreadable entry;
/// - the key is NOT trimmed around the `=`, which is not sloppiness: Factorio
///   takes everything before the first `=` as the key, so `name = X` really
///   does declare a key with a trailing space, and a checker that trimmed it
///   would pass a file the game reads differently.
///
/// Anything else is a finding rather than a silent skip: a line the parser does
/// not understand is a line the player does not get.
fn parse_locale(cfg: &str) -> (Vec<LocaleSect>, Vec<String>) {
    let mut sections: Vec<LocaleSect> = Vec::new();
    let mut findings: Vec<String> = Vec::new();
    let mut current: Option<usize> = None;

    // A BOM is REPORTED AND THEN STRIPPED. How the engine treats one here has
    // not been measured, so the checker may not silently bless it; and leaving
    // it in place would make the first key unreadable and turn one encoding
    // mistake into a cascade of findings about entries that are perfectly
    // fine.
    let mut cfg = cfg;
    if let Some(rest) = cfg.strip_prefix('\u{feff}') {
        findings.push(String::from("the file begins with a byte order mark"));
        cfg = rest;
    }

    for raw in cfg.split('\n') {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = &line[1..line.len() - 1];
            if name.is_empty() {
                // The current section is left alone rather than replaced by a
                // nameless one: the entries that follow still belong where the
                // last real header put them.
                findings.push(String::from(
                    "the locale section header [] names no section",
                ));
                continue;
            }
            current = sections.iter().position(|s| s.name == name);
            if current.is_none() {
                sections.push(LocaleSect {
                    name: String::from(name),
                    entries: Vec::new(),
                    sorted: Vec::new(),
                });
                current = Some(sections.len() - 1);
            }
            continue;
        }
        let (key, value) = match line.find('=') {
            Some(at) => (&line[..at], &line[at + 1..]),
            None => {
                findings.push(format!(
                    "the locale line {} is neither a section nor an entry",
                    locale_show(line)
                ));
                continue;
            }
        };
        if key.is_empty() {
            findings.push(format!(
                "the locale line {} has no key before its =",
                locale_show(line)
            ));
            continue;
        }
        let at = match current {
            Some(at) => at,
            None => {
                findings.push(format!(
                    "the locale entry {} sits before any section header",
                    locale_show(key)
                ));
                continue;
            }
        };
        let (pos, dup) = sections[at].find(key);
        if dup {
            findings.push(format!(
                "the [{}] entry {} is defined twice; the engine keeps the last one",
                locale_show(&sections[at].name),
                locale_show(key)
            ));
            let idx = sections[at].sorted[pos];
            sections[at].entries[idx].value = String::from(value);
            continue;
        }
        sections[at].entries.push(LocaleEntry {
            key: String::from(key),
            value: String::from(value),
        });
        let idx = sections[at].entries.len() - 1;
        sections[at].sorted.insert(pos, idx);
    }
    (sections, findings)
}

#[cfg(test)]
mod tests {
    use crate::plan::{Lib, NumericSpec};

    /// Declares the settings the example guests declare, in the same order and
    /// with the same values. The checker reads settings and nothing else, so
    /// the example's items, recipes and technologies are not repeated here;
    /// what has to match is every name a locale key is built from.
    fn steelworks_settings() -> Lib {
        let mut lib = Lib::new();
        lib.bool_setting("hardened-tools", true);
        lib.int_setting("rivet-batch", 4, NumericSpec::between(1.0, 20.0));
        lib.double_setting(
            "forging-time",
            3.0,
            NumericSpec {
                min: None,
                max: Some(120.0),
            },
        );
        lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
        lib.bool_setting("bonus-research", true);
        lib
    }

    fn read_testdata(path: &str) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            panic!("the fixture is the thing under test and it is not there: {e}")
        })
    }

    /// THE CROSS-LANGUAGE PIN. Both halves run this same plan over these same
    /// bytes and must produce this same file, so the two checkers are held to
    /// one output with no toolchain and no packaged mod in the way.
    #[test]
    fn check_locale_matches_the_golden() {
        let cfg = read_testdata("../testdata/locale/example.cfg");
        let golden = read_testdata("../testdata/locale/findings.golden");

        let mut got = steelworks_settings()
            .check_locale("fkrecipes-example", &cfg)
            .join("\n");
        got.push('\n');
        assert_eq!(got, golden, "the findings do not match the golden");
    }

    /// A file with every entry the plan needs and nothing it does not.
    #[test]
    fn check_locale_accepts_a_complete_file() {
        let cfg = "[mod-setting-name]
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
";
        let findings = steelworks_settings().check_locale("fkrecipes-example", cfg);
        assert!(
            findings.is_empty(),
            "a complete file produced findings:\n{}",
            findings.join("\n")
        );
    }

    #[test]
    fn check_locale_findings() {
        struct Case {
            name: &'static str,
            cfg: &'static str,
            want: &'static [&'static str],
        }

        const EVERY_NAME_MISSING: &[&str] = &[
            "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
            "the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
            "the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
            "the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
            "the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
            "the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
            "the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
        ];

        let cases = [
            Case {
                name: "a setting with no name entry",
                cfg: "[mod-setting-name]\nfkrecipes-example-hardened-tools=Hardened tools\n",
                want: &EVERY_NAME_MISSING[1..],
            },
            Case {
                // Present but blank renders as nothing, which is the defect the
                // player reports, so it counts as missing.
                name: "an entry that is present but empty",
                cfg: "[mod-setting-name]\nfkrecipes-example-hardened-tools=\n",
                want: EVERY_NAME_MISSING,
            },
            Case {
                name: "a description entry matching nothing",
                cfg: "[mod-setting-description]\nfkrecipes-example-scrap-recovery=Left behind by a rename.\n",
                want: &[
                    "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
                    "the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
                    "the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
                    "the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
                    "the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
                    "the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
                    "the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
                    "the [mod-setting-description] entry fkrecipes-example-scrap-recovery matches no setting this plan declares",
                ],
            },
            Case {
                // Another mod's string setting shares the section and is none
                // of this checker's business.
                name: "a value entry under a setting this plan does not own",
                cfg: "[string-mod-setting]\nsomeothermod-belt-tier-express=Express\n",
                want: EVERY_NAME_MISSING,
            },
            Case {
                name: "a line that is neither a section nor an entry",
                cfg: "[mod-setting-name]\nthis line has no equals sign\n",
                want: &[
                    "the locale line this line has no equals sign is neither a section nor an entry",
                    "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
                    "the setting fkrecipes-example-rivet-batch has no [mod-setting-name] entry",
                    "the setting fkrecipes-example-forging-time has no [mod-setting-name] entry",
                    "the setting fkrecipes-example-quench-medium has no [mod-setting-name] entry",
                    "the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
                    "the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value oil",
                    "the setting fkrecipes-example-bonus-research has no [mod-setting-name] entry",
                ],
            },
        ];

        for c in cases {
            let got = steelworks_settings().check_locale("fkrecipes-example", c.cfg);
            let want: Vec<String> = c.want.iter().map(|s| String::from(*s)).collect();
            assert_eq!(got, want, "{}", c.name);
        }
    }

    /// `<setting>-<value>` is a flat namespace: two settings whose names are
    /// prefixes of one another can produce one key for two values, and the
    /// engine keeps whichever came last. Nothing else catches that.
    #[test]
    fn check_locale_finds_colliding_value_keys() {
        let mut lib = Lib::new();
        lib.dropdown_setting_needing_locale("quench", "medium-oil", &["medium-oil"]);
        lib.dropdown_setting_needing_locale("quench-medium", "oil", &["oil"]);

        let cfg = "[mod-setting-name]
fkrecipes-example-quench=Quench
fkrecipes-example-quench-medium=Quenching medium

[string-mod-setting]
fkrecipes-example-quench-medium-oil=Oil
";
        assert_eq!(
            lib.check_locale("fkrecipes-example", cfg),
            alloc::vec![String::from(
                "the dropdown values fkrecipes-example-quench/medium-oil and fkrecipes-example-quench-medium/oil both produce the [string-mod-setting] key fkrecipes-example-quench-medium-oil"
            )]
        );
    }

    /// A key redefined to blank is EXACTLY how a blank sneaks into a file, and
    /// the engine keeps the last one. Both findings have to fire: the
    /// duplicate that explains it, and the unreadable entry the player would
    /// actually meet.
    #[test]
    fn check_locale_reads_the_last_of_a_duplicate() {
        struct Case {
            name: &'static str,
            cfg: &'static str,
            about: &'static str,
            want: &'static [&'static str],
        }

        let cases = [
            Case {
                name: "a setting name redefined to blank",
                about: "hardened-tools",
                cfg: "[mod-setting-name]\nfkrecipes-example-hardened-tools=Hardened tools\nfkrecipes-example-hardened-tools=\n",
                want: &[
                    "the [mod-setting-name] entry fkrecipes-example-hardened-tools is defined twice; the engine keeps the last one",
                    "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
                ],
            },
            Case {
                name: "a dropdown value redefined to blank",
                about: "water",
                cfg: "[string-mod-setting]\nfkrecipes-example-quench-medium-water=Water\nfkrecipes-example-quench-medium-water=\n",
                want: &[
                    "the [string-mod-setting] entry fkrecipes-example-quench-medium-water is defined twice; the engine keeps the last one",
                    "the dropdown setting fkrecipes-example-quench-medium has no [string-mod-setting] entry for its value water",
                ],
            },
        ];

        for c in cases {
            let got = steelworks_settings().check_locale("fkrecipes-example", c.cfg);
            // Only the findings this shape is about; the other settings are
            // missing for the ordinary reason and are not the point.
            let kept: Vec<&String> = got.iter().filter(|f| f.contains(c.about)).collect();
            let want: Vec<String> = c.want.iter().map(|s| String::from(*s)).collect();
            let want_refs: Vec<&String> = want.iter().collect();
            assert_eq!(kept, want_refs, "{}", c.name);
        }
    }

    /// The mis-cased section header: the entry is right there in the file, and
    /// a bare "has no entry" sends the reader looking for something they can
    /// see.
    ///
    /// The hint is SCOPED to sections that call themselves settings sections.
    /// A content section that happens to hold a colliding key is not an
    /// explanation for anything, and offering it as one would be worse than
    /// saying nothing.
    #[test]
    fn check_locale_names_the_section_a_missing_key_sits_under() {
        let cases = [
            (
                "the mis-cased header",
                "Mod-Setting-Name",
                "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry, though one sits under [Mod-Setting-Name]",
            ),
            (
                "the plural typo",
                "mod-settings-name",
                "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry, though one sits under [mod-settings-name]",
            ),
            (
                "a name entry filed under the description section",
                "mod-setting-description",
                "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry, though one sits under [mod-setting-description]",
            ),
            (
                // A content section is none of this checker's business, and an
                // item sharing a name with a setting is a coincidence rather
                // than an explanation.
                "a content section holding a colliding key",
                "item-name",
                "the setting fkrecipes-example-hardened-tools has no [mod-setting-name] entry",
            ),
        ];
        for (name, section, want) in cases {
            let cfg = format!(
                "[{}]\nfkrecipes-example-hardened-tools=Hardened tools\n",
                section
            );
            let got = steelworks_settings().check_locale("fkrecipes-example", &cfg);
            assert_eq!(got.first().map(String::as_str), Some(want), "{}", name);
        }
    }

    /// A byte order mark is reported ONCE and then stripped. How the engine
    /// treats one has not been measured, so it is never blessed silently;
    /// leaving it in would make the first key unreadable and bury the encoding
    /// mistake under findings about entries that are fine.
    #[test]
    fn check_locale_reports_a_byte_order_mark_once() {
        let cfg = "\u{feff}[mod-setting-name]\nfkrecipes-example-hardened-tools=Hardened tools\n";
        let got = steelworks_settings().check_locale("fkrecipes-example", cfg);

        assert_eq!(
            got.first().map(String::as_str),
            Some("the file begins with a byte order mark"),
            "the byte order mark was not reported first: {:?}",
            got
        );
        for f in got.iter().skip(1) {
            assert!(
                !f.contains("hardened-tools"),
                "the mark cascaded into a finding about a good entry: {}",
                f
            );
        }
    }

    #[test]
    fn check_locale_reports_malformed_shapes() {
        let cfg = "[]\n[mod-setting-name]\n=Hardened tools\nfkrecipes-example-orphan =Trailing space in the key\n";
        let got = steelworks_settings().check_locale("fkrecipes-example", cfg);

        assert_eq!(
            got.first().map(String::as_str),
            Some("the locale section header [] names no section")
        );
        assert_eq!(
            got.get(1).map(String::as_str),
            Some("the locale line =Hardened tools has no key before its =")
        );
        // The whitespace in a key is invisible in prose, so it is quoted.
        assert!(
            got.iter().any(|f| f
                == "the [mod-setting-name] entry \"fkrecipes-example-orphan \" matches no setting this plan declares"),
            "the key with trailing whitespace was not quoted: {:?}",
            got
        );
    }

    /// A generated or badly encoded file produces one finding per line, and a
    /// thousand sentences help nobody read the first.
    #[test]
    fn check_locale_caps_its_findings() {
        // 5 settings with no name and 2 dropdown values with no entry come
        // before the orphans, so the orphan count sets how far past the cap a
        // report lands.
        let report_with_orphans = |n: usize| {
            let mut cfg = String::from("[mod-setting-name]\n");
            for i in 0..n {
                cfg.push_str(&format!("fkrecipes-example-orphan-{}=Left behind\n", i));
            }
            steelworks_settings().check_locale("fkrecipes-example", &cfg)
        };

        let got = report_with_orphans(150);
        assert_eq!(
            got.len(),
            super::LOCALE_FINDING_CAP + 1,
            "got {} findings, want the cap plus one closing line",
            got.len()
        );
        assert_eq!(
            got.last().map(String::as_str),
            Some("(and 57 more findings)")
        );

        // The boundary: exactly one finding past the cap reads as one.
        let got = report_with_orphans(94);
        assert_eq!(got.len(), super::LOCALE_FINDING_CAP + 1);
        assert_eq!(got.last().map(String::as_str), Some("(and 1 more finding)"));

        // One below it is not capped at all, so no closing line is added.
        let got = report_with_orphans(93);
        assert_eq!(got.len(), super::LOCALE_FINDING_CAP);
        assert!(!got.last().unwrap().starts_with("(and "));
    }

    /// A wrong mod name is a wrong prefix for every key at once, which is the
    /// loud failure the doc comment promises rather than a subtle one.
    #[test]
    fn check_locale_with_the_wrong_mod_name_fails_everything() {
        let cfg = read_testdata("../testdata/locale/example.cfg");
        let findings = steelworks_settings().check_locale("steelworks", &cfg);

        assert!(
            findings.len() >= 6,
            "a wrong mod name produced only {} findings",
            findings.len()
        );
        for f in findings.iter().take(6) {
            assert!(
                f.contains("steelworks-"),
                "finding does not name the wrong prefix: {}",
                f
            );
        }
    }

    // -----------------------------------------------------------------------
    // check_locale_with: the complete-list orphan rule.
    // -----------------------------------------------------------------------

    /// The migration pilot's shape: two legacy dropdowns declared here, and a
    /// third setting the mod declares itself.
    fn bbb_plan() -> Lib {
        let mut lib = Lib::new();
        lib.legacy_dropdown_setting_needing_locale(
            "bbb-recipe-cost",
            "vanilla",
            &["vanilla", "cheap"],
            "a",
        );
        lib.legacy_dropdown_setting_needing_locale(
            "bbb-tech-cost",
            "logistics",
            &["logistics", "logistics-2"],
            "b",
        );
        lib
    }

    const BBB_CFG: &str = "[mod-setting-name]\n\
        bbb-recipe-cost=Recipe cost\n\
        bbb-tech-cost=Research cost\n\
        bbb-multi-edge-parts=Multi-edge parts\n\
        bbb-renamed-away=Left over from a rename\n\
        \n\
        [string-mod-setting]\n\
        bbb-recipe-cost-vanilla=Vanilla\n\
        bbb-recipe-cost-cheap=Cheap\n\
        bbb-tech-cost-logistics=Logistics\n\
        bbb-tech-cost-logistics-2=Logistics 2\n";

    /// The GAP THIS PARAMETER EXISTS FOR, stated as the difference between the
    /// two calls over one file. Neither the hand-rolled name nor the leftover
    /// carries the mod prefix, so the plain call cannot see either of them.
    #[test]
    fn plain_check_locale_cannot_see_unprefixed_entries() {
        let got = bbb_plan().check_locale("better-belt-balancer", BBB_CFG);
        assert_eq!(got, Vec::<String>::new());
    }

    /// Told what the mod declares elsewhere, the same file gives up the
    /// leftover and stays quiet about the hand-rolled one.
    #[test]
    fn check_locale_with_polices_the_complete_list() {
        let got = bbb_plan().check_locale_with(
            "better-belt-balancer",
            BBB_CFG,
            &["bbb-multi-edge-parts"],
        );
        assert_eq!(
            got,
            ["the [mod-setting-name] entry bbb-renamed-away matches no setting this plan declares"]
        );
    }

    /// The list is what suppresses the hand-rolled entry, so leaving it out
    /// reports that entry too. This is the other side of the test above:
    /// without it the pair could both pass on a checker that reported nothing.
    #[test]
    fn check_locale_with_reports_an_unlisted_hand_rolled_entry() {
        let got = bbb_plan().check_locale_with("better-belt-balancer", BBB_CFG, &[]);
        assert_eq!(
            got,
            [
                "the [mod-setting-name] entry bbb-multi-edge-parts matches no setting this plan declares",
                "the [mod-setting-name] entry bbb-renamed-away matches no setting this plan declares",
            ]
        );
    }

    /// The prefix-only FALSE POSITIVE, closed: a hand-rolled setting that
    /// happens to carry the mod prefix is an orphan to the plain call and is
    /// not one here.
    #[test]
    fn check_locale_with_clears_a_prefixed_hand_rolled_name() {
        let mut lib = Lib::new();
        lib.bool_setting("hardened-tools", true);
        let cfg = "[mod-setting-name]\n\
            steelworks-hardened-tools=Hardened tools\n\
            steelworks-written-by-hand=Written by hand\n";
        assert_eq!(
            lib.check_locale("steelworks", cfg),
            ["the [mod-setting-name] entry steelworks-written-by-hand matches no setting this plan declares"]
        );
        assert_eq!(
            lib.check_locale_with("steelworks", cfg, &["steelworks-written-by-hand"]),
            Vec::<String>::new()
        );
    }

    /// The list names what this plan does NOT declare, so a name in both is a
    /// contradiction and is said first, before any verdict that would rest on
    /// it.
    #[test]
    fn check_locale_with_refuses_a_contradictory_list() {
        let got = bbb_plan().check_locale_with(
            "better-belt-balancer",
            BBB_CFG,
            &["bbb-recipe-cost", "bbb-multi-edge-parts"],
        );
        assert_eq!(
            got,
            [
                "the hand-rolled name bbb-recipe-cost is also a setting this plan declares; the list names only settings declared outside this library",
                "the [mod-setting-name] entry bbb-renamed-away matches no setting this plan declares",
            ]
        );
    }

    /// THE MISSING DIRECTION IS UNCHANGED, and that is the point of the
    /// parameter suppressing orphans without creating obligations: this
    /// library knows a hand-rolled setting's NAME and nothing else, so it
    /// cannot say what entries that setting needs. The declared settings are
    /// still held to theirs.
    #[test]
    fn check_locale_with_leaves_the_missing_direction_alone() {
        let cfg = "[mod-setting-name]\n\
            bbb-recipe-cost=Recipe cost\n\
            \n\
            [string-mod-setting]\n\
            bbb-recipe-cost-vanilla=Vanilla\n\
            bbb-recipe-cost-cheap=Cheap\n";
        let got =
            bbb_plan().check_locale_with("better-belt-balancer", cfg, &["bbb-multi-edge-parts"]);
        assert_eq!(
            got,
            [
                "the setting bbb-tech-cost has no [mod-setting-name] entry",
                "the dropdown setting bbb-tech-cost has no [string-mod-setting] entry for its value logistics",
                "the dropdown setting bbb-tech-cost has no [string-mod-setting] entry for its value logistics-2",
            ]
        );
    }

    /// The VALUE direction is unchanged too: another mod's string setting in
    /// the same file is not this checker's business whether or not a complete
    /// list was given, because a name alone says nothing about what values a
    /// setting offers.
    #[test]
    fn check_locale_with_leaves_foreign_value_keys_alone() {
        let cfg = "[mod-setting-name]\n\
            bbb-recipe-cost=Recipe cost\n\
            bbb-tech-cost=Research cost\n\
            bbb-multi-edge-parts=Multi-edge parts\n\
            \n\
            [string-mod-setting]\n\
            bbb-recipe-cost-vanilla=Vanilla\n\
            bbb-recipe-cost-cheap=Cheap\n\
            bbb-tech-cost-logistics=Logistics\n\
            bbb-tech-cost-logistics-2=Logistics 2\n\
            bbb-multi-edge-parts-aggressive=Aggressive\n";
        let got =
            bbb_plan().check_locale_with("better-belt-balancer", cfg, &["bbb-multi-edge-parts"]);
        assert_eq!(got, Vec::<String>::new());
    }
}

/// One key and value from a `.cfg`, with the section it sat under.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CfgEntry {
    pub section: String,
    pub key: String,
    pub value: String,
}

/// Parses a `.cfg` into its entries, in FILE ORDER, for a consumer's own
/// assertions.
///
/// [`Lib::check_locale`] answers the questions this library can ask, which are
/// the ones about the settings it generated. A consumer has questions of their
/// own: that an entity name entry exists for their hand-rolled entity, that no
/// key is blank, that a translation file carries the same keys as the English
/// one. Writing a second `.cfg` parser to ask them is the kind of duplication
/// that drifts, so this is the same parser's output, exported.
///
/// PARSE FINDINGS ARE NOT REPORTED HERE. A malformed line is skipped exactly
/// as `check_locale` skips it; run `check_locale` for the diagnosis. This
/// returns what the file says, not what is wrong with it.
pub fn locale_entries(cfg: &str) -> Vec<CfgEntry> {
    let (sections, _) = parse_locale(cfg);
    let mut out = Vec::new();
    for sec in &sections {
        for e in &sec.entries {
            out.push(CfgEntry {
                section: sec.name.clone(),
                key: e.key.clone(),
                value: e.value.clone(),
            });
        }
    }
    out
}
