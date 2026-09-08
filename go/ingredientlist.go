package fkrecipes

import (
	"math"
	"strconv"
	"strings"
	"unicode/utf8"
)

// THE INGREDIENT LIST: the little language a player types into a startup
// setting to say what a recipe is made of, or what a research costs.
//
// docs/ingredient-list.md is the player-facing reference and
// testdata/ingredient-list/cases.txt is the CONTRACT: every case in it is an
// input and the exact text this file must produce, and the Rust mirror runs
// the same file. A message changed here without the corpus is a red suite in
// both languages, which is the point.
//
// THE SHAPE OF THE PASS, and why it is this one:
//
//   - The WHOLE TEXT is judged first, before any of it is read as a list: not
//     text at all, then too long, then invisible characters stripped, then
//     empty, then the two reserved words. Each of those is about the setting
//     rather than about anything inside it, and a text that fails one of them
//     has no entries worth quoting back.
//   - Then split on commas outside a rich-text tag, then read each entry on
//     its own, then look for duplicates across entries. Each stage refuses
//     with the FIRST problem it finds and the stages run in that order, so two
//     languages diagnosing one malformed text say the same sentence.
//   - An entry is lexed into three token kinds (an amount, a name, a sign) and
//     then diagnosed in a FIXED order: a stray character, then no name, then
//     two amounts, then two names, then the sign rules, then the amount's
//     value, then what the name resolves to, and only then the kinds. The
//     order is the contract; see parseEntry.
//   - Every message quotes the entry as typed and numbers it from 1. A
//     character offset would be two different numbers in two languages the
//     moment a player types a non-ASCII character, and an entry number plus
//     the quoted text is exact in both.
//   - NOTHING IS GUESSED. A name the game does not have is a refusal, with the
//     case-folded or dash-folded name offered when the game has THAT, because
//     a silent substitute would hide the player's typo behind a recipe they
//     did not ask for.
//
// The parser answers with a message rather than an error: the caller composes
// it into the refusal the host raises, and a string keeps this half free of
// any opinion about how the consumer's stage fails. An empty message is the
// success.

// listWhitespace is the whole whitespace set this language knows, for trimming
// and for splitting an entry into words.
//
// A CLOSED SET, MEASURED, not a Unicode class. The stored text arrives verbatim
// (a setting with auto_trim=true read back "  3 iron-plate , 0.5 [fluid=water]  "
// with both space runs, because auto_trim is a GUI behaviour and does not touch
// what mod-settings.dat holds), so the parser trims for itself, and two
// languages agreeing on a whole Unicode space class is a promise neither can
// keep cheaply. These eight are ASCII space, tab, CR and LF plus the four
// spaces a paste from a document or a spreadsheet actually carries: no-break
// space, figure space, narrow no-break space and ideographic space. Every other
// invisible character is REFUSED by code point rather than silently eaten,
// because a player cannot see what they pasted and needs to be told.
const listWhitespace = "\u0009\u000a\u000d\u0020\u00a0\u2007\u202f\u3000"

// listStripped is deleted wherever it occurs, before anything else looks at the
// text: the byte-order mark and the three zero-width joiners and non-joiners.
// They carry no meaning here, they arrive from copying a name out of a wiki
// page, and a player cannot delete a character they cannot see.
const listStripped = "\ufeff\u200b\u200c\u200d"

// noneWord is the empty list written out. It is a word rather than an empty
// text because a free recipe is something a player should have to say on
// purpose, and because an empty setting is far more likely to be a mistake.
const noneWord = "none"

// defaultWord is the mod's own declared list, with every fallback ladder the
// mod declared, and it keeps meaning that when the mod changes its list in a
// later release.
//
// IT IS THE SETTING'S DEFAULT VALUE, which is what makes it load-bearing rather
// than a convenience: the engine stores every setting's current value in
// mod-settings.dat, untouched defaults included, so "untouched" cannot be a
// comparison against a rendered list. A mod that changed its list would
// otherwise turn every player who never opened the settings screen into a
// player with a hand-typed list, frozen at the old balance and refusing to load
// in a modpack that lacks a rung of a ladder they never saw.
const defaultWord = "default"

// maxListChars is the ceiling on the whole text, in Unicode scalars.
//
// MEASURED: a stored value of 98000 characters reaches the guest intact. Such a
// text is not an ingredient list, and quoting an entry out of it would put tens
// of kilobytes into a load failure, so it is refused whole before parsing.
const maxListChars = 2000

// maxItemAmount is the engine's ceiling on an item ingredient's count.
//
// MEASURED (Factorio 2.0.77, build 84539): amount=65536 refuses the load with
// "Value (65536) outside of range. The data type allows values from 0 to
// 65535"; 65535 loads. The floor is exclusive and separate: amount=0 refuses
// with "Item ingredient can't have count of 0".
const maxItemAmount int64 = 65535

// maxFluidAmount is the engine's ceiling on a fluid ingredient's amount.
//
// MEASURED: 1e301 loads and dumps; 1e302 ABORTS the engine with
// "FixedPointNumber.hpp:31: double value not in range for fixed point number:
// inf" and the crash handler. A crash is not a refusal a player can read, so
// this half refuses first and names the ceiling.
const maxFluidAmount = 1e301

// categoryTakesItemsOnly is the engine's fluid rule, written ONCE for the two
// places that ask it: the declared ingredients of a plan (validateIngredients)
// and a list the player typed (parseEntry). The two used to spell the same
// condition apart from each other, which is a rule that can drift by half a
// commit and then refuse a declaration the language accepts.
//
// MEASURED: a fluid ingredient in a recipe with no category refuses the load
// with "Recipe is in 'crafting' category but has a non-item ingredient 'water'
// (fluid)", and the same ingredient loads under advanced-crafting,
// basic-crafting, smelting, crafting-with-fluid and chemistry. An ABSENT
// category IS crafting, which is why the empty string answers true here, and
// the rule keys on the category NAME rather than on hand-craftability: a fluid
// in a mod category added to character.crafting_categories loads.
func categoryTakesItemsOnly(category string) bool {
	return category == "" || category == "crafting"
}

// listKind is which of the two lists is being read. They share every rule but
// three: what a name may resolve to, whether none is a list, and the wording
// of the empty-text refusal.
type listKind uint8

