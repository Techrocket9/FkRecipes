use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::plan::{Lib, SettingKind};
use crate::settings::{
    text_description, text_format_line, text_ladder_line, Presets, TEXT_FALLBACK_LINE,
};
use crate::value::Value;

/// How many problems a report names before it stops. A generated or badly
/// encoded file can produce one finding per line, and a thousand sentences
/// help nobody read the first. The cycle path is capped for the same reason
/// and in the same shape.
const LOCALE_FINDING_CAP: usize = 100;

/// The same ceiling over the ADVISORIES, which are a report of their own: see
/// [`Lib::check_locale_advisories`]. Two caps rather than one is what keeps the
/// promise that an advisory can never displace a finding, and the separate
/// accessor is what keeps it from being one.
const LOCALE_ADVISORY_CAP: usize = 100;

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
    /// A PROTOTYPE'S OWN DESCRIPTION KEY IS NEVER ASKED FOR, and that is a
    /// third class beside REQUIRED and ADVISORY rather than an omission. The
    /// data stage composes `recipe-description.<emitted name>` and
    /// `technology-description.<name>` onto a prototype carrying a note with
    /// no declared `description`, and both are inside the mod's prefix, which
    /// by the rule above would make them REQUIRED. They must not be: they are
    /// the AUTHOR'S OWN OPTIONAL entry, referenced precisely so an author who
    /// wrote one keeps it, and requiring them would make every consumer owe a
    /// description for every recipe and technology they emit in a report
    /// `docs/migration.md` tells them to assert is EMPTY. They are not
    /// advisory either, because an advisory says the key is somebody else's
    /// and these are the consumer's own. This function walks SETTINGS and
    /// never recipes or technologies, so the silence is structural: there is
    /// no exclusion to forget. See
    /// [`description_ref`](crate::settings::description_ref).
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

        // (0) THE LIBRARY'S OWN LINES, AHEAD OF EVERY FINDING ABOUT THE FILE.
        // This is the one rule here that is not about the author's .cfg at
        // all, so it goes first and it goes once. See
        // [`Lib::check_composed_text_lines`].
        let mut findings = self.check_composed_text_lines(&prefix);
        let (sections, parsed) = parse_locale(cfg);
        findings.extend(parsed);

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
        let numbers = self.research_number_settings();
        for (i, s) in self.settings.iter().enumerate() {
            let full = s.emitted_name(&prefix);
            if !locale_has(&sections, "mod-setting-name", &full) {
                findings.push(format!(
                    "the setting {} has no [mod-setting-name] entry{}",
                    full,
                    elsewhere(&sections, "mod-setting-name", &full)
                ));
            }
            // THE THREE SETTINGS WHOSE DESCRIPTION IS NOT OPTIONAL. A text
            // setting's description is where the library writes the declared
            // list, the format and its length limit, the switch, the fallback
            // and, on every field that is the one to disclose it, the ladder,
            // so a missing entry loses all of them along with whatever the
            // consumer meant to say; a research number's is where the range and
            // what 0 means are written; and a dropdown with a text setting
            // beside it has its presets composed onto its own description, so a
            // missing entry there is a key rendered raw in the tooltip.
            let missing_description = !locale_has(&sections, "mod-setting-description", &full);
            if matches!(s.kind, SettingKind::Ingredients | SettingKind::Packs) {
                if missing_description {
                    findings.push(format!(
                        "the setting {} has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
                        full
                    ));
                }
                continue;
            }
            if numbers[i].bound {
                if missing_description {
                    findings.push(format!(
                        "the setting {} has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
                        full
                    ));
                }
                continue;
            }
            if s.kind != SettingKind::Dropdown {
                continue;
            }
            // A dropdown the library composes anything onto has its
            // description stop being optional, and WHICH SENTENCE says so is
            // the shape: a dropdown with a text setting beside it loses a
            // preset list, and a bare ingredient dropdown loses the one line
            // about the ladder. Saying "its preset list" of the second would
            // name a list that dropdown composes nothing of.
            if missing_description {
                match self.composed_dropdown_presets(i + 1) {
                    None => {}
                    Some(Presets::Ingredients(_, None)) => findings.push(format!(
                        "the dropdown setting {} has no [mod-setting-description] entry, and the library composes onto that entry the line saying what a name this game does not have costs the list",
                        full
                    )),
                    Some(_) => findings.push(format!(
                        "the dropdown setting {} has no [mod-setting-description] entry, and the library composes its preset list onto that entry",
                        full
                    )),
                }
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

    /// What this library has to SAY about a consumer's locale file without
    /// asking anything of it: one note per locale key the composition
    /// references from OUTSIDE the mod's own prefix, in composition order. It
    /// is INFORMATIONAL, and a test suite must not fail on it.
    ///
    /// IT IS NOT PART OF [`check_locale`](Lib::check_locale), AND THAT IS THE
    /// POINT. An empty return from `check_locale` means a clean file, and a
    /// consumer's suite is told in `docs/migration.md` to assert exactly that;
    /// an advisory in that return would be a permanent red test over a thing
    /// the consumer is told NOT to fix, which is the defect this separation
    /// closes. It does not depend on the .cfg at all, so it takes no cfg: it is
    /// a property of the PLAN.
    ///
    /// WHAT IT COVERS is today exactly `technology-name.<source>`, from a cost
    /// dropdown's preset lines. Factorio's locale namespace is FLAT and shared:
    /// defining `technology-name.logistics-2` in this mod's own .cfg sets the
    /// displayed name of BASE's technology for every mod in the game. Requiring
    /// the key would therefore be requiring exactly the hazard `check_locale`'s
    /// own collision scan exists to catch, so the sentence names the key, says
    /// what the tooltip shows where the game does not define it, and pointedly
    /// does not tell the consumer to define it.
    ///
    /// A MISSING KEY IS NOT A DEFECT.
    /// [`locale_ref`](crate::settings::locale_ref) wraps every composed
    /// reference in the engine's alternatives form, so an undefined
    /// `technology-name` key degrades to the raw internal name and the tooltip
    /// survives whole; before that wrapper it cost the consumer the entire
    /// tooltip, silently.
    ///
    /// IT HAS A CAP OF ITS OWN, [`LOCALE_ADVISORY_CAP`], with the same closing
    /// line the findings cap uses. Two reports, two budgets, and neither can
    /// crowd out the other.
    pub fn check_locale_advisories(&self, mod_name: &str) -> Vec<String> {
        let prefix = format!("{}-", mod_name);
        let mut advisories = self.composed_game_key_advisories(&prefix);
        if advisories.len() > LOCALE_ADVISORY_CAP {
            let rest = advisories.len() - LOCALE_ADVISORY_CAP;
            advisories.truncate(LOCALE_ADVISORY_CAP);
            // A singular arm, for the reason the findings cap has one.
            advisories.push(if rest == 1 {
                String::from("(and 1 more advisory)")
            } else {
                format!("(and {} more advisories)", rest)
            });
        }
        advisories
    }

    /// [`check_locale_advisories`](Lib::check_locale_advisories)' uncapped
    /// walk: one sentence per locale key the composition references from
    /// outside this mod's prefix, which today is exactly
    /// `technology-name.<source>`.
    ///
    /// THE ORDER IS THE COMPOSITION'S, setting by setting in declaration order
    /// and choice by choice within one, and it steps past exactly what the
    /// composition steps past, because it asks the same function:
    /// `composed_dropdown_presets` is where a dropdown's presets are chosen,
    /// `describes` and all, and it is the only spelling of that. One dropdown
    /// naming one key twice says the same sentence twice, so it is said once.
    ///
    /// THE ORDER MOVED FROM TECHNOLOGY ORDER TO SETTING ORDER WITH THAT, and
    /// the two differ only on a plan with two cost dropdowns declared in one
    /// order and described by technologies declared in another. Setting order
    /// is the right one of the two, because what each advisory is about is a
    /// setting.
    fn composed_game_key_advisories(&self, prefix: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (i, s) in self.settings.iter().enumerate() {
            let choices = match self.composed_dropdown_presets(i + 1) {
                Some(Presets::Cost(choices, _)) => choices,
                _ => continue,
            };
            let full = s.emitted_name(prefix);
            for choice in choices {
                // A CHOICE CARRYING `display` NAMES NO KEY AT ALL, so there is
                // nothing to advise about: the composed tail is the author's
                // own literal and the `technology-name` reference is not
                // composed for that line. Taking the advisory away is the
                // second thing the override is for.
                if !choice.display.is_empty() {
                    continue;
                }
                let source = match choice.sources.first() {
                    Some(s) => s,
                    None => continue,
                };
                let line = game_key_advisory(&full, &format!("technology-name.{}", source), source);
                if out.contains(&line) {
                    continue;
                }
                out.push(line);
            }
        }
        out
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

    /// The DRIFT GUARD over what this library composes onto a text setting's
    /// description: the ladder line where that field carries one, the format
    /// line, the switch line and the
    /// fallback line, each reported by name when the composition stops carrying
    /// it.
    ///
    /// IT IS NOT AN AUTHOR FINDING, and that is why it is worded and placed the
    /// way it is. The consumer writes the `[mod-setting-description]` entry and
    /// this library writes everything under it, so a report here says the library
    /// shipped a description missing a line it owes; nothing the consumer can type
    /// puts one back. The checker is where it lives because the composition has no
    /// other reader that runs on a host: a settings-stage refusal would be a load
    /// failure over a tooltip, and the engine's own dump proves the shape only
    /// where somebody runs an engine.
    ///
    /// ONCE PER REPORT, NOT ONCE PER SETTING, and the sentence names the library
    /// rather than a setting. What it inspects does not vary with the setting in
    /// any way the rule reads: it asks for four lines at most, of which the
    /// fallback line is a constant, the ladder line and the format line are
    /// constants the KIND chooses between and the switch line is one of two
    /// sentences the declaration chooses between; all four arrive from the
    /// caller that built the composition, beside the flag saying whether the
    /// ladder line is one of them at all, and the only per-setting part of the
    /// composition, the consumer's own key, is not what it looks at. Run inside the per-setting loop it turned
    /// ONE library defect into one finding per text setting, five of them on the
    /// example guest, and at fifty text settings the sentences alone would fill
    /// [`LOCALE_FINDING_CAP`] and push every author finding out of the report.
    ///
    /// FIRST IN THE REPORT, WHICH IS ALSO HOW IT SURVIVES THE CAP. The cap keeps
    /// the first [`LOCALE_FINDING_CAP`] findings and replaces the tail with a
    /// count, so a finding emitted ahead of the parse findings and of every rule
    /// about the author's file cannot be dropped by a file that produces a
    /// thousand of its own. That is the cheaper of the two ways to keep it: a cap
    /// exemption would have to be carried through the truncation in both halves,
    /// and this is one ordering decision instead.
    ///
    /// NOTHING TO GUARD WITHOUT A TEXT SETTING. A plan that declares none composes
    /// no text description, so there is no shipped description for a line to have
    /// gone missing from, and a report about one would name a defect that plan
    /// cannot carry.
    ///
    /// THE LIST IS LEFT OUT OF THE COMPOSITION, and that is the one deviation from
    /// "check what is emitted". Rendering the declared list is the only part of
    /// [`text_description`] that reaches the language, and it is also the only
    /// part that indexes `self.items`. The checker validates nothing, exactly as
    /// the rest of it validates nothing, so a plan the planners would refuse must
    /// not panic here: with the list left out neither the language nor the item
    /// table is touched, and the lines under test are the ones this
    /// function can see. What the rendered list itself says is the settings
    /// stage's business and the corpus's.
    pub(crate) fn check_composed_text_lines(&self, prefix: &str) -> Vec<String> {
        match self.guarded_text_description(prefix) {
            Some((desc, switch_line, ingredients, ladder)) => {
                composed_text_lines_missing(&desc, &switch_line, ingredients, ladder)
            }
            None => Vec::new(),
        }
    }

    /// WHICH composition the guard inspects, split out from the rule so that
    /// the choice is visible to a test on its own: the first text setting's in
    /// declaration order, with the declared list left out, and no composition
    /// at all when the plan declares no text setting.
    ///
    /// THE SWITCH LINE, THE KIND AND THE LADDER FLAG COME BACK BESIDE THE
    /// COMPOSITION because they are the three inputs the composition is not a
    /// constant in. The switch line names the option above or below when a
    /// dropdown is bound to the same declaration and the mod's own list when
    /// none is; the kind decides TWO lines, the ladder line's whole vocabulary
    /// (a pack and a research that takes fewer of them, against a name and a
    /// shorter craft) and whether the format line names the word `none`,
    /// which only an ingredient list takes; and the ladder flag decides whether
    /// the ladder line is there to look for at all, which is the same question
    /// `text_carries_ladder_line` answers for the composition. The rule cannot
    /// recompute any of them without the
    /// declaration, so the caller that built the composition hands over what it
    /// built it with, and what the guard then answers is whether
    /// [`text_description`] put those lines into the table it returned.
    ///
    /// THE FLAG IS WHAT KEEPS THE GUARD FROM ASKING FOR A LINE THAT IS NOT
    /// OWED. Beside an ingredient dropdown the ladder is disclosed on that
    /// dropdown instead, so a guard that always looked for it would report a
    /// defect on every plan the customizer was designed for; one that never
    /// looked for it would stop watching the compositions that do carry it,
    /// which is every other shape.
    ///
    /// The tuple is the composition, the switch line, whether the setting is an
    /// ingredient list, and whether it carries a ladder line.
    pub(crate) fn guarded_text_description(
        &self,
        prefix: &str,
    ) -> Option<(Value, String, bool, bool)> {
        self.settings
            .iter()
            .enumerate()
            .find(|(_, s)| matches!(s.kind, SettingKind::Ingredients | SettingKind::Packs))
            .map(|(i, s)| {
                let line = self.text_switch_line(i);
                let ingredients = s.kind == SettingKind::Ingredients;
                let ladder = self.text_carries_ladder_line(i);
                (
                    text_description(&s.emitted_name(prefix), "", &line, ingredients, ladder),
                    line,
                    ingredients,
                    ladder,
                )
            })
    }
}

/// The guard's rule over one composition.
///
/// THE DESCRIPTION IS A PARAMETER so that a test can hand it the composition
/// with one line taken out of it, which is the only way to see the finding
/// without editing the source: nothing a consumer can declare produces a
/// composition missing a line.
///
/// `ladder` IS THE COMPOSITION'S OWN ANSWER AND NOT A SECOND RULE. It arrives
/// beside the description from the caller that built it, so the guard asks for
/// exactly the lines that composition put in; deriving it here from the switch
/// line's wording would be a second spelling of `text_carries_ladder_line`, and
/// the two could then disagree about which shape they are looking at. The
/// switch line cannot answer it in any case: it says a dropdown decides without
/// saying which kind of dropdown, and the kind is the whole of the rule.
pub(crate) fn composed_text_lines_missing(
    desc: &Value,
    switch_line: &str,
    ingredients: bool,
    ladder: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    // THE ORDER IS THE COMPOSITION'S OWN, so a description that lost more than
    // one of them reports them in the order a reader would have met them, and
    // the ladder line is asked for FIRST or not at all, which is where it sits.
    let mut want = Vec::new();
    if ladder {
        want.push((
            String::from(text_ladder_line(ingredients)),
            "no line about a name in the list this game does not have",
        ));
    }
    want.push((
        text_format_line(ingredients),
        "no line about the format and the length limit",
    ));
    want.push((
        String::from(switch_line),
        "no line about which field decides while the text says default",
    ));
    want.push((
        String::from(TEXT_FALLBACK_LINE),
        "no line about what happens to a text this mod cannot use",
    ));
    for (line, missing) in want {
        if !localised_carries(desc, &line) {
            out.push(format!(
                "the library composes {} onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
                missing
            ));
        }
    }
    out
}

/// Whether a composed localised string holds this exact string as one of its
/// parameters, at any depth.
///
/// DEPTH BECAUSE THE QUESTION IS "DOES THE PLAYER READ IT", NOT "WHERE". The
/// composition it is handed is flat past the consumer's own key, which is
/// itself a nested table: [`text_description`] is at most six parameters and
/// never reaches the nesting rule, and a dropdown's composition is never handed
/// here at all. A top-level scan would therefore be a claim about the shape of
/// the composition rather than about the lines, and it would go quietly wrong
/// the day a line moves into a group. This asks only what the guard needs.
fn localised_carries(v: &Value, want: &str) -> bool {
    match v {
        Value::Str(s) => s == want,
        Value::Arr(items) => items.iter().any(|item| localised_carries(item, want)),
        _ => false,
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

/// The advisory's one sentence, and it is one function in each half so that
/// the two cannot drift apart a word at a time.
fn game_key_advisory(full: &str, key: &str, raw: &str) -> String {
    format!(
        "note: the dropdown setting {} composes the game's own key {}, which this plan does not own; where the game does not define it the tooltip shows {} instead, and defining it here would rename it for every mod",
        full, key, raw
    )
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
    use crate::plan::{
        CostChoice, CostChoices, CustomCost, Ingredient, IngredientChoice, IngredientChoices,
        ItemSpec, Lib, NumericSpec, Pack, RecipeSpec, TechSpec, UnitSpec,
    };
    use alloc::string::String;
    use alloc::vec::Vec;

    /// THE EXAMPLE GUEST'S OWN PLAN, which is what makes the golden below a
    /// cross-language pin over a real mod rather than over a sketch: every one
    /// of the seventeen settings both example guests declare, in the same
    /// order, with the same values, and enough of the recipes and
    /// technologies for the checker to see WHICH DROPDOWNS HAVE A TEXT SETTING
    /// BESIDE THEM. A dropdown that composes a preset list onto its description
    /// needs that description, and the scan that decides so reads the bindings.
    ///
    /// The items and recipes are the fewest that carry those bindings: the
    /// checker reads no ingredient and no cost, only who reads which setting.
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
        let medium =
            lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
        lib.bool_setting("bonus-research", true);
        let tier = lib.dropdown_setting_needing_locale(
            "tips-research-tier",
            "projectile",
            &["projectile", "military"],
        );
        lib.double_setting(
            "tempering-hold",
            1.5,
            NumericSpec {
                min: Some(0.5),
                max: None,
            },
        );

        let plate = lib.item("hardened-steel-plate", ItemSpec::default());
        let rivet = lib.item("steel-rivet", ItemSpec::default());

        let rivet_list = lib.ingredients_setting(
            "rivet-ingredients",
            alloc::vec![Ingredient::named(1, "iron-plate", &[])],
        );
        let quench_list = lib.ingredients_setting(
            "quench-ingredients",
            alloc::vec![
                Ingredient::named(2, "tungsten-plate", &["steel-plate"]),
                Ingredient::of(rivet, 4),
                Ingredient::named(1, "tungsten-carbide", &["titanium-plate"]),
            ],
        );
        let links = lib.dropdown_setting_needing_locale("chain-links", "short", &["short", "long"]);
        let chain_list =
            lib.ingredients_setting("chain-ingredients", alloc::vec![Ingredient::of(rivet, 4)]);
        let tips_packs = lib.packs_setting(
            "tips-packs",
            alloc::vec![
                Pack::new("automation-science-pack", 1),
                Pack::new("military-science-pack", 1),
            ],
        );
        let tips_count = lib.int_setting("tips-count", 0, NumericSpec::between(0.0, 100000.0));
        let tips_seconds = lib.int_setting("tips-seconds", 0, NumericSpec::between(0.0, 600.0));
        let chain_packs = lib.packs_setting(
            "chain-packs",
            alloc::vec![Pack::new("automation-science-pack", 1)],
        );
        let chain_count = lib.int_setting("chain-count", 20, NumericSpec::between(1.0, 100000.0));
        let chain_seconds = lib.int_setting("chain-seconds", 10, NumericSpec::between(1.0, 600.0));

        lib.recipe(
            rivet,
            RecipeSpec {
                ingredients_from: Some(rivet_list),
                ..Default::default()
            },
        );
        lib.recipe(
            plate,
            RecipeSpec {
                name: String::from("hardened-steel-plate-quenching"),
                category: String::from("crafting-with-fluid"),
                ingredients_by: Some(IngredientChoices {
                    describes: false,
                    setting: medium,
                    choices: alloc::vec![
                        IngredientChoice {
                            value: String::from("water"),
                            ingredients: alloc::vec![Ingredient::of(rivet, 4)],
                        },
                        IngredientChoice {
                            value: String::from("oil"),
                            ingredients: alloc::vec![Ingredient::of(rivet, 2)],
                        },
                    ],
                }),
                ingredients_from: Some(quench_list),
                ..Default::default()
            },
        );
        lib.recipe(
            rivet,
            RecipeSpec {
                name: String::from("steel-chain"),
                ingredients_by: Some(IngredientChoices {
                    describes: false,
                    setting: links,
                    choices: alloc::vec![
                        IngredientChoice {
                            value: String::from("short"),
                            ingredients: alloc::vec![Ingredient::of(rivet, 4)],
                        },
                        IngredientChoice {
                            value: String::from("long"),
                            ingredients: alloc::vec![Ingredient::of(rivet, 8)],
                        },
                    ],
                }),
                ingredients_from: Some(chain_list),
                ..Default::default()
            },
        );

        lib.technology(
            "hardened-tips",
            TechSpec {
                cost_by: Some(CostChoices {
                    describes: false,
                    setting: tier,
                    choices: alloc::vec![
                        CostChoice {
                            display: String::new(),
                            value: String::from("projectile"),
                            sources: alloc::vec![String::from("physical-projectile-damage-7")],
                        },
                        CostChoice {
                            display: String::new(),
                            value: String::from("military"),
                            sources: alloc::vec![String::from("military-4")],
                        },
                    ],
                    fallback: UnitSpec {
                        count: 200,
                        seconds: 30.0,
                        packs: alloc::vec![Pack::new("automation-science-pack", 1)],
                    },
                }),
                cost_from: Some(CustomCost {
                    packs: tips_packs,
                    count: tips_count,
                    seconds: tips_seconds,
                }),
                ..Default::default()
            },
        );
        lib.technology(
            "chain-forging",
            TechSpec {
                cost_from: Some(CustomCost {
                    packs: chain_packs,
                    count: chain_count,
                    seconds: chain_seconds,
                }),
                ..Default::default()
            },
        );
        lib
    }

    /// A SMALL PLAN for the rule tests below, which are about the checker's
    /// own rules rather than about this mod: five settings keep each expected
    /// report readable. The golden test runs the example's plan instead,
    /// because that one is pinned byte for byte across the two halves.
    fn a_few_settings() -> Lib {
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

    /// What this plan's cost dropdown composes out of the GAME's namespace, in
    /// composition order: one note per out-of-prefix key its presets
    /// reference. They are ADVISORIES rather than findings, so they are in no
    /// findings report at all, and no .cfg makes them appear or go away. PINNED
    /// HERE RATHER THAN IN A GOLDEN FILE, because the two halves carry the same
    /// two literal sentences and a third copy on disk would be a third place to
    /// forget.
    const ADVISORY_NOTES: &[&str] = &[
        "note: the dropdown setting fkrecipes-example-tips-research-tier composes the game's own key technology-name.physical-projectile-damage-7, which this plan does not own; where the game does not define it the tooltip shows physical-projectile-damage-7 instead, and defining it here would rename it for every mod",
        "note: the dropdown setting fkrecipes-example-tips-research-tier composes the game's own key technology-name.military-4, which this plan does not own; where the game does not define it the tooltip shows military-4 instead, and defining it here would rename it for every mod",
    ];

    /// THE OUT-OF-PREFIX KEY IS AN ADVISORY AND THE IN-PREFIX ONE IS REQUIRED,
    /// which is the whole distinction this rule turns on.
    ///
    /// FACTORIO'S LOCALE NAMESPACE IS FLAT AND SHARED. Defining
    /// `technology-name.military-4` in this mod's .cfg sets the displayed name
    /// of BASE's technology for every mod in the game, and this checker's own
    /// collision scan exists for exactly that hazard, so requiring the key
    /// would be requiring what the same checker flags. The note names the key,
    /// says what the tooltip shows where the game does not define it, and does
    /// not ask for an entry.
    ///
    /// AN ADVISORY IS NOT IN A FINDINGS REPORT AT ALL, and that is the property
    /// this test exists for. `docs/migration.md` tells a consumer to run
    /// `check_locale_with` from their own suite and that it should be CLEAN; an
    /// advisory in that return would be a permanent red test over a key the
    /// consumer is told NOT to define. So the accessor carries them and neither
    /// `check_locale` nor `check_locale_with` does, over a file that names
    /// nothing and over a file that names everything.
    ///
    /// IT READS NO .cfg, which is why the accessor takes none.
    #[test]
    fn check_locale_advisories() {
        let got = steelworks_settings().check_locale_advisories("fkrecipes-example");
        assert_eq!(got, ADVISORY_NOTES, "the advisories moved");

        let complete = read_testdata("../testdata/locale/example.cfg");
        for (name, cfg) in [("an empty file", ""), ("the fixture", complete.as_str())] {
            let reports = [
                (
                    "check_locale",
                    steelworks_settings().check_locale("fkrecipes-example", cfg),
                ),
                (
                    "check_locale_with",
                    steelworks_settings().check_locale_with("fkrecipes-example", cfg, &[]),
                ),
            ];
            for (call, report) in &reports {
                for (i, f) in report.iter().enumerate() {
                    assert!(
                        !f.starts_with("note: "),
                        "{} over {} carries an advisory at {}: {}",
                        call,
                        name,
                        i,
                        f
                    );
                }
            }
        }
    }

    /// THE ADVISORY IS ABOUT THE PLAN AND NOT ABOUT THE FILE, so a .cfg that
    /// DEFINES both of the game's keys, which is the squat the last clause
    /// warns about, gets the same two sentences. The accessor takes no cfg at
    /// all, which is what makes that unarguable; this holds the findings side
    /// of it, where a squatting file could have produced an orphan or a note
    /// and produces neither.
    #[test]
    fn check_locale_advisories_do_not_depend_on_the_file() {
        let squatting = "[technology-name]
physical-projectile-damage-7=Projectile damage 7
military-4=Military 4
";
        assert_eq!(
            steelworks_settings().check_locale_advisories("fkrecipes-example"),
            ADVISORY_NOTES,
            "the advisories moved"
        );
        for f in steelworks_settings().check_locale("fkrecipes-example", squatting) {
            assert!(
                !f.starts_with("note: "),
                "a squatting file put an advisory in the findings: {}",
                f
            );
        }
    }

    /// A plan whose one technology prices itself through a cost dropdown of
    /// `n` tiers, each naming its own source unless `sources` says otherwise.
    fn cost_ladder(sources: &[&str]) -> Lib {
        let mut lib = Lib::new();
        let values: Vec<String> = (0..sources.len())
            .map(|i| alloc::format!("t{}", i))
            .collect();
        let refs: Vec<&str> = values.iter().map(|v| v.as_str()).collect();
        let tier = lib.dropdown_setting_needing_locale("tier", refs[0], &refs);
        let packs = lib.packs_setting(
            "packs",
            alloc::vec![Pack::new("automation-science-pack", 1)],
        );
        let count = lib.int_setting("count", 0, NumericSpec::between(0.0, 100000.0));
        let seconds = lib.int_setting("seconds", 0, NumericSpec::between(0.0, 600.0));
        lib.technology(
            "hardened-tips",
            TechSpec {
                cost_by: Some(CostChoices {
                    describes: false,
                    setting: tier,
                    choices: sources
                        .iter()
                        .enumerate()
                        .map(|(i, s)| CostChoice {
                            display: String::new(),
                            value: values[i].clone(),
                            sources: alloc::vec![String::from(*s)],
                        })
                        .collect(),
                    fallback: UnitSpec {
                        count: 200,
                        seconds: 30.0,
                        packs: alloc::vec![Pack::new("automation-science-pack", 1)],
                    },
                }),
                cost_from: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                }),
                ..Default::default()
            },
        );
        lib
    }

    /// ONE DROPDOWN NAMING ONE KEY TWICE SAYS THE SENTENCE ONCE. A cost ladder
    /// may put the same first source under two tiers, which is an ordinary
    /// declaration and not a mistake, and the advisory is per (dropdown, key):
    /// without the dedupe the report repeats itself, and a consumer reading the
    /// same sentence twice learns nothing the second time.
    ///
    /// THE WITNESS IS THE COUNT AND THE ORDER TOGETHER: three tiers, two of
    /// them on the same source, produce two sentences, the repeated one at its
    /// FIRST position.
    #[test]
    fn check_locale_advisories_say_a_repeated_key_once() {
        let want = [
            super::game_key_advisory("mymod-tier", "technology-name.logistics", "logistics"),
            super::game_key_advisory("mymod-tier", "technology-name.military-4", "military-4"),
        ];
        let got =
            cost_ladder(&["logistics", "military-4", "logistics"]).check_locale_advisories("mymod");
        assert_eq!(got, want, "the advisories are not the deduped pair");
    }

    /// THE ADVISORIES HAVE A CAP OF THEIR OWN, with the shape the findings cap
    /// has and a budget no finding can spend: one dropdown with more
    /// distinctly-sourced choices than the cap takes is all it needs, and a
    /// plan can carry one.
    #[test]
    fn check_locale_advisories_are_capped() {
        let report = |n: usize| {
            let names: Vec<String> = (0..n).map(|i| alloc::format!("src{}", i)).collect();
            let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            cost_ladder(&refs).check_locale_advisories("mymod")
        };

        let got = report(super::LOCALE_ADVISORY_CAP + 50);
        assert_eq!(
            got.len(),
            super::LOCALE_ADVISORY_CAP + 1,
            "got {} advisories, want the cap plus one closing line",
            got.len()
        );
        assert_eq!(
            got.last().map(String::as_str),
            Some("(and 50 more advisories)")
        );

        // The boundary: exactly one advisory past the cap reads as one.
        let got = report(super::LOCALE_ADVISORY_CAP + 1);
        assert_eq!(got.len(), super::LOCALE_ADVISORY_CAP + 1);
        assert_eq!(
            got.last().map(String::as_str),
            Some("(and 1 more advisory)")
        );

        // One below it is not capped at all, so no closing line is added.
        let got = report(super::LOCALE_ADVISORY_CAP);
        assert_eq!(got.len(), super::LOCALE_ADVISORY_CAP);
        assert!(!got.last().unwrap().starts_with("(and "));
    }

    /// THE COMPOSITION'S CONDITION IS ONE FUNCTION AND THIS IS WHAT HOLDS ITS
    /// CLAUSES. `cost_dropdown_composes_preset_lines` decides, for a
    /// technology, whether its cost dropdown has a preset list composed onto
    /// its description, and THREE readers ask it: the composition in
    /// `composed_dropdown_presets`, the checker's required-description rule through
    /// the same function, and the advisory walk. One spelling means one place
    /// to break, and this is the test that notices.
    ///
    /// IT IS HERE BECAUSE CONSOLIDATION ALONE DID NOT COVER THEM, which was
    /// measured rather than assumed: with the spellings reduced to one,
    /// dropping `valid_packs_setting`, `valid_dropdown_setting` or the
    /// `named_cost_sources` clause each left the suite green, so the
    /// required-description findings do NOT exercise these guards and no
    /// fixture in the suite reaches them.
    ///
    /// A HANDLE FROM ANOTHER PLAN IS THE SHAPE, because it is the one an author
    /// actually produces and the one every validator in this library already
    /// names. The checker validates nothing by design, so it must SKIP such a
    /// declaration: it may not demand a description for a preset list nobody
    /// composes, and it may not say anything about the game's keys in lines
    /// nobody emits.
    #[test]
    fn a_cost_dropdown_with_a_handle_from_another_plan_composes_nothing() {
        let mut other = Lib::new();
        let foreign_dropdown = other.dropdown_setting_needing_locale("tier", "a", &["a", "b"]);
        let foreign_packs = other.packs_setting(
            "packs",
            alloc::vec![Pack::new("automation-science-pack", 1)],
        );

        let build = |use_foreign_dropdown: bool| {
            let mut lib = Lib::new();
            let mut tier = lib.dropdown_setting_needing_locale("tier", "a", &["a", "b"]);
            let mut packs = lib.packs_setting(
                "packs",
                alloc::vec![Pack::new("automation-science-pack", 1)],
            );
            let count = lib.int_setting("count", 0, NumericSpec::between(0.0, 100000.0));
            let seconds = lib.int_setting("seconds", 0, NumericSpec::between(0.0, 600.0));
            if use_foreign_dropdown {
                tier = foreign_dropdown;
            } else {
                packs = foreign_packs;
            }
            lib.technology(
                "hardened-tips",
                TechSpec {
                    cost_by: Some(CostChoices {
                        describes: false,
                        setting: tier,
                        choices: alloc::vec![
                            CostChoice {
                                display: String::new(),
                                value: String::from("a"),
                                sources: alloc::vec![String::from("logistics")],
                            },
                            CostChoice {
                                display: String::new(),
                                value: String::from("b"),
                                sources: alloc::vec![String::from("military-4")],
                            },
                        ],
                        fallback: UnitSpec {
                            count: 200,
                            seconds: 30.0,
                            packs: alloc::vec![Pack::new("automation-science-pack", 1)],
                        },
                    }),
                    cost_from: Some(CustomCost {
                        packs,
                        count,
                        seconds,
                    }),
                    ..Default::default()
                },
            );
            lib
        };

        for (name, foreign) in [
            ("the dropdown handle is another plan's", true),
            ("the packs handle is another plan's", false),
        ] {
            let lib = build(foreign);
            let advisories = lib.check_locale_advisories("mymod");
            assert!(
                advisories.is_empty(),
                "{}: a composition nobody emits produced advisories:\n{}",
                name,
                advisories.join("\n")
            );
            for f in lib.check_locale("mymod", "") {
                assert!(
                    !f.contains("composes its preset list onto that entry"),
                    "{}: a preset list nobody composes was required: {}",
                    name,
                    f
                );
            }
        }
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
        let findings = a_few_settings().check_locale("fkrecipes-example", cfg);
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
            let got = a_few_settings().check_locale("fkrecipes-example", c.cfg);
            let want: Vec<String> = c.want.iter().map(|s| String::from(*s)).collect();
            assert_eq!(got, want, "{}", c.name);
        }
    }

    /// The customizer's own plan: a text setting bound to a recipe, and a
    /// dropdown that offers the same recipe a text of its own.
    fn customizer_plan() -> Lib {
        use crate::plan::{Ingredient, IngredientChoice, IngredientChoices, ItemSpec, RecipeSpec};

        let mut lib = Lib::new();
        // A dropdown with NO text setting beside it, so the rule that a
        // description is optional for an ordinary dropdown keeps its witness.
        lib.dropdown_setting_needing_locale("smelting-style", "furnace", &["furnace", "foundry"]);
        let medium =
            lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
        let plate = lib.item("hardened-steel-plate", ItemSpec::default());
        let rivet = lib.item("steel-rivet", ItemSpec::default());
        let quench = lib.ingredients_setting(
            "quench-ingredients",
            alloc::vec![Ingredient::named(2, "steel-plate", &[])],
        );
        let rivets = lib.ingredients_setting(
            "rivet-ingredients",
            alloc::vec![Ingredient::named(1, "iron-plate", &[])],
        );
        lib.recipe(
            plate,
            RecipeSpec {
                ingredients_by: Some(IngredientChoices {
                    describes: false,
                    setting: medium,
                    choices: alloc::vec![
                        IngredientChoice {
                            value: String::from("water"),
                            ingredients: alloc::vec![Ingredient::named(2, "steel-plate", &[])],
                        },
                        IngredientChoice {
                            value: String::from("oil"),
                            ingredients: alloc::vec![Ingredient::named(2, "steel-plate", &[])],
                        },
                    ],
                }),
                ingredients_from: Some(quench),
                ..Default::default()
            },
        );
        lib.recipe(
            rivet,
            RecipeSpec {
                ingredients_from: Some(rivets),
                ..Default::default()
            },
        );
        lib
    }

    /// A DESCRIPTION IS NOT OPTIONAL FOR THESE TWO. A text setting's is where
    /// the player learns what the setting is for; the declared list, the
    /// format, the limits, the switch and the fallback are the library's own
    /// lines under it, so an absent entry loses all of them at once. A dropdown
    /// with a text setting beside it has the preset lines composed onto its
    /// own, so a missing entry renders a raw key in the tooltip.
    #[test]
    fn check_locale_requires_a_description_where_one_is_composed() {
        let cfg = "[mod-setting-name]
fkrecipes-example-smelting-style=Smelting
fkrecipes-example-quench-medium=Quenching medium
fkrecipes-example-quench-ingredients=Quenching ingredients
fkrecipes-example-rivet-ingredients=Rivet ingredients

[string-mod-setting]
fkrecipes-example-smelting-style-furnace=Furnace
fkrecipes-example-smelting-style-foundry=Foundry
fkrecipes-example-quench-medium-water=Water
fkrecipes-example-quench-medium-oil=Oil
";
        assert_eq!(
            customizer_plan().check_locale("fkrecipes-example", cfg),
            [
                "the dropdown setting fkrecipes-example-quench-medium has no [mod-setting-description] entry, and the library composes its preset list onto that entry",
                "the setting fkrecipes-example-quench-ingredients has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
                "the setting fkrecipes-example-rivet-ingredients has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
            ]
        );
    }

    /// THE CHECKER ASKS FOR EXACTLY WHAT THE SETTINGS STAGE COMPOSES, and it
    /// asks through the same walk: a recipe naming `Ingredients` beside
    /// `IngredientsBy` is one the binding validator steps past, so no preset
    /// list is composed onto its dropdown and no description is demanded for
    /// it. A finding about a description nothing writes to is one the author
    /// cannot act on.
    #[test]
    fn check_locale_asks_for_no_description_where_none_is_composed() {
        use crate::plan::{Ingredient, IngredientChoice, IngredientChoices, ItemSpec, RecipeSpec};

        let mut lib = Lib::new();
        let medium = lib.dropdown_setting_needing_locale("quench-medium", "water", &["water"]);
        let plate = lib.item("hardened-steel-plate", ItemSpec::default());
        let quench = lib.ingredients_setting(
            "quench-ingredients",
            alloc::vec![Ingredient::named(2, "steel-plate", &[])],
        );
        lib.recipe(
            plate,
            RecipeSpec {
                // The declaration the data planner answers with "pick one".
                ingredients: alloc::vec![Ingredient::named(2, "steel-plate", &[])],
                ingredients_by: Some(IngredientChoices {
                    describes: false,
                    setting: medium,
                    choices: alloc::vec![IngredientChoice {
                        value: String::from("water"),
                        ingredients: alloc::vec![Ingredient::named(2, "steel-plate", &[])],
                    }],
                }),
                ingredients_from: Some(quench),
                ..Default::default()
            },
        );

        let cfg = "[mod-setting-name]
fkrecipes-example-quench-medium=Quenching medium
fkrecipes-example-quench-ingredients=Quenching ingredients

[mod-setting-description]
fkrecipes-example-quench-ingredients=Amount then name, separated by commas.

[string-mod-setting]
fkrecipes-example-quench-medium-water=Water
";
        assert_eq!(
            lib.check_locale("fkrecipes-example", cfg),
            Vec::<String>::new()
        );
    }

    /// THE DRIFT GUARD OVER WHAT THE LIBRARY ITSELF COMPOSES. The consumer's
    /// entry is checked above; these lines are this library's, so no locale
    /// file can put one back and no plan can leave one out. What the rule can
    /// see is a composition that stopped carrying a line, which is why the
    /// composition is what it is handed.
    ///
    /// THE SENTENCE NAMES THE LIBRARY AND NOT A SETTING, because the rule's
    /// input does not vary with the setting in any way the rule reads: one
    /// line is a constant, the others are the switch line and the two the KIND
    /// decides, the ladder line and the format line, all of which the
    /// composition was built with and which are handed in beside it, and the
    /// only per-setting part of a text description is the consumer's key, which
    /// the rule does not look at. One defect is therefore one finding.
    ///
    /// THE LADDER FLAG IS THE FOURTH INPUT AND BOTH OF ITS ARMS ARE WALKED
    /// HERE. A text setting that discloses the ladder carries the line and the
    /// rule looks for it; one beside an ingredient dropdown does not, and a
    /// rule that looked for it anyway would report a defect on every plan the
    /// customizer was designed for.
    ///
    /// THE HEALTHY PATH FIRST, so a rule that fired on everything would be
    /// caught here rather than in a golden somewhere: the real composition
    /// reports nothing.
    #[test]
    fn composed_text_lines_missing_guards_the_composed_lines() {
        use crate::locale::composed_text_lines_missing;
        use crate::settings::{
            text_description, text_format_line, text_ladder_line, TEXT_FALLBACK_LINE,
        };
        use crate::value::Value;

        const FULL: &str = "steelworks-axe-ingredients";
        const SWITCH: &str =
            "\nLeave this as default and this mod's own list applies; anything else applies instead.";
        let whole = text_description(FULL, "1 steel-plate", SWITCH, true, true);
        assert_eq!(
            composed_text_lines_missing(&whole, SWITCH, true, true),
            Vec::<String>::new()
        );
        // AND THE PACKS COMPOSITION IS CLEAN UNDER THE PACKS RULE. The ladder
        // line and the format line are the two lines whose bytes depend on the
        // kind, so a rule asked about the wrong kind reports a healthy
        // description as broken.
        let packs = text_description(
            "steelworks-axe-packs",
            "1 automation-science-pack",
            SWITCH,
            false,
            true,
        );
        assert_eq!(
            composed_text_lines_missing(&packs, SWITCH, false, true),
            Vec::<String>::new()
        );
        // AND THE COMPOSITION BESIDE A DROPDOWN IS CLEAN UNDER THE RULE THAT
        // KNOWS IT HAS NO LADDER LINE. This is the shape the customizer was
        // designed for, so a guard that got this wrong would fire on the
        // commonest plan there is.
        const BESIDE: &str =
            "\nLeave this as default and the option chosen above decides; anything else applies instead.";
        let beside = text_description(FULL, "1 steel-plate", BESIDE, true, false);
        assert_eq!(
            composed_text_lines_missing(&beside, BESIDE, true, false),
            Vec::<String>::new()
        );
        // AND THE SAME COMPOSITION UNDER THE OTHER ARM IS NOT CLEAN, which is
        // what says the flag is read at all: a rule told to expect a ladder
        // line over a description that correctly has none reports that line.
        assert_eq!(
            composed_text_lines_missing(&beside, BESIDE, true, true),
            ["the library composes no line about a name in the list this game does not have onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file"]
        );

        // The same composition with one line taken out of it, which is the
        // only way to reach the finding: nothing a consumer declares composes
        // a description missing a line.
        let without = |line: &str| match &whole {
            Value::Arr(items) => Value::Arr(
                items
                    .iter()
                    .filter(|v| !matches!(v, Value::Str(s) if s == line))
                    .cloned()
                    .collect(),
            ),
            _ => unreachable!("the composition is an array"),
        };
        assert_eq!(
            composed_text_lines_missing(&without(text_ladder_line(true)), SWITCH, true, true),
            ["the library composes no line about a name in the list this game does not have onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file"]
        );
        assert_eq!(
            composed_text_lines_missing(&without(&text_format_line(true)), SWITCH, true, true),
            ["the library composes no line about the format and the length limit onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file"]
        );
        assert_eq!(
            composed_text_lines_missing(&without(SWITCH), SWITCH, true, true),
            ["the library composes no line about which field decides while the text says default onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file"]
        );
        assert_eq!(
            composed_text_lines_missing(&without(TEXT_FALLBACK_LINE), SWITCH, true, true),
            ["the library composes no line about what happens to a text this mod cannot use onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file"]
        );
        // THE LADDER LINE AND THE FORMAT LINE ARE PER KIND, and the rule asked
        // about the wrong kind sees two lines it does not recognise: an
        // ingredient composition read as a packs one is missing the packs
        // ladder line and the packs format line, exactly as if both had been
        // deleted.
        assert_eq!(
            composed_text_lines_missing(&whole, SWITCH, false, true),
            [
                "the library composes no line about a name in the list this game does not have onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
                "the library composes no line about the format and the length limit onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
            ]
        );
        // All four gone: all four reported, in the order the lines sit in,
        // which is the order every other rule here reports in.
        let stripped = Value::Arr(alloc::vec![
            Value::string(""),
            Value::Arr(alloc::vec![Value::Str(format!(
                "mod-setting-description.{}",
                FULL
            ))]),
        ]);
        assert_eq!(
            composed_text_lines_missing(&stripped, SWITCH, true, true),
            [
                "the library composes no line about a name in the list this game does not have onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
                "the library composes no line about the format and the length limit onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
                "the library composes no line about which field decides while the text says default onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
                "the library composes no line about what happens to a text this mod cannot use onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
            ]
        );
    }

    /// THE GUARD IS HANDED ONE COMPOSITION, NOT ONE PER SETTING, and it is the
    /// first text setting's in declaration order. A plan that declares no text
    /// setting composes no text description at all, so there is nothing for a
    /// line to have gone missing from and the guard has nothing to inspect.
    ///
    /// THE INPUT IS WHAT THIS PINS, because the finding itself is unreachable
    /// from a healthy library: the sentences are pinned above and the cap
    /// defect was about how many times this input is taken, not about what the
    /// rule says. Run once per text setting it turned one library defect into
    /// five findings on the example guest.
    ///
    /// AND THE LADDER FLAG COMES BACK WITH IT, off a real plan in all three of
    /// its shapes: a standalone text setting carries the ladder line, one
    /// beside an INGREDIENT dropdown does not, and a packs setting beside a
    /// COST dropdown does, because that dropdown composes no ladder line of its
    /// own.
    #[test]
    fn the_drift_guard_inspects_the_first_text_setting_only() {
        use crate::plan::{Ingredient, ItemSpec, Pack};
        use crate::settings::text_description;

        let mut lib = Lib::new();
        lib.bool_setting("hint", true);
        assert_eq!(lib.guarded_text_description("steelworks-"), None);

        let axe = lib.item("steel-axe", ItemSpec::default());
        lib.ingredients_setting("axe-ingredients", alloc::vec![Ingredient::of(axe, 1)]);
        lib.packs_setting(
            "axe-packs",
            alloc::vec![Pack::named(1, "automation-science-pack", &[])],
        );
        // The FIRST one's, and with the list left out: the packs setting
        // declared after it is not what the guard reads, and neither is any
        // rendering. AND THE LINE AND THE KIND COME BACK BESIDE IT, because the
        // rule cannot recompute a line that names a neighbouring setting, nor
        // the format line whose bytes depend on which of the two kinds this is.
        const OWN: &str =
            "\nLeave this as default and this mod's own list applies; anything else applies instead.";
        assert_eq!(
            lib.guarded_text_description("steelworks-"),
            Some((
                text_description("steelworks-axe-ingredients", "", OWN, true, true),
                String::from(OWN),
                true,
                true
            ))
        );

        // AND THE OTHER ARM, off a real plan: a text setting with a dropdown
        // beside it hands the guard a composition with no ladder line in it,
        // and the flag that says so. Deriving that from the switch line's
        // wording instead would be a second spelling of `text_switch_dropdown`.
        const BESIDE: &str =
            "\nLeave this as default and the option chosen above decides; anything else applies instead.";
        assert_eq!(
            customizer_plan().guarded_text_description("fkrecipes-example-"),
            Some((
                text_description(
                    "fkrecipes-example-quench-ingredients",
                    "",
                    BESIDE,
                    true,
                    false
                ),
                String::from(BESIDE),
                true,
                false
            ))
        );

        // AND THE THIRD SHAPE, which is the one that says the flag follows the
        // dropdown's KIND rather than its presence: a packs text setting
        // beside a COST dropdown is handed a composition that DOES carry the
        // ladder line, and the flag that says to look for it. A guard keyed on
        // "is there a dropdown" answers false here and would stop watching the
        // line on every research-cost plan there is.
        const COST: &str =
            "\nLeave this as default and the option chosen above decides; anything else applies instead.";
        assert_eq!(
            packs_beside_cost_dropdown_plan().guarded_text_description("steelworks-"),
            Some((
                text_description("steelworks-tips-packs", "", COST, false, true),
                String::from(COST),
                false,
                true
            ))
        );
    }

    /// A packs text setting whose technology also names a COST dropdown.
    ///
    /// A cost dropdown's presets are a localised label and a localised
    /// technology name, so it composes no ladder line of its own and the packs
    /// field beside it is the only one of the two where the ladder can be
    /// disclosed. The twin of this plan is in `tests::customize`, which is
    /// another module and builds its own.
    fn packs_beside_cost_dropdown_plan() -> Lib {
        use crate::plan::{
            CostChoice, CostChoices, CustomCost, NumericSpec, Pack, TechSpec, UnitSpec,
        };

        let mut lib = Lib::new();
        let tier =
            lib.dropdown_setting_needing_locale("tips-tier", "projectile", &["projectile", "none"]);
        let packs = lib.packs_setting(
            "tips-packs",
            alloc::vec![Pack::named(1, "automation-science-pack", &[])],
        );
        let count = lib.int_setting("tips-count", 0, NumericSpec::between(0.0, 100000.0));
        let seconds = lib.int_setting("tips-seconds", 0, NumericSpec::between(0.0, 600.0));
        lib.technology(
            "hardened-tips",
            TechSpec {
                cost_by: Some(CostChoices {
                    describes: false,
                    setting: tier,
                    choices: alloc::vec![
                        CostChoice {
                            display: String::new(),
                            value: String::from("projectile"),
                            sources: alloc::vec![String::from("mining-productivity-4")],
                        },
                        CostChoice {
                            display: String::new(),
                            value: String::from("none"),
                            sources: alloc::vec![],
                        },
                    ],
                    fallback: UnitSpec {
                        count: 200,
                        seconds: 30.0,
                        packs: alloc::vec![Pack::named(1, "automation-science-pack", &[])],
                    },
                }),
                cost_from: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                }),
                ..Default::default()
            },
        );
        lib
    }

    /// The same file with the three descriptions written: clean. The ordinary
    /// dropdown still needs none, which is what says the new rule is scoped
    /// rather than a blanket one.
    #[test]
    fn check_locale_accepts_a_complete_customizer_file() {
        let cfg = "[mod-setting-name]
fkrecipes-example-smelting-style=Smelting
fkrecipes-example-quench-medium=Quenching medium
fkrecipes-example-quench-ingredients=Quenching ingredients
fkrecipes-example-rivet-ingredients=Rivet ingredients

[mod-setting-description]
fkrecipes-example-quench-medium=What the plate is quenched in.
fkrecipes-example-quench-ingredients=Amount then name, separated by commas.
fkrecipes-example-rivet-ingredients=Amount then name, separated by commas.

[string-mod-setting]
fkrecipes-example-smelting-style-furnace=Furnace
fkrecipes-example-smelting-style-foundry=Foundry
fkrecipes-example-quench-medium-water=Water
fkrecipes-example-quench-medium-oil=Oil
";
        let findings = customizer_plan().check_locale("fkrecipes-example", cfg);
        assert!(
            findings.is_empty(),
            "a complete file produced findings:\n{}",
            findings.join("\n")
        );
    }

    /// A RESEARCH NUMBER'S DESCRIPTION IS REQUIRED TOO, for the reason a text
    /// setting's is: the range and what 0 means are composed onto that entry,
    /// and an absent one loses both along with whatever the consumer meant to
    /// say about what the number is for. The Go half pins the same pair.
    #[test]
    fn check_locale_requires_a_research_number_description() {
        use crate::plan::{CustomCost, Lib, NumericSpec, Pack, TechSpec};

        let mut lib = Lib::new();
        let packs = lib.packs_setting(
            "axe-packs",
            alloc::vec![Pack::new("automation-science-pack", 1)],
        );
        let count = lib.int_setting("axe-count", 20, NumericSpec::between(1.0, 100000.0));
        let seconds = lib.int_setting("axe-seconds", 10, NumericSpec::between(1.0, 600.0));
        lib.technology(
            "steel-axes",
            TechSpec {
                cost_from: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                }),
                ..Default::default()
            },
        );

        let cfg = "[mod-setting-name]
