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
// back. Of the World it asks only ModName and StageName, so the emit layer
// may pass one that answers the data-stage questions emptily.
//
// This is the seam the emit layer stands on at the settings stage; consumers
// call Emit and never this. It is exported so a consumer's own tests can hold
// a plan up to the light without a wasm toolchain.
func (l *Lib) PlanSettings(w World) ([]Op, error) {
	// A nil World is a Go-only hazard: the Rust mirror takes &dyn World,
	// which cannot be null, so it needs no guard.
	if w == nil {
		return nil, errors.New("fkrecipes: PlanSettings was given a nil World")
	}
	stage := w.StageName()
	if l.id == 0 {
		return nil, errors.New("fkrecipes: at the " + stage + " stage, this Lib was built without New, so its handles cannot be validated")
	}
	modName := w.ModName()
	if modName == "" {
		return nil, errors.New("fkrecipes: at the " + stage + " stage, the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName")
	}
	prefix := modName + "-"
	bound := l.craftTimeBoundSettings()
	if err := l.validateSettings(stage, prefix, bound); err != nil {
		return nil, err
	}
	ops := make([]Op, 0, len(l.settings))
	for i, s := range l.settings {
		// A legacy setting carries the name and the order the mod already
		// ships; everything else is prefixed and ordered by declaration.
		order := orderString(i)
		if s.legacy {
			order = s.order
		}
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
func (l *Lib) validateSettings(stage, prefix string, bound []bool) error {
	at := "fkrecipes: at the " + stage + " stage, "
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
	return nil
}

func (s settingDecl) defaultValue() Value {
	switch s.kind {
	case settingBool:
		return Bool(s.defBool)
	case settingDropdown:
		return Str(s.defStr)
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