const (
	// listRecipe is a recipe's ingredients: items and fluids, and none for a
	// recipe that is free to craft.
	listRecipe listKind = iota
	// listPacks is a research unit's science packs: tool-type items only, and
	// no none, because whether a research with no packs can be completed was
	// not measured and a headless probe cannot measure it.
	listPacks
)

// listEntry is one resolved ingredient: what the game calls it, which
// namespace it came from, and the amount in the shape that kind takes.
type listEntry struct {
	kind   ingredientKind
	name   string
	amount int64   // kindItem: the whole count the engine takes
	fluid  float64 // kindFluid: any positive amount
}

// ingredientList is a parsed list in the order it was typed. The empty list is
// the word none and renders back to it.
type ingredientList []listEntry

// parsedList is what one setting's text turned out to say, and it has TWO arms
// rather than one, because "the mod's own list" is not a list.
//
// isDefault is the word default: the caller keeps the author's declared
// ingredients with their ladders, which is a different thing from any list this
// parser could hand back, and the difference has to survive as a value the
// caller can test for rather than as a sentinel list somebody has to remember
// the meaning of. entries is everything else, the empty list included.
type parsedList struct {
	isDefault bool
	entries   ingredientList
}

// render is the canonical text for a parse result: the word for the default
// marker, and the list's own rendering otherwise.
func (p parsedList) render() string {
	if p.isDefault {
		return defaultWord
	}
	return renderIngredientList(p.entries)
}

// parseIngredientList reads one setting's text. The second result is the WHOLE
// refusal, ready to raise, or empty on success.
//
// The setting's full name is a parameter rather than something the list knows,
// because every message begins with it: a player reading a refusal in a load
// failure has to be told which of the mod's settings it is about before
// anything else in the sentence helps them.
func parseIngredientList(text string, kind listKind, category string, setting string, w World) (parsedList, string) {
	// NOT TEXT AT ALL, FIRST. fkdata's two halves disagree about a byte string
	// that is not valid UTF-8: the Rust side decodes lossily and hands the
	// guest U+FFFD, the Go side hands the bytes through. Either half would then
	// quote a different entry back at the player, so both refuse the whole text
	// and neither has to be right about what the bytes meant. U+FFFD itself is
	// refused for the same reason: it is what the other half's lossy decode
	// produces, and a text that legitimately contains one is not a list.
	if !utf8.ValidString(text) || strings.Contains(text, string(utf8.RuneError)) {
		return parsedList{}, "fkrecipes: " + setting + " contains characters that are not text; retype the list"
	}
	// Counted in scalars on the RAW text, before anything is stripped: the
	// ceiling is about what the player stored, and a rule that counted bytes
	// would be a different number in a language with accents in it.
	if utf8.RuneCountInString(text) > maxListChars {
		return parsedList{}, "fkrecipes: " + setting + " is longer than " +
			strconv.Itoa(maxListChars) + " characters; that is not an ingredient list"
	}

	trimmed := strings.Trim(stripInvisible(text), listWhitespace)
	if trimmed == "" {
		// The one message shape with neither a comma nor a colon after the
		// setting name: it is about the setting itself and not about anything
		// inside it. The recipe wording offers none and the pack wording does
		// not, because research refuses none.
		if kind == listPacks {
			return parsedList{}, "fkrecipes: " + setting + ` is empty; write the science packs as "1 automation-science-pack, 1 logistic-science-pack", or the word default for the mod's own list`
		}
		return parsedList{}, "fkrecipes: " + setting + ` is empty; write the ingredients as "2 iron-plate, 3 copper-cable", the word default for the mod's own list, or the word none for a recipe with no ingredients`
	}
	// The default marker, recognised on the whole text before it is cut into
	// entries. It is also recognised as an entry below, which is what lets the
	// tolerated trailing comma apply to it; this arm is the one a reader of the
	// reference looks for, and both halves carry both.
	if trimmed == defaultWord {
		return parsedList{isDefault: true}, ""
	}

	entries := splitEntries(trimmed)

	// EVERY EMPTY ENTRY FIRST, over the whole list, and only then the reserved
	// words. Two passes rather than one, so that a text carrying both problems
	// is answered by the STRUCTURAL one: "none,, 2 iron-plate" is a comma the
	// player did not mean before it is a word in the wrong company. The Rust
	// half reads the two in the same order, which is what makes the choice
	// worth writing down.
	for i, e := range entries {
		if e == "" {
			return parsedList{}, "fkrecipes: " + setting + ", entry " + strconv.Itoa(i+1) +
				" is empty; one comma separates two ingredients"
		}
	}
	// The reserved words are recognised as ENTRIES rather than as the whole
	// text, so the tolerated trailing comma applies to them like anything else,
	// and BEFORE any entry is diagnosed, so "none, 2 iron.plate" is answered by
	// the word standing in company rather than by the stop in the other entry.
	for _, e := range entries {
		switch e {
		case noneWord:
			if len(entries) > 1 {
				return parsedList{}, listProblem(setting, "none stands alone; remove the other entries or the word")
			}
			if kind == listPacks {
				return parsedList{}, listProblem(setting, "research takes at least one science pack")
			}
			return parsedList{entries: ingredientList{}}, ""
		case defaultWord:
			if len(entries) > 1 {
				return parsedList{}, listProblem(setting, "default stands alone; remove the other entries or the word")
			}
			return parsedList{isDefault: true}, ""
		}
	}

	list := make(ingredientList, 0, len(entries))
	for i, e := range entries {
		// The NEXT entry travels with this one, and only for the decimal-comma
		// hint: "0,5 water" is two entries and the first of them is a lone "0",
		// which is a sentence about a comma rather than about a missing name.
		next := ""
		if i+1 < len(entries) {
			next = entries[i+1]
		}
		parsed, problem := parseEntry(e, next, kind, category, w)
		if problem != "" {
			return parsedList{}, entryProblem(setting, i+1, e, problem)
		}
		list = append(list, parsed)
	}

	// Duplicates LAST, and across the whole list, because they are the one
	// problem no single entry can see. MEASURED: iron-plate twice in one
	// recipe refuses the load with "Duplicate item ingredients are not allowed
	// (iron-plate exists 2 or more times)".
	//
	// The pair reported is the first SECOND occurrence with the earliest
	// partner, which is what walking j upward and i below it gives: the player
	// is pointed at the entry they most likely meant to change.
	for j := 1; j < len(list); j++ {
		for i := 0; i < j; i++ {
			if list[i].kind != list[j].kind || list[i].name != list[j].name {
				continue
			}
			return parsedList{}, listProblem(setting, "entries "+strconv.Itoa(i+1)+" and "+
				strconv.Itoa(j+1)+" both name "+list[j].name)
		}
	}
	return parsedList{entries: list}, ""
}