fkrecipes-example-axe-packs=Science packs
fkrecipes-example-axe-count=Research count
fkrecipes-example-axe-seconds=Research seconds

[mod-setting-description]
fkrecipes-example-axe-packs=Amount, then name, commas between.
";
        assert_eq!(
            lib.check_locale("fkrecipes-example", cfg),
            [
                "the setting fkrecipes-example-axe-count has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
                "the setting fkrecipes-example-axe-seconds has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
            ]
        );

        let full = alloc::format!(
            "{}fkrecipes-example-axe-count=How many units.\nfkrecipes-example-axe-seconds=Seconds per unit.\n",
            cfg
        );
        assert_eq!(
            lib.check_locale("fkrecipes-example", &full),
            Vec::<String>::new()
        );
    }

    /// A blank description renders as nothing, which is the defect the player
    /// meets, so it counts as missing here exactly as it does for a name.
    #[test]
    fn check_locale_counts_a_blank_description_as_missing() {
        let cfg = "[mod-setting-name]
fkrecipes-example-rivet-ingredients=Rivet ingredients

[mod-setting-description]
fkrecipes-example-rivet-ingredients=
";
        let got = customizer_plan().check_locale("fkrecipes-example", cfg);
        assert!(
            got.iter().any(|f| f
                == "the setting fkrecipes-example-rivet-ingredients has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it"),
            "a blank description was accepted: {:?}",
            got
        );
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
            let got = a_few_settings().check_locale("fkrecipes-example", c.cfg);
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
            let got = a_few_settings().check_locale("fkrecipes-example", &cfg);
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
        let got = a_few_settings().check_locale("fkrecipes-example", cfg);

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
        let got = a_few_settings().check_locale("fkrecipes-example", cfg);

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
            a_few_settings().check_locale("fkrecipes-example", &cfg)
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
