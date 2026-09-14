package fkrecipes

import (
	"errors"
	"strconv"
)

// PlanSettings turns the declared settings into one Extend op per setting
// prototype, in declaration order, with every name prefixed.
//
// It takes the same World the data half takes, and the prefix comes from it,
// NOT from a parameter: the two stages have to agree on a setting's name to
// the byte, and a name passed in here can drift from the one PlanData reads
// back. It asks only for the MOD NAME, which is why its parameter is Named
// rather than World: a consumer's host test of their settings implements one
// method, and World embeds Named so the emit layer hands the same value to
// both planners.
//
// This is the seam the emit layer stands on at the settings stage; consumers
// call Emit and never this. It is exported so a consumer's own tests can hold
// a plan up to the light without a wasm toolchain.
func (l *Lib) PlanSettings(w Named) ([]Op, error) {
	// A nil World is a Go-only hazard: the Rust mirror takes &dyn World,
	// which cannot be null, so it needs no guard.
	if w == nil {
		return nil, errors.New("fkrecipes: PlanSettings was given a nil World")
	}
	if l.id == 0 {
		return nil, errors.New("fkrecipes: this Lib was built without New, so its handles cannot be validated")
	}
	modName := w.ModName()
	if modName == "" {
		return nil, errors.New("fkrecipes: the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName")
	}
	prefix := modName + "-"
	bound := l.craftTimeBoundSettings()
	if err := l.validateSettings(prefix, bound); err != nil {
		return nil, err
	}
	// THE SETTINGS STAGE VALIDATES BINDINGS TOO, and it has to: a text
	// setting's default is rendered into its description here, and a dropdown
	// with a text setting beside it has its whole preset list composed here, and
	// a research number its range. All of them read the
	// recipes and technologies, so both need them well formed. The two
	// validators are shared with the data planner rather than written twice.
	if err := l.validateBindings(prefix); err != nil {
		return nil, err
	}
	if err := l.validateTextSettings(prefix); err != nil {
		return nil, err
	}
	descriptions := l.settingDescriptions(prefix)
	ops := make([]Op, 0, len(l.settings))
	for i, s := range l.settings {
		// A legacy setting carries the name and the order the mod already
		// ships; everything else is prefixed and ordered by declaration,
		// under whatever prefix OrderAfter had in force when it was declared.
		order := s.emittedOrder(i)
		pairs := []KV{
			kv("type", Str(settingTypeName(s.kind))),
			kv("name", Str(s.emittedName(prefix))),
			kv("setting_type", Str("startup")),
			kv("default_value", s.defaultValue()),
			kv("order", Str(order)),
		}
		if s.kind == settingInt || s.kind == settingDouble {
			spec := l.effectiveNumericSpec(i, bound)
			if spec.HasMin {
				pairs = append(pairs, kv("minimum_value", Num(spec.Min)))
			}
			if spec.HasMax {
				pairs = append(pairs, kv("maximum_value", Num(spec.Max)))
			}
		}
		if s.kind == settingDropdown {
			pairs = append(pairs, kv("allowed_values", strArr(s.values)))
		}
		// A TEXT SETTING CARRIES NEITHER allowed_values NOR allow_blank, and
		// that is measured rather than stylistic: allowed_values would turn a
		// free-text field into a picker, and the engine RESETS a stored empty
		// or blank text to the default before any stage runs, so allow_blank
		// would be the one way to reach a blank value the parser then has to
		// refuse. auto_trim is a GUI courtesy: it tidies what the player sees
		// and leaves what mod-settings.dat holds untouched (measured, both
		// space runs read back), so the parser trims for itself anyway.
		if s.kind.isText() {
			pairs = append(pairs, kv("auto_trim", Bool(true)))
		}
		// LAST, because it is the bulkiest field and because it is composed
		// out of everything above it. Nil for the settings that carry none,
		// which is every kind but the three that compose one: a text setting, a
		// research number, and a dropdown that has a text setting bound to the
		// same recipe or technology.
		if descriptions[i].Kind != KindNil {
			pairs = append(pairs, kv("localised_description", descriptions[i]))
		}
		ops = append(ops, extendOp(Obj(pairs...)))
	}
	return ops, nil
}