// stripInvisible deletes the zero-width characters that carry no meaning here.
// Written as a scan rather than four Replace passes so the text is walked once
// and the common case, which is a text with none of them, allocates nothing.
func stripInvisible(text string) string {
	if !strings.ContainsAny(text, listStripped) {
		return text
	}
	var b strings.Builder
	b.Grow(len(text))
	for _, r := range text {
		if strings.ContainsRune(listStripped, r) {
			continue
		}
		b.WriteRune(r)
	}
	return b.String()
}

// renderIngredientList is the inverse: the canonical text for a list.
//
// RENDERING THEN PARSING IS AN IDENTITY, and both suites hold it as a property
// over generated lists. That is what lets a declared list be written into a
// setting's description and read back, and it is what makes a log line saying
// what a recipe is now made of a text the player can paste back into the field.
func renderIngredientList(list ingredientList) string {
	if len(list) == 0 {
		return noneWord
	}
	var b strings.Builder
	for i, e := range list {
		if i > 0 {
			b.WriteString(", ")
		}
		if e.kind == kindFluid {
			// A fluid ALWAYS carries its tag. Its name is not in the item
			// namespace, and an untagged name is read as an item first, so a
			// fluid that shares a name with an item would parse back as the
			// item.
			b.WriteString(formatListAmount(e.fluid))
			b.WriteString(" [fluid=" + e.name + "]")
			continue
		}
		b.WriteString(strconv.FormatInt(e.amount, 10))
		b.WriteString(" ")
		if nameNeedsTag(e.name) {
			b.WriteString("[item=" + e.name + "]")
			continue
		}
		b.WriteString(e.name)
	}
	return b.String()
}

// formatListAmount is the ONE way a fluid amount reaches a player's eyes, and
// it is NEITHER language's shortest-round-trip printer.
//
// Go's printer and Rust's Display break an exact tie in the last digit
// differently (MEASURED by this commit's code review and recorded in
// agents/customizer-design.md: 129 divergences in 204105 doubles, the smallest
// at 1.0000076293945312), and a rendering the two halves
// disagree about is a corpus case that cannot be pinned at all. So the digits
// are chosen by a rule that asks neither printer: for p from 0 to 16 the value
// is formatted in scientific form with p digits after the leading one, which is
// correctly rounded at a FIXED precision and therefore the same answer in both
// languages, and the three candidates around those digits are each asked
// whether they read back as exactly v. The first p with a qualifying candidate
// wins, and among the qualifying candidates the first whose last digit is EVEN
// is the answer: that is the tie break neither platform gets to make.
//
// FIXED NOTATION, never exponent form, because 1e21 is not something the parser
// accepts and a rendering the parser refuses would break the identity the
// settings layer rests on.
//
// THE PRECONDITION IS THAT v IS FINITE AND POSITIVE, and it is a precondition
// rather than a case: no rendering of an infinity, a NaN, a zero or a negative
// is specified, so none of the four may be reachable. The typed path refuses
// all four before
// it builds an entry (parseEntry: an amount at or below zero, then a fluid
// amount that is not finite or is above the ceiling), and the declared path
// refuses the same before a plan is validated (validateIngredients). An
// infinity or a NaN arriving here would PANIC rather than render: FormatFloat
// writes them as "+Inf" and "NaN" (measured), which carry no "e" for
// splitScientific to cut at. That is the right failure for a precondition
// nothing may break, and it is why no arm here pretends to have an answer.
func formatListAmount(v float64) string {
	for p := 0; ; p++ {
		digits, exp, ok := listAmountDigits(v, p)
		// p = 16 is 17 significant digits, which read back as every double
		// there is, so the loop ends there at the latest.
		if ok || p == 16 {
			return layOutAmount(digits, exp)
		}
	}
}

// listAmountDigits is one turn of that rule: the correctly rounded mantissa
// digits of v at p digits after the leading one, the decimal exponent, and the
// candidate chosen among the three.
//
// THE CANDIDATES NEVER BORROW OR CARRY, which is what keeps them three strings
// of the same length and the exponent a fixed number: the last digit moves by
// one, and the arm is skipped when it cannot (a 0 cannot go down, a 9 cannot go
// up). ok is false when none of the three reads back as v, and the digits
// handed back are then the rounded ones, so the caller's last turn has an
// answer without asking twice.
func listAmountDigits(v float64, p int) (string, int, bool) {
	digits, exp := splitScientific(strconv.FormatFloat(v, 'e', p, 64))
	last := digits[len(digits)-1]
	candidates := make([]string, 0, 3)
	if last != '0' {
		candidates = append(candidates, withLastDigit(digits, last-1))
	}
	candidates = append(candidates, digits)
	if last != '9' {
		candidates = append(candidates, withLastDigit(digits, last+1))
	}
	first, even := "", ""
	rounded := false
	for _, c := range candidates {
		if !readsBackAs(c, exp, v) {
			continue
		}
		if first == "" {
			first = c
		}
		if even == "" && (c[len(c)-1]-'0')%2 == 0 {
			even = c
		}
		if c == digits {
			rounded = true
		}
	}
	switch {
	case even != "":
		return even, exp, true
	case rounded:
		return digits, exp, true
	case first != "":
		return first, exp, true
	}
	return digits, exp, false
}

// splitScientific takes Go's own scientific form apart into the mantissa digits
// with no dot and the decimal exponent. The form is the one FormatFloat writes
// for a positive number, which is the only kind that reaches here.
func splitScientific(s string) (string, int) {
	at := strings.IndexByte(s, 'e')
	digits := strings.Replace(s[:at], ".", "", 1)
	exp, _ := strconv.Atoi(s[at+1:])
	return digits, exp
}

