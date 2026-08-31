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
    /// wrote by hand under a name of its own. That arm cannot be widened
    /// soundly, because a stale legacy name and a deliberately hand-rolled one
    /// are the same string to this function; policing them needs a
    /// known-hand-rolled parameter it does not have rather than a guess over
    /// unrecognized names.
    ///
    /// A DESCRIPTION IS OPTIONAL HERE, AND THAT IS A DELIBERATE DIVERGENCE
    /// from BetterBeltBalancer, which requires one. The engine's failure mode
    /// for a missing description is a lost tooltip, not an `Unknown key`
    /// render in the player's face, so it is not the defect this tripwire
    /// exists for. A description that names nothing is still reported: a
    /// renamed setting leaves one behind exactly as it leaves a name behind.
    pub fn check_locale(&self, mod_name: &str, cfg: &str) -> Vec<String> {
        let prefix = format!("{}-", mod_name);
        let (sections, mut findings) = parse_locale(cfg);

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
                        if e.key.starts_with(&prefix) && !self.declares_setting(&prefix, &e.key) {
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
}