// validateSettings returns the FIRST refusal, scanning in declaration order.
// The engine's own answers are why each one exists: two settings of the same
// type sharing a name is silent last-writer-wins, and a default outside the
// allowed values or the bounds is refused at load with no mod named.
//
// No refusal here prints a number. A float rendered by two languages is two
// different strings sooner or later, and these messages are compared byte for
// byte, so each one names the setting and the relationship instead.
func (l *Lib) validateSettings(prefix string, bound []bool) error {
	at := "fkrecipes: "
	// BEFORE THE LOOP, because an empty OrderAfter is a mistake in the plan
	// rather than in any one setting: the call is wrong whether or not a
	// setting was declared after it, and a plan that declared none would
	// otherwise carry it silently until somebody added one. The message names
	// the GO NAME in both halves, as the language guard's does: the surface
	// is one surface and the sentence is compared byte for byte.
	if l.orderAfterEmpty {
		return errors.New(at + "OrderAfter was given an empty order; name the order string the generated settings should follow")
	}
	for i, s := range l.settings {
		if s.name == "" {
			return errors.New(at + "a setting was declared with an empty name")
		}
		// A legacy setting supplies its own order because a mod that already
		// shipped chose one; an empty string is not a choice.
		if s.legacy && s.order == "" {
			return errors.New(at + "the legacy setting " + s.name + " was declared with an empty order")
		}
		// Compared on the EMITTED names, which is the namespace the engine
		// keeps: a legacy name and a generated one can arrive at the same
		// string from different declarations, and only one of them survives.
		for j := 0; j < i; j++ {
			if l.settings[j].emittedName(prefix) == s.emittedName(prefix) {
				return errors.New(at + "two settings share the name " + s.emittedName(prefix) + "; the engine keeps the last one silently")
			}
		}
		switch s.kind {
		case settingDropdown:
			found := false
			for _, v := range s.values {
				if v == s.defStr {
					found = true
					break
				}
			}
			if !found {
				return errors.New(at + "the dropdown setting " + s.name + " defaults to " + s.defStr + ", which is not one of its allowed values")
			}
		case settingInt, settingDouble:
			// The declared integer first, while it is still an integer: past
			// 2^53 the conversion to double has already rounded it, so the
			// setting the player sees is not the one the consumer wrote. The
			// bound is on magnitude because an int setting's default is
			// legitimately negative and rounds just the same below -2^53.
			if s.kind == settingInt && (s.defInt > maxExactInt || s.defInt < -maxExactInt) {
				return errors.New(at + "the numeric setting " + s.name + " declares a default a Lua double cannot hold exactly: " + strconv.FormatInt(s.defInt, 10))
			}
			// Finiteness next: every comparison below is meaningless against
			// a NaN, and an infinity would reach a prototype.
			if !finite(s.defNum) || (s.spec.HasMin && !finite(s.spec.Min)) || (s.spec.HasMax && !finite(s.spec.Max)) {
				return errors.New(at + "the numeric setting " + s.name + " declares a value that is not a finite number")
			}
			// A setting the player turns into a crafting time may not offer a
			// value the engine refuses, so its own minimum has to clear the
			// floor. Nothing here prints a number: the floor appears once, as
			// literal text inside the message.
			if bound[i] && s.spec.HasMin && s.spec.Min <= craftTimeFloor {
				return errors.New(at + "the setting " + s.name + " backs a crafting time but declares a minimum at or below the engine floor (energy_required can't be <= 0.001)")
			}
			// The EFFECTIVE bounds, so a generated minimum is checked against
			// the default exactly as a declared one is. A GENERATED minimum
			// says so in its own refusals: blaming a "declared minimum" the
			// consumer never wrote sends them looking for the wrong line.
			spec := l.effectiveNumericSpec(i, bound)
			if bound[i] && !s.spec.HasMin {
				if s.spec.HasMax && spec.Min > s.spec.Max {
					return errors.New(at + "the setting " + s.name + " backs a crafting time, so its minimum is 0.002, which is above the declared maximum")
				}
				if s.defNum < spec.Min {
					return errors.New(at + "the setting " + s.name + " backs a crafting time, so its minimum is 0.002, which is above the declared default")
				}
				if s.spec.HasMax && s.defNum > s.spec.Max {
					return errors.New(at + "the numeric setting " + s.name + " declares a default outside its own minimum and maximum")
				}
			} else {
				if spec.HasMin && spec.HasMax && spec.Min > spec.Max {
					return errors.New(at + "the numeric setting " + s.name + " declares a minimum above its maximum")
				}
				if (spec.HasMin && s.defNum < spec.Min) || (spec.HasMax && s.defNum > spec.Max) {
					return errors.New(at + "the numeric setting " + s.name + " declares a default outside its own minimum and maximum")
				}
			}
		}
	}
	// THE ORDER PASS RUNS AFTER THE LOOP ABOVE RATHER THAN INSIDE IT, because
	// every refusal it can give QUOTES A SECOND SETTING. Inside the loop it
	// reached settings the loop had not validated yet, so a plan whose real
	// mistake was an empty name got a sentence with a blank where a name
	// belongs, and five refusals a consumer needs (an empty legacy order, a
	// later empty name, a dropdown default outside its values, a craft-time
	// minimum at the floor, an int default past what a double holds) were
	// pre-empted by a placement remark. Every setting named below has passed
	// its own checks.
	//
	// GENERATED SETTINGS IN DECLARATION ORDER, and inside each of them the
	// legacy settings in declaration order, so a plan with two order problems
	// always answers with the same one.
	//
	// TWO GENERATED SETTINGS ARE NEVER COMPARED. The two letters are always
	// two letters, so two equal emitted orders force equal prefixes, and a
	// tie under one prefix takes the 676 the letters count to, which
	// orderString documents as cosmetic.
	//
	// THIS REFUSES PLANS THAT LOADED BEFORE, deliberately: a legacy order of
	// "aa" beside a generated first setting has always tied, silently, and is
	// a refusal from here on. The migration notes say so.
	for i, s := range l.settings {
		if s.legacy {
			continue
		}
		order := s.emittedOrder(i)
		for j := range l.settings {
			other := l.settings[j]
			if !other.legacy {
				continue
			}
			legacyOrder := other.emittedOrder(j)
			// THE TIE FIRST for this legacy setting and the placement second.
			// Only one of the two can hold: a legacy order equal to the
			// generated one does not sort before it.
			if legacyOrder == order {
				return errors.New(at + "the setting " + s.name + " would carry the order " + order +
					", which the legacy setting " + other.name + " already carries; give one of them an order of its own")
			}
			// THE PLACEMENT OrderAfter PROMISES, which equality alone does not
			// keep: a legacy order that EXTENDS the named one sits inside the
			// range the two letters walk, so a generated setting far enough
			// along the alphabet sorts past it. Under OrderAfter("a") beside
			// a legacy "ab", the generated setting whose two letters are "ba"
			// carries "aba" and lands past it with nothing said.
			//
			// A PLAN WITH NO OrderAfter MADE NO SUCH PROMISE, which is why an
			// empty prefix is excluded rather than treated as one every
			// legacy order extends: a legacy "a" beside the generated "aa" is
			// the two letters' arithmetic working as documented.
			if p := s.orderPrefix; p != "" && len(legacyOrder) > len(p) && legacyOrder[:len(p)] == p && legacyOrder < order {
				return errors.New(at + "the setting " + s.name + " would carry the order " + order +
					" and sort past the legacy setting " + other.name + " at " + legacyOrder +
					", which extends " + p + "; OrderAfter(" + p + ") places settings before every legacy order that extends " + p)
			}
		}
	}
	return nil
}

func (s settingDecl) defaultValue() Value {
	switch s.kind {
	case settingBool:
		return Bool(s.defBool)
	case settingDropdown:
		return Str(s.defStr)
	case settingIngredients, settingPacks:
		// THE RESERVED WORD, never the rendered list. See IngredientsSetting:
		// the engine stores every setting's current value including untouched
		// defaults, so a rendered default would freeze every silent player's
		// balance at the day they installed the mod.
		return Str(defaultWord)
	default:
		return Num(s.defNum)
	}
}

func settingTypeName(k settingKind) string {
	switch k {
	case settingBool:
		return "bool-setting"
	case settingInt:
		return "int-setting"
	case settingDouble:
		return "double-setting"
	default:
		return "string-setting"
	}
}

// orderString is the settings screen's sort key: two base-26 letters from the
// declaration index, so the screen shows the order the consumer wrote.
//
// Two letters is 676 startup settings, past anything real; beyond that the
// strings repeat and the engine breaks the tie by name, which is cosmetic.
// The arithmetic is integer on purpose: it must agree with the Rust mirror.
func orderString(i int) string {
	i = i % (26 * 26)
	return string([]byte{byte('a' + i/26), byte('a' + i%26)})
}