// withLastDigit is one candidate: the same digits with a different last one.
func withLastDigit(digits string, b byte) string {
	return digits[:len(digits)-1] + string(b)
}

// readsBackAs is the qualification: the candidate written in scientific form
// parses back to exactly v. An exponent past what a double holds arrives as an
// error and is not a qualification, so a candidate that overflows is skipped
// rather than counted as an infinity.
func readsBackAs(digits string, exp int, v float64) bool {
	sci := digits[:1]
	if len(digits) > 1 {
		sci += "." + digits[1:]
	}
	back, err := strconv.ParseFloat(sci+"e"+strconv.Itoa(exp), 64)
	return err == nil && back == v
}

// layOutAmount writes the chosen digits in fixed notation from the exponent,
// with no trailing zero in a fraction and no dot when the fraction is empty.
func layOutAmount(digits string, exp int) string {
	p := len(digits) - 1
	switch {
	case exp >= p:
		// Every digit is in the integer part, and the exponent asks for more.
		return digits + strings.Repeat("0", exp-p)
	case exp >= 0:
		return withFraction(digits[:exp+1], digits[exp+1:])
	default:
		return withFraction("0", strings.Repeat("0", -exp-1)+digits)
	}
}

func withFraction(whole, fraction string) string {
	fraction = strings.TrimRight(fraction, "0")
	if fraction == "" {
		return whole
	}
	return whole + "." + fraction
}

// nameNeedsTag reports whether a name has to be written in its [item=...] tag
// to survive a round trip.
//
// MEASURED: item("42"), item("x") and item("none") all load, and base 2.0.77
// itself ships loader-1x1 and 1x2-remnants, so names that look like amounts,
// like signs, like the two reserved words, and names that merely CONTAIN an x
// between digits all really exist.
//
// THE LEXER IS THE JUDGE, not a list of shapes kept in step by hand: a name
// needs its tag exactly when reading it back as a bare piece would produce
// anything other than that one name. That is what keeps loader-1x1 plain while
// 2x4 and X2 take a tag, without either answer being written down twice.
func nameNeedsTag(name string) bool {
	if name == noneWord || name == defaultWord {
		return true
	}
	var probe []lexToken
	if problem := classifyPiece(name, &probe); problem != "" {
		return true
	}
	if len(probe) != 1 || probe[0].kind != tokenName {
		return true
	}
	// Untagged and unchanged: a name that lexed into one name token which is
	// not the text itself is not a name that can be written bare.
	return probe[0].tagged || probe[0].name != name
}

// entryProblem and listProblem are the two message shapes. An entry's problem
// quotes the entry as typed and numbers it; a list's problem names the setting
// and nothing else, because it is about the whole text.
func entryProblem(setting string, n int, entry, problem string) string {
	return "fkrecipes: " + setting + ", entry " + strconv.Itoa(n) + ` ("` + entry + `"): ` + problem
}

func listProblem(setting, problem string) string {
	return "fkrecipes: " + setting + ": " + problem
}

// splitEntries cuts the text at every comma OUTSIDE a rich-text tag, and trims
// what each cut leaves.
//
// A "[" opens a span that ends at the next "]" or at the end of the text, so
// the comma in [item=iron-plate,quality=rare] does not split the entry that
// carries it: the quality refusal has to see the whole tag to quote the plain
// form back at the player.
//
// ONE TRAILING COMMA IS TOLERATED, because a list written one ingredient per
// line ends with one about half the time and the player meant nothing by it.
// A second one is not: two commas in a row is an entry the player thinks they
// wrote and did not, and it is diagnosed as an empty entry by position.
//
// THE ORDER OF THOSE TWO RULES IS ITSELF A DECISION, and both languages take
// it the same way: the cut comes FIRST and the empty last entry is dropped
// after it. A trailing comma stripped from the whole text beforehand would
// reach inside an unclosed tag, and "[item=iron-plate," would be quoted back
// at the player without the comma they typed.
func splitEntries(text string) []string {
	out := make([]string, 0, 4)
	inTag := false
	start := 0
	// Bytes rather than runes, and safely: the three delimiters are ASCII, and
	// every byte of a multi-byte UTF-8 sequence is >= 0x80, so no character
	// can be cut in half here.
	for i := 0; i < len(text); i++ {
		switch text[i] {
		case '[':
			inTag = true
		case ']':
			inTag = false
		case ',':
			if !inTag {
				out = append(out, strings.Trim(text[start:i], listWhitespace))
				start = i + 1
			}
		}
	}
	out = append(out, strings.Trim(text[start:], listWhitespace))
	if len(out) > 1 && out[len(out)-1] == "" {
		out = out[:len(out)-1]
	}
	return out
}

// tokenKind is what one piece of an entry turned out to be. Three kinds cover
// the language: how much, of what, and the optional sign between them.
type tokenKind uint8

const (
	tokenAmount tokenKind = iota
	tokenSign
	tokenName
)

// lexToken is one lexed piece. text is what the player wrote, which is what
// the sign messages quote back; name, tagged and tagKind are the name arm's
// payload.
//
// NOT NAMED token, and not for taste: source_test.go imports go/token to walk
// this package's own string literals, and a package-level token here would
// collide with that file's import name.
type lexToken struct {
	kind    tokenKind
	text    string
	name    string
	tagged  bool
	tagKind ingredientKind
}

// parseEntry reads one entry and answers with its resolved ingredient or with
// the problem, unprefixed: the caller adds the setting and the entry number.
// next is the entry after this one, or empty, and it is read for exactly one
// message: the decimal comma.
//
// THE DIAGNOSIS ORDER IS THE CONTRACT. Both halves say the same thing about a
// malformed entry only if they look for the problems in the same order, so it
// is written out here once:
//
//  1. a character outside the syntax, an invisible one, a thousands separator
//     or a signed or exponent number (the lexer stops at the first piece that
//     carries one)
//  2. no name at all: a name that reads as an amount, a decimal comma, or the
//     bare "has no name"
//  3. two amounts
//  4. two names: the multi-word fold, which is three outcomes
//  5. more than one sign
//  6. a sign with no amount beside it
//  7. a sign that is not between the amount and the name
//  8. the amount's own value: not above zero
//  9. what the name resolves to in this game
//  10. the kind rules: a fluid where the category takes items, a fraction or
//     an out-of-range count on an item, an out-of-range amount on a fluid
//
// Duplicates are step 11 and belong to the list, not the entry.
func parseEntry(entry, next string, kind listKind, category string, w World) (listEntry, string) {
	tokens, problem := lexEntry(entry)
	if problem != "" {
		return listEntry{}, problem
	}

	var amounts, names, signs int
	var amountTok, nameTok, signTok lexToken
	for _, t := range tokens {
		switch t.kind {
		case tokenAmount:
			amounts++
			if amounts == 1 {
				amountTok = t
			}
		case tokenName:
			names++
			if names == 1 {
				nameTok = t
			}
		case tokenSign:
			signs++
			if signs == 1 {
				signTok = t
			}
		}
	}

	if names == 0 {
		return listEntry{}, noNameProblem(entry, next, tokens, kind, w)
	}
	// TWO AMOUNTS BEFORE TWO NAMES, and the order is visible: "2 iron-plate 3
	// copper-cable" is both, and it is a missing comma between two whole
	// ingredients, so the number the player can see twice is the better thing
	// to point at.
	if amounts > 1 {
		return listEntry{}, "has two amounts"
	}
	if names > 1 {
		return listEntry{}, manyNamesProblem(tokens, kind, w)
	}
	if signs > 1 {
		return listEntry{}, `has more than one "` + signTok.text + `"`
	}
	if signs == 1 {
		if amounts == 0 {
			return listEntry{}, `has "` + signTok.text + `" with no amount beside it`
		}
		if !signIsBetween(tokens) {
			return listEntry{}, `"` + signTok.text + `" goes between the amount and the name`
		}
	}

	// A missing amount means 1, which is what a player writing a list of one
	// of each expects and what the reference promises.
	amount := 1.0
	if amounts == 1 {
		// The text is plain digits with at most one dot, so ParseFloat has no
		// syntax error left to report and its RESULT is the whole answer: an
		// overflow arrives as an infinity, which the kind rules below refuse by
		// name. Reading the value rather than the error is what makes the rule
		// one sentence in both languages instead of a pair of error taxonomies.
		v, _ := strconv.ParseFloat(amountTok.text, 64)
		// MEASURED: an item ingredient with count 0 and a fluid with amount 0
		// are both load failures, so zero is refused before anything asks what
		// the name is.
		if v <= 0 {
			return listEntry{}, "the amount must be more than 0"
		}
		amount = v
	}

	resolved, problem := resolveName(nameTok, kind, w)
	if problem != "" {
		return listEntry{}, problem
	}

	if resolved == kindFluid {
		// The engine's own rule, refused here so the sentence names the
		// setting and the entry rather than arriving as a prototype error the
		// player cannot trace back to what they typed.
		if kind == listRecipe && categoryTakesItemsOnly(category) {
			return listEntry{}, nameTok.name + " is a fluid, and a recipe in the crafting category takes items only"
		}
		// MEASURED: above about 1e301 the engine does not refuse, it aborts.
		if !finite(amount) || amount > maxFluidAmount {
			return listEntry{}, "the amount is too large; fluid amounts go up to 1e301"
		}
		return listEntry{kind: kindFluid, name: nameTok.name, fluid: amount}, ""
	}
	// A FRACTION ON AN ITEM IS THE LIBRARY'S OWN RULE, not the engine's:
	// amount=1.5 LOADS and is dumped as 1.5 (measured), and what a third of an
	// iron plate means to a player at an assembler is not something to ship.
	if amount != math.Trunc(amount) {
		return listEntry{}, nameTok.name + " is an item, and items take whole amounts"
	}
	if amount > float64(maxItemAmount) {
		return listEntry{}, nameTok.name + " takes at most " + strconv.FormatInt(maxItemAmount, 10)
	}
	return listEntry{kind: kindItem, name: nameTok.name, amount: int64(amount)}, ""
}

// noNameProblem is the three-way answer for an entry with no name in it.
//
// A NAME THE GAME HAS COMES FIRST, whatever else the entry looks like. Base
// 2.0.77 really does carry items called 42, x, X2 and 2x4 by the engine's own
// charset, and a player who types one of them has written the right name and
// only needs to be told where to put the brackets. Guessing the other way round
// would tell somebody the game has no item called 42 while it is in their
// inventory.
func noNameProblem(entry, next string, tokens []lexToken, kind listKind, w World) string {
	if word, ok := tagWordForName(entry, kind, w); ok {
		return `"` + entry + `" is a name that reads as an amount; write it in its tag, as [` +
			word + "=" + entry + "]"
	}
	// A DECIMAL COMMA, which is how most of the world writes a fraction and how
	// a good part of it writes a thousand. Both leave a bare number as this
	// entry and digits at the start of the next one, and the old message told
	// the player to write the amount before the name, which is exactly what
	// they had done.
	if len(tokens) == 1 && tokens[0].kind == tokenAmount && next != "" && next[0] >= '0' && next[0] <= '9' {
		return `a comma separates two ingredients, not the digits of one number; write a dot for a fraction, as in "0.5 water"`
	}
	return `has no name; write the amount before the name, as in "2 iron-plate"`
}

// tagWordForName answers which tag a name that the game has should be written
// in, or false when the game does not have it. The lookup is the kind's own, so
// a pack list never points a player at an item that is not a science pack.
func tagWordForName(name string, kind listKind, w World) (string, bool) {
	if kind == listPacks {
		if w.ToolExists(name) {
			return "item", true
		}
		return "", false
	}
	if w.ItemExists(name) {
		return "item", true
	}
	if w.FluidExists(name) {
		return "fluid", true
	}
	return "", false
}

// manyNamesProblem is the multi-word fold, and it has THREE outcomes because
// two names in one entry mean three different mistakes.
//
// A display name is the common one: "2 iron plates" is what the screen calls
// the thing, and a mod's own dropdown labels spell display names too, so the
// player has every reason to think that is the vocabulary. Joining the words
// with a dash, folding case and underscores, and dropping one trailing s is
// what turns that into the internal name, and it is offered only when the game
// really has the result. Two names that BOTH exist is a missing comma. Anything
// else is a text that names nothing, and the sentence says where names come
// from rather than only asking for a comma.
func manyNamesProblem(tokens []lexToken, kind listKind, w World) string {
	words := make([]string, 0, len(tokens))
	for _, t := range tokens {
		if t.kind == tokenName {
			// A TAG CONTRIBUTES ITS NAME, not its brackets: "[item=iron] plate"
			// is the same mistake written two ways.
			words = append(words, t.name)
		}
	}
	spaced := strings.Join(words, " ")
	joined := strings.ReplaceAll(strings.ToLower(strings.Join(words, "-")), "_", "-")
	in := lookupItemOrFluid
	if kind == listPacks {
		in = lookupTool
	}
	candidates := []string{joined}
	if strings.HasSuffix(joined, "s") {
		candidates = append(candidates, joined[:len(joined)-1])
	}
	for _, c := range candidates {
		if existsIn(c, in, w) {
			return noSuchName(kind) + ` "` + spaced + `"; did you mean ` + c
		}
	}
	every := true
	for _, word := range words {
		if !existsIn(word, in, w) {
			every = false
			break
		}
	}
	if every {
		return "names two ingredients; a comma separates them"
	}
	return noSuchName(kind) + ` "` + spaced + `"; ` + internalNamesHint(kind)
}

func noSuchName(kind listKind) string {
	if kind == listPacks {
		return "no science pack is named"
	}
	return "no item or fluid is named"
}

func internalNamesHint(kind listKind) string {
	if kind == listPacks {
		return "names are the game's internal names, such as automation-science-pack, and a comma separates two ingredients"
	}
	return "names are the game's internal names, such as iron-plate, and a comma separates two ingredients"
}

// signIsBetween reports whether the one sign sits where a sign may sit: an
// amount on one side and a name on the other, in either order. "2x iron-plate"
// and "iron-plate x2" are what players type from habit; "water 0.5 x" is a
// trailing operator, and it is a typo rather than a dialect.
func signIsBetween(tokens []lexToken) bool {
	if len(tokens) != 3 || tokens[1].kind != tokenSign {
		return false
	}
	if tokens[0].kind == tokenAmount && tokens[2].kind == tokenName {
		return true
	}
	return tokens[0].kind == tokenName && tokens[2].kind == tokenAmount
}

// lexEntry splits an entry into pieces and classifies each of them, left to
// right, stopping at the FIRST piece that carries a problem.
//
// Two splits, in this order: on whitespace, and then on "*" and the
// multiplication sign, which are never part of a name (MEASURED: item("a*b")
// refuses with "Invalid prototype name. Only characters A-Z a-z 0-9 _- are
// allowed") and so are always signs. That is what makes "iron-plate*2" one
// entry of three tokens rather than a name with punctuation in it.
func lexEntry(entry string) ([]lexToken, string) {
	tokens := make([]lexToken, 0, 3)
	for _, word := range strings.FieldsFunc(entry, isListSpace) {
		start := 0
		for i, r := range word {
			if r != '*' && r != '×' {
				continue
			}
			if i > start {
				if problem := classifyPiece(word[start:i], &tokens); problem != "" {
					return nil, problem
				}
			}
			tokens = append(tokens, lexToken{kind: tokenSign, text: string(r)})
			start = i + utf8.RuneLen(r)
		}
		if start < len(word) {
			if problem := classifyPiece(word[start:], &tokens); problem != "" {
				return nil, problem
			}
		}
	}
	return tokens, ""
}

// isListSpace is listWhitespace as a predicate, read out of the constant
// rather than spelled a second time: two copies of one set is one of them
// going stale.
func isListSpace(r rune) bool { return strings.ContainsRune(listWhitespace, r) }

// tagShape is the answer to every bracketed thing that is not one of the two
// tags, including one that is never closed.
const tagShape = "a tag is [item=name] or [fluid=name]"

// classifyPiece turns one piece into tokens, or answers with its problem.
//
// THE ORDER OF THE ARMS IS LOAD-BEARING, and each one is here for a name the
// engine's charset really allows:
//
//   - the thousands shape before the amount shapes, or "1.000 iron-plate"
//     would silently become one iron plate for the large part of the world
//     that writes a thousand that way
//   - the amount shapes are WHOLE-PIECE ONLY, which is what keeps loader-1x1
//     and 1x2-remnants (both in base 2.0.77) names rather than arithmetic
//   - the signed and exponent shapes before the plain-name arm, because "-2"
//     and "1e3" are both legal prototype names by the charset and a player
//     typing either of them meant a number
func classifyPiece(piece string, tokens *[]lexToken) string {
	if strings.HasPrefix(piece, "[") {
		return classifyTag(piece, tokens)
	}
	if isThousandsShape(piece) {
		return `"` + piece + `" is not an amount here; a dot marks a fraction, and a thousand is written 1000`
	}
	if isNumber(piece) {
		*tokens = append(*tokens, lexToken{kind: tokenAmount, text: piece})
		return ""
	}
	if n := len(piece); n > 1 {
		if isSignLetter(piece[n-1]) && isNumber(piece[:n-1]) {
			*tokens = append(*tokens,
				lexToken{kind: tokenAmount, text: piece[:n-1]},
				lexToken{kind: tokenSign, text: piece[n-1:]})
			return ""
		}
		if isSignLetter(piece[0]) && isNumber(piece[1:]) {
			*tokens = append(*tokens,
				lexToken{kind: tokenSign, text: piece[:1]},
				lexToken{kind: tokenAmount, text: piece[1:]})
			return ""
		}
		// An x with a whole number on each side: two amounts, which the entry
		// diagnosis then reports as such, or as a name the game has when the
		// whole piece turns out to be one.
		for i := 1; i < n-1; i++ {
			if !isSignLetter(piece[i]) || !isNumber(piece[:i]) || !isNumber(piece[i+1:]) {
				continue
			}
			*tokens = append(*tokens,
				lexToken{kind: tokenAmount, text: piece[:i]},
				lexToken{kind: tokenSign, text: piece[i : i+1]},
				lexToken{kind: tokenAmount, text: piece[i+1:]})
			return ""
		}
	}
	if piece == "x" || piece == "X" {
		*tokens = append(*tokens, lexToken{kind: tokenSign, text: piece})
		return ""
	}
	if isSignedNumber(piece) || isExponentNumber(piece) {
		return `"` + piece + `" is not an amount; amounts are plain digits such as 2 or 0.5`
	}
	if isPlainName(piece) {
		*tokens = append(*tokens, lexToken{kind: tokenName, text: piece, name: piece})
		return ""
	}
	return strangeCharacterProblem(piece)
}

// classifyTag reads a rich-text tag, which is what the game's own rich text
// produces and the only way to name an item called 42.
//
// TEXT GLUED AFTER THE CLOSING BRACKET KEEPS GOING, so "[item=iron-plate]2" is
// a name and an amount: a player pasting a tag next to a number left no space
// and meant exactly what it looks like.
func classifyTag(piece string, tokens *[]lexToken) string {
	end := strings.IndexByte(piece, ']')
	if end < 0 {
		return tagShape
	}
	name, kind, quality, ok := readTag(piece[1:end])
	if !ok {
		return tagShape
	}
	// A quality parameter is what the game's own rich text produces for a
	// quality item. Recipes and research costs carry no quality, so the plain
	// form is quoted back rather than the parameter silently dropped.
	if quality {
		return "ingredients carry no quality; write [" + tagWord(kind) + "=" + name + "]"
	}
	*tokens = append(*tokens, lexToken{
		kind:    tokenName,
		text:    piece[:end+1],
		name:    name,
		tagged:  true,
		tagKind: kind,
	})
	if rest := piece[end+1:]; rest != "" {
		return classifyPiece(rest, tokens)
	}
	return ""
}

// readTag reads a tag's inside. ok is false for every shape that is not
// item= or fluid= followed by a name and optionally a quality parameter.
func readTag(inner string) (string, ingredientKind, bool, bool) {
	head := inner
	quality := false
	if at := strings.IndexByte(inner, ','); at >= 0 {
		head = inner[:at]
		if !strings.HasPrefix(inner[at+1:], "quality=") {
			return "", kindItem, false, false
		}
		quality = true
	}
	if name, ok := strings.CutPrefix(head, "item="); ok && isPlainName(name) {
		return name, kindItem, quality, true
	}
	if name, ok := strings.CutPrefix(head, "fluid="); ok && isPlainName(name) {
		return name, kindFluid, quality, true
	}
	return "", kindItem, false, false
}

func tagWord(kind ingredientKind) string {
	if kind == kindFluid {
		return "fluid"
	}
	return "item"
}

// lookup is the namespace a name is asked about. It is what keeps a suggestion
// honest: the folded name is offered only where the original was looked for,
// so a pack list never suggests a name that is an item and not a science pack.
type lookup uint8

const (
	lookupItemOrFluid lookup = iota
	lookupItem
	lookupFluid
	lookupTool
)

// resolveName asks the game what the name is, and answers with the kind it
// resolved to or with the refusal.
//
// AN ITEM WINS A TIE. An untagged name is asked of the items first and of the
// fluids only then, so a name that is both means the item; a player who wants
// the fluid writes its tag. That rule is in the reference because a player has
// to be able to predict it.
func resolveName(t lexToken, kind listKind, w World) (ingredientKind, string) {
	name := t.name
	if kind == listPacks {
		// A FLUID TAG IN A PACK LIST IS A FLUID QUESTION AND NOTHING ELSE. The
		// player named a namespace; answering out of another one would be the
		// guess this language does not make, so an existing fluid gets the
		// fluid sentence and everything else is "no fluid is named", even for a
		// name the game has as a science pack.
		if t.tagged && t.tagKind == kindFluid {
			if w.FluidExists(name) {
				return kindItem, name + " is a fluid, and research takes science packs only"
			}
			return kindItem, "no fluid is named " + name + suggestion(name, lookupFluid, w)
		}
		// THE TOOL QUESTION FIRST, then the two near misses in the order the
		// engine would hit them. The engine's own refusal here is "Invalid
		// research unit (iron-plate). Research unit(s) can only be tool type
		// items at the moment", which names neither the setting nor the entry.
		// These do both.
		if w.ToolExists(name) {
			return kindItem, ""
		}
		if w.ItemExists(name) {
			return kindItem, name + " is an item, not a science pack"
		}
		if w.FluidExists(name) {
			return kindItem, name + " is a fluid, and research takes science packs only"
		}
		return kindItem, "no science pack is named " + name + suggestion(name, lookupTool, w)
	}
	if t.tagged {
		if t.tagKind == kindFluid {
			if w.FluidExists(name) {
				return kindFluid, ""
			}
			return kindItem, "no fluid is named " + name + suggestion(name, lookupFluid, w)
		}
		if w.ItemExists(name) {
			return kindItem, ""
		}
		return kindItem, "no item is named " + name + suggestion(name, lookupItem, w)
	}
	if w.ItemExists(name) {
		return kindItem, ""
	}
	if w.FluidExists(name) {
		return kindFluid, ""
	}
	return kindItem, "no item or fluid is named " + name + suggestion(name, lookupItemOrFluid, w)
}

// suggestion is the one guess this language makes, and it guesses at the
// PLAYER's text rather than at the game's: a name that is the typed one
// lowercased with underscores turned into dashes is what a player typing
// "Iron_Plate" meant, and it is offered only when the game really has it.
// Everything else is a refusal with no suggestion at all.
func suggestion(name string, in lookup, w World) string {
	folded := strings.ReplaceAll(strings.ToLower(name), "_", "-")
	if folded == name || !existsIn(folded, in, w) {
		return ""
	}
	return "; did you mean " + folded
}

func existsIn(name string, in lookup, w World) bool {
	switch in {
	case lookupItem:
		return w.ItemExists(name)
	case lookupFluid:
		return w.FluidExists(name)
	case lookupTool:
		return w.ToolExists(name)
	}
	return w.ItemExists(name) || w.FluidExists(name)
}

// strangeCharacterProblem is the answer for a piece that is none of the
// language's shapes: the FIRST character outside the engine's prototype-name
// charset, quoted, or named by its code point when quoting it would show the
// player nothing.
//
// MEASURED: a no-break space, a zero-width space, a soft hyphen, a BOM and an
// ESC all survive mod-settings.dat and reach the guest. A message quoting one
// of those between two quotation marks shows a blank or nothing at all, which
// is worse than useless in a load failure, so the invisible set is named by
// code point instead and the advice is to retype rather than paste.
func strangeCharacterProblem(piece string) string {
	r, ok := firstStrangeRune(piece)
	if !ok {
		// Unreachable: a piece whose every rune is in the charset is a plain
		// name and was classified as one. Quoting the piece is the honest
		// answer if that ever stops being true.
		return `"` + piece + `" has no place here; ` + charsetHint
	}
	if isInvisibleRune(r) {
		return "an invisible character (U+" + codePointHex(r) + ") has no place here; retype the entry rather than pasting it"
	}
	return `"` + string(r) + `" has no place here; ` + charsetHint
}

// charsetHint says what a name and an amount may be made of. It replaces an
// older sentence that repeated "write the amount before the name", which told a
// player who had typed "iron-plate:2" to do the thing they had just done.
const charsetHint = `names use the letters a to z, digits, - and _, and an amount is plain digits, as in "2 iron-plate"`

// firstStrangeRune is the first character outside the engine's own
// prototype-name charset. A WHOLE UNICODE SCALAR, so a pasted accented letter
// or an emoji is one character rather than the first byte of one.
func firstStrangeRune(piece string) (rune, bool) {
	for _, r := range piece {
		if !isNameRune(r) {
			return r, true
		}
	}
	return 0, false
}

// codePointHex is a code point in the U+XXXX form, uppercase, padded to at
// least four digits, which is how the standard writes them and how a player can
// look one up.
func codePointHex(r rune) string {
	hex := strings.ToUpper(strconv.FormatInt(int64(r), 16))
	for len(hex) < 4 {
		hex = "0" + hex
	}
	return hex
}

// isInvisibleRune is the set named by code point rather than quoted.
//
// The C0 and C1 controls, the soft hyphen, the Arabic letter mark, the Mongolian
// vowel separator, the general-punctuation spaces and bidirectional marks, the
// line and paragraph separators, the narrow spaces, the mathematical and
// deprecated-format block, the ideographic space, the byte-order mark and the
// interlinear annotation marks. Some of these are also in listWhitespace or in
// listStripped and are handled before a piece is ever classified; they are
// listed here anyway so this predicate answers the question it is named for
// rather than the question the callers happen to ask today.
func isInvisibleRune(r rune) bool {
	switch {
	case r >= 0 && r <= 0x1f:
		return true
	case r >= 0x7f && r <= 0x9f:
		return true
	case r == 0x00ad, r == 0x061c, r == 0x180e:
		return true
	case r >= 0x2000 && r <= 0x200f:
		return true
	case r >= 0x2028 && r <= 0x202f:
		return true
	case r >= 0x205f && r <= 0x206f:
		return true
	case r == 0x3000, r == 0xfeff:
		return true
	case r >= 0xfff9 && r <= 0xfffb:
		return true
	}
	return false
}

// isNumber reports the amount shape: one or more digits, optionally a dot and
// one or more digits. No sign, no exponent, no bare leading dot.
func isNumber(s string) bool {
	dot := strings.IndexByte(s, '.')
	if dot < 0 {
		return isDigits(s)
	}
	return isDigits(s[:dot]) && isDigits(s[dot+1:])
}

// isThousandsShape is a NONZERO integer part, a dot and exactly three zeros:
// the way a large part of the world writes a thousand, and a number this
// language would otherwise read as 1.
//
// THE INTEGER PART HAS TO CARRY A NONZERO DIGIT, or 0.000 would be told that a
// thousand is written 1000, which is advice about a number nobody typed. It is
// a plain amount of zero and gets the sentence about zero instead.
func isThousandsShape(s string) bool {
	dot := strings.IndexByte(s, '.')
	if dot < 0 || s[dot+1:] != "000" || !isDigits(s[:dot]) {
		return false
	}
	for i := 0; i < dot; i++ {
		if s[i] != '0' {
			return true
		}
	}
	return false
}

// isSignedNumber is a plus or minus in front of an amount, in EITHER of the two
// shapes that are numbers here: a plain one and an exponent one. It is a legal
// prototype name by the engine's charset for the minus form, which is why it is
// classified rather than left to the plain-name arm.
//
// -1e3 IS A NUMBER WITH A SIGN, not a name: a player who typed it meant minus a
// thousand, and telling them the game has no item called -1e3 answers a
// question they did not ask. The tag form still reaches an item of that name.
func isSignedNumber(s string) bool {
	if len(s) < 2 || (s[0] != '-' && s[0] != '+') {
		return false
	}
	return isNumber(s[1:]) || isExponentNumber(s[1:])
}

// isExponentNumber is an amount in exponent form. The engine's charset allows
// it as a name, and a mod could ship an item called 1e3, but a player who typed
// it into an amount field meant a thousand and is told what an amount looks
// like. The tag form reaches the item.
func isExponentNumber(s string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] != 'e' && s[i] != 'E' {
			continue
		}
		if !isNumber(s[:i]) {
			return false
		}
		rest := s[i+1:]
		if len(rest) > 1 && (rest[0] == '-' || rest[0] == '+') {
			rest = rest[1:]
		}
		return isDigits(rest)
	}
	return false
}

func isDigits(s string) bool {
	if s == "" {
		return false
	}
	for i := 0; i < len(s); i++ {
		if s[i] < '0' || s[i] > '9' {
			return false
		}
	}
	return true
}

func isSignLetter(b byte) bool { return b == 'x' || b == 'X' }

// isPlainName reports the engine's prototype-name charset, MEASURED: item names
// containing any of . (space) : , / * [ ] or a non-ASCII letter each refuse the
// load with "Invalid prototype name. Only characters A-Z a-z 0-9 _- are
// allowed". Bytes rather than runes, which is the same test: every byte of a
// multi-byte sequence is >= 0x80 and so fails it.
func isPlainName(s string) bool {
	if s == "" {
		return false
	}
	for i := 0; i < len(s); i++ {
		b := s[i]
		switch {
		case b >= 'a' && b <= 'z', b >= 'A' && b <= 'Z', b >= '0' && b <= '9', b == '-', b == '_':
		default:
			return false
		}
	}
	return true
}

func isNameRune(r rune) bool {
	switch {
	case r >= 'a' && r <= 'z', r >= 'A' && r <= 'Z', r >= '0' && r <= '9', r == '-', r == '_':
		return true
	}
	return false
}
