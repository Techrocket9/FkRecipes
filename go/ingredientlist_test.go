package fkrecipes

import (
	"os"
	"strconv"
	"strings"
	"testing"
	"unicode/utf8"
)

// THE CORPUS IS THE CONTRACT. testdata/ingredient-list/cases.txt holds every
// case as an input and the exact text the language must answer with, and the
// Rust half runs the same file against the same fixture. A message reworded in
// one language and not in the corpus is a red suite in BOTH, which is what
// keeps two implementations of one language from drifting apart a sentence at
// a time.
//
// The fixture World is read out of the file's own header rather than written
// here as well: a second copy of it in each language is a second thing to keep
// in step, and the first case to notice the drift would be one nobody wrote.

const corpusPath = "../testdata/ingredient-list/cases.txt"

// refusalPrefix is what every refusal this language builds begins with. A later
// caller strips it to compose a log line, so the property has to be TOTAL over
// the corpus rather than true of the messages somebody remembered.
const refusalPrefix = "fkrecipes: "

// corpusWorld is the fixture the corpus states: three name lists and nothing
// else. It EMBEDS UnimplementedWorld, which is the shape a consumer's fixture
// should copy: four methods are implemented because the language asks four
// questions, and every other question this interface has now or grows later
// panics naming itself instead of answering a comfortable lie.
type corpusWorld struct {
	UnimplementedWorld
	items  []string
	fluids []string
	tools  []string
}

// The embed is what makes this a World with four methods written. Asserted at
// compile time, because that is the property under test.
var _ World = (*corpusWorld)(nil)

func (w *corpusWorld) ModName() string             { return "mymod" }
func (w *corpusWorld) ItemExists(name string) bool { return corpusHas(w.items, name) }
func (w *corpusWorld) FluidExists(name string) bool {
	return corpusHas(w.fluids, name)
}
func (w *corpusWorld) ToolExists(name string) bool { return corpusHas(w.tools, name) }

func corpusHas(names []string, name string) bool {
	for _, n := range names {
		if n == name {
			return true
		}
	}
	return false
}

// A World method the fixture does not implement PANICS NAMING ITSELF. This is
// the whole promise of the embed: the day this interface grows a method, a
// consumer's fixture keeps compiling and any plan that actually needs the new
// answer says which method to write rather than reporting that the game has no
// items.
func TestUnimplementedWorldNamesTheMissingMethod(t *testing.T) {
	cases := []struct {
		name string
		call func(World)
	}{
		{"TechExists", func(w World) { w.TechExists("automation") }},
		{"EntityExists", func(w World) { w.EntityExists("steel-chest") }},
		{"RecipeExists", func(w World) { w.RecipeExists("iron-gear-wheel") }},
		{"TechNames", func(w World) { w.TechNames() }},
		{"TechPrereqs", func(w World) { w.TechPrereqs("automation") }},
		{"TechUnit", func(w World) { w.TechUnit("automation") }},
		{"TechMaxLevel", func(w World) { w.TechMaxLevel("automation") }},
		{"TechHasResearchTrigger", func(w World) { w.TechHasResearchTrigger("automation") }},
		{"StartupSetting", func(w World) { w.StartupSetting("mymod-parts") }},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			want := "fkrecipes: World." + c.name + " is not implemented by this fixture"
			defer func() {
				r := recover()
				if r == nil {
					t.Fatalf("%s answered; an unimplemented question must not be mistaken for an answer", c.name)
				}
				if got, _ := r.(string); got != want {
					t.Errorf("\n got: %v\nwant: %s", r, want)
				}
			}()
			c.call(&corpusWorld{})
		})
	}
}

// The four the fixture DOES implement are reached, not inherited: a shadowing
// mistake would turn every corpus case into a panic, which is loud, but a
// fixture that answered ModName out of the embed would panic in a plan test
// far from here.
func TestCorpusWorldAnswersTheFourQuestionsItImplements(t *testing.T) {
	w := &corpusWorld{items: []string{"iron-plate"}, fluids: []string{"water"}, tools: []string{"automation-science-pack"}}
	if w.ModName() != "mymod" {
		t.Errorf("ModName is %q", w.ModName())
	}
	if !w.ItemExists("iron-plate") || w.ItemExists("water") {
		t.Error("ItemExists answers out of the wrong list")
	}
	if !w.FluidExists("water") || w.FluidExists("iron-plate") {
		t.Error("FluidExists answers out of the wrong list")
	}
	if !w.ToolExists("automation-science-pack") || w.ToolExists("iron-plate") {
		t.Error("ToolExists answers out of the wrong list")
	}
}

// corpusCase is one in/ok or in/refuse pair, with everything its section said
// about it, so a failure can name the case exactly as the file spells it.
type corpusCase struct {
	line     int
	section  string
	kind     listKind
	category string
	setting  string
	in       string
	want     string
	refuses  bool
}

func TestIngredientListCorpus(t *testing.T) {
	w, cases := readCorpus(t)

	// Anti-vacuity: a reader that silently matched nothing would be a test
	// that passes over an empty set, which reads exactly like a pass. The floor
	// tracks the corpus and is RAISED when it grows, never lowered to fit a
	// reader; the Rust half carries the same number.
	if len(cases) < 242 {
		t.Fatalf("only %d cases were read from %s; the reader is not seeing the file", len(cases), corpusPath)
	}
	oks, refusals := 0, 0
	for _, c := range cases {
		if c.refuses {
			refusals++
			// EVERY REFUSAL BEGINS WITH THE LIBRARY'S OWN PREFIX, over the
			// whole corpus rather than over the cases somebody thought to
			// check. A caller that strips it to build a log line needs the
			// property to be total, and a message written without it would
			// otherwise only show up wherever that caller is exercised.
			if !strings.HasPrefix(c.want, refusalPrefix) {
				t.Errorf("%s:%d %s: the refusal |%s| does not begin with %q",
					corpusPath, c.line, c.section, c.want, refusalPrefix)
			}
			continue
		}
		oks++
	}
	if oks < 20 || refusals < 20 {
		t.Fatalf("read %d accepted and %d refused cases; the reader is losing one of the two kinds", oks, refusals)
	}
	t.Logf("%d cases (%d accepted, %d refused) against a fixture of %d items, %d fluids and %d science packs",
		len(cases), oks, refusals, len(w.items), len(w.fluids), len(w.tools))

	for _, c := range cases {
		got, problem := parseIngredientList(c.in, c.kind, c.category, c.setting, w)
		if c.refuses {
			if problem == "" {
				t.Errorf("%s:%d %s\n  in:     |%s|\n  want:   |%s|\n  got:    accepted, rendering as |%s|",
					corpusPath, c.line, c.section, c.in, c.want, got.render())
				continue
			}
			if problem != c.want {
				t.Errorf("%s:%d %s\n  in:     |%s|\n  want:   |%s|\n  got:    |%s|",
					corpusPath, c.line, c.section, c.in, c.want, problem)
			}
			if !strings.HasPrefix(problem, refusalPrefix) {
				t.Errorf("%s:%d %s: the refusal this half produced does not begin with %q\n  got: |%s|",
					corpusPath, c.line, c.section, refusalPrefix, problem)
			}
			// A REFUSED TEXT COMES BACK AS NOTHING AT ALL, neither a list nor
			// the default marker: a caller that read the result past a refusal
			// would apply the mod's own list to a text the player got wrong.
			if got.isDefault || got.entries != nil {
				t.Errorf("%s:%d %s: a refused text still came back as %+v",
					corpusPath, c.line, c.section, got)
			}
			continue
		}
		if problem != "" {
			t.Errorf("%s:%d %s\n  in:     |%s|\n  want:   |%s|\n  got:    refused with |%s|",
				corpusPath, c.line, c.section, c.in, c.want, problem)
			continue
		}
		// THE DEFAULT MARKER IS A VALUE, not the word round-tripping through a
		// list: the only text that renders as "default" must be the arm the
		// caller tests for, and nothing else may reach it.
		if (c.want == "default") != got.isDefault {
			t.Errorf("%s:%d %s\n  in:     |%s|\n  want default marker: %v\n  got:                %v",
				corpusPath, c.line, c.section, c.in, c.want == "default", got.isDefault)
		}
		if rendered := got.render(); rendered != c.want {
			t.Errorf("%s:%d %s\n  in:     |%s|\n  want:   |%s|\n  got:    |%s|",
				corpusPath, c.line, c.section, c.in, c.want, rendered)
			continue
		}
		// THE CANONICAL FORM IS A FIXED POINT. Every accepted case's own
		// expected text is fed back through, because that text is what a log
		// line hands the player to paste back into the field: a rendering that
		// did not parse back to itself would be advice that refuses.
		back, problem := parseIngredientList(c.want, c.kind, c.category, c.setting, w)
		if problem != "" {
			t.Errorf("%s:%d %s: the canonical text |%s| does not parse: %s",
				corpusPath, c.line, c.section, c.want, problem)
			continue
		}
		if again := back.render(); again != c.want {
			t.Errorf("%s:%d %s: the canonical text is not a fixed point\n  want: |%s|\n  got:  |%s|",
				corpusPath, c.line, c.section, c.want, again)
		}
	}
}

// readCorpus reads the file into the fixture World its header states and the
// cases its sections carry. Every shape it does not understand is a failure
// rather than a skipped line: a case the reader silently dropped is a case
// that passes.
func readCorpus(t *testing.T) (*corpusWorld, []corpusCase) {
	t.Helper()
	raw, err := os.ReadFile(corpusPath)
	if err != nil {
		t.Fatalf("the corpus is the contract and it is not readable: %v", err)
	}
	lines := strings.Split(string(raw), "\n")

	w := &corpusWorld{}
	var cases []corpusCase
	var section string
	var kind listKind
	var category, setting string
	var pendingIn string
	pendingLine := 0
	havePending := false
	// Which of the header's three name lists the reader is inside, so a list
	// that wraps onto a second line keeps going.
	current := ""

	for n, line := range lines {
		switch {
		case strings.HasPrefix(line, "#"):
			body := strings.TrimSpace(strings.TrimPrefix(line, "#"))
			label, names, ok := headerList(body)
			if ok {
				current = label
				w.add(label, names)
				continue
			}
			if current != "" && body != "" && allPlainNames(body) {
				w.add(current, strings.Fields(body))
				continue
			}
			current = ""
		case strings.TrimSpace(line) == "":
			current = ""
		case strings.HasPrefix(line, "["):
			current = ""
			if havePending {
				t.Fatalf("%s:%d: an in: line with no ok: or refuse: after it", corpusPath, pendingLine)
			}
			head := strings.TrimSuffix(strings.TrimPrefix(line, "["), "]")
			fields := strings.Fields(head)
			section = line
			switch {
			case len(fields) == 3 && fields[0] == "recipe":
				kind, category, setting = listRecipe, fields[1], fields[2]
			case len(fields) == 2 && fields[0] == "packs":
				kind, category, setting = listPacks, "", fields[1]
			default:
				t.Fatalf("%s:%d: a section line the reader does not understand: %s", corpusPath, n+1, line)
			}
		case strings.HasPrefix(line, "in:"):
			if havePending {
				t.Fatalf("%s:%d: two in: lines in a row", corpusPath, pendingLine)
			}
			text, ok := between(line)
			if !ok {
				t.Fatalf("%s:%d: an in: line with no |text|: %s", corpusPath, n+1, line)
			}
			pendingIn, pendingLine, havePending = unescapeCase(t, n+1, text), n+1, true
		case strings.HasPrefix(line, "ok:"), strings.HasPrefix(line, "refuse:"):
			if !havePending {
				t.Fatalf("%s:%d: an answer with no in: line before it: %s", corpusPath, n+1, line)
			}
			if section == "" {
				t.Fatalf("%s:%d: a case before any section line", corpusPath, n+1)
			}
			text, ok := between(line)
			if !ok {
				t.Fatalf("%s:%d: an answer with no |text|: %s", corpusPath, n+1, line)
			}
			cases = append(cases, corpusCase{
				line:     pendingLine,
				section:  section,
				kind:     kind,
				category: category,
				setting:  setting,
				in:       pendingIn,
				want:     unescapeCase(t, n+1, text),
				refuses:  strings.HasPrefix(line, "refuse:"),
			})
			havePending = false
		default:
			t.Fatalf("%s:%d: a line the reader does not understand: %s", corpusPath, n+1, line)
		}
	}
	if havePending {
		t.Fatalf("%s:%d: an in: line with no ok: or refuse: after it", corpusPath, pendingLine)
	}
	if len(w.items) == 0 || len(w.fluids) == 0 || len(w.tools) == 0 {
		t.Fatalf("the header's fixture came out as %d items, %d fluids and %d tools; the reader is not seeing it",
			len(w.items), len(w.fluids), len(w.tools))
	}
	return w, cases
}

func (w *corpusWorld) add(label string, names []string) {
	switch label {
	case "items":
		w.items = append(w.items, names...)
	case "fluids":
		w.fluids = append(w.fluids, names...)
	case "tools":
		w.tools = append(w.tools, names...)
	}
}

// headerList reads one of the header's fixture lines: a label, a parenthesised
// method name, a colon and then the names.
func headerList(body string) (string, []string, bool) {
	for _, label := range []string{"items", "fluids", "tools"} {
		if !strings.HasPrefix(body, label) {
			continue
		}
		at := strings.IndexByte(body, ':')
		if at < 0 {
			return "", nil, false
		}
		names := strings.Fields(body[at+1:])
		if len(names) == 0 || !allPlainNames(body[at+1:]) {
			return "", nil, false
		}
		return label, names, true
	}
	return "", nil, false
}

// allPlainNames is what makes a header line a CONTINUATION rather than prose:
// a line of nothing but prototype names. Prose in this header carries a comma,
// a colon, a bracket or a full stop, none of which are in the charset.
func allPlainNames(body string) bool {
	fields := strings.Fields(body)
	if len(fields) == 0 {
		return false
	}
	for _, f := range fields {
		if !isPlainName(f) {
			return false
		}
	}
	return true
}

// between takes the text between the FIRST and the LAST bar, so a case can
// carry leading and trailing spaces, bars of its own, and nothing at all.
func between(line string) (string, bool) {
	first := strings.IndexByte(line, '|')
	last := strings.LastIndexByte(line, '|')
	if first < 0 || last <= first {
		return "", false
	}
	return line[first+1 : last], true
}

// unescapeCase applies the corpus file's escapes to one case's text, and it is
// applied to in:, ok: and refuse: lines alike, because a refusal QUOTES the
// entry the player typed and so carries whatever the input carried.
//
// \xNN IS ONE RAW BYTE and not a code point, which is the whole reason the
// escape exists: an input that is not valid UTF-8 is a text the parser must
// refuse, and a file of source text cannot hold one any other way. The result
// is therefore built as bytes and only then made a string, so this reader can
// produce a Go string that utf8.ValidString rejects. \uNNNN and \UNNNNNNNN are
// code points and are encoded; \t \r \n and \\ are the ordinary four.
func unescapeCase(t *testing.T, line int, s string) string {
	t.Helper()
	out := make([]byte, 0, len(s))
	for i := 0; i < len(s); {
		if s[i] != '\\' {
			out = append(out, s[i])
			i++
			continue
		}
		if i+1 >= len(s) {
			t.Fatalf("%s:%d: a backslash with nothing after it", corpusPath, line)
			return ""
		}
		switch s[i+1] {
		case '\\':
			out = append(out, '\\')
			i += 2
		case 't':
			out = append(out, '\t')
			i += 2
		case 'r':
			out = append(out, '\r')
			i += 2
		case 'n':
			out = append(out, '\n')
			i += 2
		case 'x':
			out = append(out, byte(hexDigits(t, line, s, i+2, 2)))
			i += 4
		case 'u', 'U':
			width := 4
			if s[i+1] == 'U' {
				width = 8
			}
			r, ok := caseScalar(hexDigits(t, line, s, i+2, width))
			// A SURROGATE OR A POINT ABOVE U+10FFFF IS A READER ERROR, which
			// the corpus header states, and it is a failure here rather than a
			// substitution because utf8.AppendRune would quietly write U+FFFD
			// for one: the case would then be about a character nobody wrote,
			// and the bytes it was written for would never reach the parser. A
			// case that needs those bytes writes them with \x.
			if !ok {
				t.Fatalf("%s:%d: \\%c%s names a surrogate or a point above U+10FFFF, which is not a scalar value; write the bytes with \\x",
					corpusPath, line, s[i+1], s[i+2:i+2+width])
				return ""
			}
			out = utf8.AppendRune(out, r)
			i += 2 + width
		default:
			t.Fatalf("%s:%d: an escape the reader does not understand: \\%c", corpusPath, line, s[i+1])
			return ""
		}
	}
	return string(out)
}

// caseScalar is what a \u or \U escape may name: a Unicode SCALAR VALUE, which
// is every code point except the surrogates D800..DFFF and everything above
// 10FFFF. ok is false for the rest, and the int64 round trip is what keeps an
// eight-digit escape from wrapping into a rune that looks legal: FFFFFFFF is
// -1 as an int32.
func caseScalar(v int64) (rune, bool) {
	r := rune(v)
	return r, int64(r) == v && utf8.ValidRune(r)
}

func hexDigits(t *testing.T, line int, s string, at, width int) int64 {
	t.Helper()
	if at+width > len(s) {
		t.Fatalf("%s:%d: an escape with fewer than %d hex digits after it", corpusPath, line, width)
		return 0
	}
	v, err := strconv.ParseInt(s[at:at+width], 16, 64)
	if err != nil {
		t.Fatalf("%s:%d: %q is not %d hex digits: %v", corpusPath, line, s[at:at+width], width, err)
		return 0
	}
	return v
}

// THE ESCAPE READER, held up to the light on its own. Every case in the file
// runs through it, so a reader that silently produced the wrong bytes would
// turn a corpus case into a test of something nobody wrote; and the one shape
// the parser most needs, a text that is not valid UTF-8, exists nowhere else.
func TestCorpusEscapesAreRead(t *testing.T) {
	cases := []struct {
		in   string
		want []byte
	}{
		{`plain`, []byte("plain")},
		{`2\x20iron-plate`, []byte("2 iron-plate")},
		{`\xff\xfe`, []byte{0xff, 0xfe}},
		{`a\tb\r\nc`, []byte("a\tb\r\nc")},
		{`2 iron\\plate`, []byte(`2 iron\plate`)},
		{"\u00d7", []byte("\u00d7")},
		{"\uFFFD", []byte("\uFFFD")},
		{`\U0001F642`, []byte("🙂")},
		{`\x1b[31m`, append([]byte{0x1b}, []byte("[31m")...)},
	}
	for _, c := range cases {
		got := unescapeCase(t, 0, c.in)
		if got != string(c.want) {
			t.Errorf("unescapeCase(%q) = %q (% x), want %q (% x)", c.in, got, got, c.want, c.want)
		}
	}
	// THE ESCAPE THE READER REFUSES. utf8.AppendRune writes U+FFFD for a
	// surrogate and for a point above U+10FFFF: a corpus case written with
	// \ud800 would silently become a case about a replacement character, and
	// the unpaired-surrogate bytes it was written for would never be tested at
	// all. The reader fails the test instead, and this is the discrimination it
	// fails on.
	for _, c := range []struct {
		v    int64
		want bool
	}{
		{0x41, true},
		{0x00, true},
		{0xD7FF, true},
		{0xD800, false},
		{0xDFFF, false},
		{0xE000, true},
		{0x10FFFF, true},
		{0x110000, false},
		{0xFFFFFFFF, false},
	} {
		if _, ok := caseScalar(c.v); ok != c.want {
			t.Errorf("caseScalar(%#x) accepted = %v, want %v", c.v, ok, c.want)
		}
	}
	// The point of \xNN: a Go string that is not text.
	if utf8.ValidString(unescapeCase(t, 0, `\xff\xfe`)) {
		t.Error("the byte escape produced valid UTF-8; the not-text rule has no input to refuse")
	}
	// And the corpus really uses it: a reader that were never exercised by the
	// file would be a mechanism with no cases behind it.
	raw, err := os.ReadFile(corpusPath)
	if err != nil {
		t.Fatal(err)
	}
	for _, escape := range []string{`\x`, `\u`, `\U`, `\t`, `\\`} {
		if !strings.Contains(string(raw), escape) {
			t.Errorf("the corpus carries no %s escape; the reader's arm for it is untested by the file", escape)
		}
	}
}

// WHAT THE CORPUS DOES NOT PIN, pinned here so that a change to it is a
// decision rather than a drift. Each of these is a rule the corpus is silent
// about today, and each one is a place where the two language halves could
// disagree without any suite going red: they are listed for the reviewer as
// much as for the compiler.
func TestRulesBeyondTheCorpus(t *testing.T) {
	w := &corpusWorld{
		items:  []string{"iron-plate", "copper-cable", "none", "default", "defaults", "automation-science-pack"},
		fluids: []string{"water"},
		tools:  []string{"automation-science-pack"},
	}
	cases := []struct {
		name    string
		in      string
		kind    listKind
		want    string
		refuses bool
	}{
		{
			// THE PAIR REPORTED when a name appears three times, or when two
			// names each appear twice. The corpus has only lists with a single
			// duplicate pair, where every scan order agrees; these two say
			// which pair a list with several is reported by. The rule is the
			// first SECOND occurrence with the earliest partner.
			name:    "three of one name report the first pair",
			in:      "2 iron-plate, 3 iron-plate, 4 iron-plate",
			want:    "fkrecipes: mymod-parts: entries 1 and 2 both name iron-plate",
			refuses: true,
		},
		{
			name:    "two duplicated names report the earlier one",
			in:      "iron-plate, copper-cable, iron-plate, copper-cable",
			want:    "fkrecipes: mymod-parts: entries 1 and 3 both name iron-plate",
			refuses: true,
		},
		{
			// THE TOLERATED TRAILING COMMA APPLIES TO none TOO. The word is
			// recognised as an ENTRY rather than as the whole text, so a
			// player who ends every line with a comma still gets the empty
			// list rather than the item that happens to be called none.
			name: "none with the tolerated trailing comma",
			in:   "none,",
			want: "none",
		},
		{
			// THE STRUCTURAL PROBLEM WINS. A text carrying both an empty entry
			// and a misplaced none is answered by the comma, because the empty
			// entries are scanned over the whole list before the word is
			// looked for. Both halves read them in that order.
			name:    "an empty entry is reported before the misplaced none",
			in:      "none,, 2 iron-plate",
			want:    "fkrecipes: mymod-parts, entry 2 is empty; one comma separates two ingredients",
			refuses: true,
		},
		{
			// A tagged item IS reachable when the name is also a reserved word:
			// the tag is not the word.
			name: "the item called none, in its tag",
			in:   "[item=none]",
			want: "1 [item=none]",
		},
		{
			name: "the item called default, in its tag",
			in:   "[item=default]",
			want: "1 [item=default]",
		},
		{
			// A FLUID TAG IN A PACK LIST IS A FLUID QUESTION, even over a name
			// the game has as a science pack. The corpus pins the refusal for a
			// name that is a pack; this is the same rule with the fluid absent
			// too, so no arm of it answers out of the tool namespace.
			name:    "a fluid tag over a name the game does not have",
			in:      "[fluid=unobtainium]",
			kind:    listPacks,
			want:    "fkrecipes: mymod-parts, entry 1 (\"[fluid=unobtainium]\"): no fluid is named unobtainium",
			refuses: true,
		},
		{
			// A tagged ITEM in a pack list falls through the same three
			// questions an untagged name does, so a fluid named in an item tag
			// still gets the sentence about research taking packs.
			name:    "an item tag over a fluid, in a pack list",
			in:      "[item=water]",
			kind:    listPacks,
			want:    "fkrecipes: mymod-parts, entry 1 (\"[item=water]\"): water is a fluid, and research takes science packs only",
			refuses: true,
		},
		{
			// THE FIRST RESERVED WORD IN ORDER is the one the sentence names.
			// Both words stand alone, so a text carrying both is wrong twice;
			// reading the entries in order is what makes the answer the same in
			// both halves.
			name:    "none and default together",
			in:      "none, default",
			want:    "fkrecipes: mymod-parts: none stands alone; remove the other entries or the word",
			refuses: true,
		},
		{
			// The multi-word fold's suggestion is offered out of the kind's own
			// lookup, so a recipe entry may fold to a FLUID as well as an item.
			name:    "two words that fold to a fluid",
			in:      "1 Water",
			want:    "fkrecipes: mymod-parts, entry 1 (\"1 Water\"): no item or fluid is named Water; did you mean water",
			refuses: true,
		},
		{
			// A CASED RESERVED WORD IS THE WORD, so it stands alone by the same
			// rule, and the sentence names the word in the one spelling the
			// language has rather than echoing the player's capitals back. The
			// corpus pins the all-capitals spelling; this is the mixed one,
			// which is the shape a fold written as two comparisons would miss.
			name:    "a cased none beside another entry",
			in:      "NoNe, 2 iron-plate",
			want:    "fkrecipes: mymod-parts: none stands alone; remove the other entries or the word",
			refuses: true,
		},
		{
			// THE FOLD IS AN EQUALITY, not a prefix: an item whose name merely
			// begins with a reserved word is a name like any other.
			name: "a name that only begins with a reserved word",
			in:   "2 defaults",
			want: "2 defaults",
		},
		{
			// A HOMOGLYPH IS NOT THE WORD. The last letter here is a Cyrillic
			// capital Te (U+0422), so the entry reads as DEFAULT on screen and
			// is none of the language's words: the fold is over ASCII letters
			// and a name is what its code points are. The character rule
			// answers, and it quotes the character that is not what it looks
			// like.
			name:    "a homoglyph that reads as the word",
			in:      "DEFAUL\u0422",
			want:    "fkrecipes: mymod-parts, entry 1 (\"DEFAUL\u0422\"): \"\u0422\" has no place here; names use the letters a to z, digits, - and _, and an amount is plain digits, as in \"2 iron-plate\"",
			refuses: true,
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got, problem := parseIngredientList(c.in, c.kind, "crafting", "mymod-parts", w)
			if c.refuses {
				if problem != c.want {
					t.Errorf("\n in:   |%s|\n got:  |%s|\nwant:  |%s|", c.in, problem, c.want)
				}
				return
			}
			if problem != "" {
				t.Fatalf("|%s| was refused: %s", c.in, problem)
			}
			if rendered := got.render(); rendered != c.want {
				t.Errorf("\n in:   |%s|\n got:  |%s|\nwant:  |%s|", c.in, rendered, c.want)
			}
		})
	}
}

// THE WHOLE-TEXT RULES, which run before the text is a list at all. The corpus
// pins each of them once; these are the neighbours of each threshold, which a
// corpus of examples cannot carry without becoming a table of off-by-ones.
func TestWholeTextRules(t *testing.T) {
	w := &corpusWorld{
		items:  []string{"iron-plate", "copper-cable"},
		fluids: []string{"water"},
		tools:  []string{"automation-science-pack"},
	}

	// A TEXT THAT IS NOT TEXT. fkdata hands both halves the stored bytes
	// unchanged, and bytes that are not valid UTF-8 are refused whole rather
	// than quoted at the player, so neither half has to be right about what
	// they meant. U+FFFD is not in this list: it is an ordinary character the
	// names rule refuses by quoting it, and the corpus pins that.
	t.Run("not text", func(t *testing.T) {
		want := "fkrecipes: mymod-parts contains characters that are not text; retype the list"
		for _, in := range []string{
			"2 iron-\xff\xfeplate",
			"\xff",
			// The bytes of a lone surrogate, which no encoder should produce
			// and a hand-edited file can still hold.
			"2 \xed\xa0\x80 iron-plate",
		} {
			got, problem := parseIngredientList(in, listRecipe, "crafting", "mymod-parts", w)
			if problem != want {
				t.Errorf("|% x|\n got:  |%s|\nwant:  |%s|", in, problem, want)
			}
			if got.isDefault || got.entries != nil {
				t.Errorf("|% x| came back as %+v", in, got)
			}
		}
		// The other side of the rule: a text with an accent in it is perfectly
		// good UTF-8 and is refused by the CHARACTER rule with the character
		// quoted, not by this one.
		_, problem := parseIngredientList("2 iron-platé", listRecipe, "crafting", "mymod-parts", w)
		if strings.Contains(problem, "not text") {
			t.Errorf("valid UTF-8 was refused as not text: %s", problem)
		}
	})

	// THE LENGTH RULE, at its two neighbours. MEASURED: a stored value of 98000
	// characters reaches the guest, so the ceiling is a real defence and not a
	// theoretical one.
	t.Run("length", func(t *testing.T) {
		want := "fkrecipes: mymod-parts is longer than 2000 characters; that is not an ingredient list"
		// 2000 characters exactly: read, and refused for what is actually wrong
		// with it rather than for its length.
		at := "1 iron-plate" + strings.Repeat(" ", 2000-len("1 iron-plate"))
		if utf8.RuneCountInString(at) != 2000 {
			t.Fatalf("the fixture text is %d characters, not 2000", utf8.RuneCountInString(at))
		}
		got, problem := parseIngredientList(at, listRecipe, "crafting", "mymod-parts", w)
		if problem != "" {
			t.Errorf("a 2000-character text was refused: %s", problem)
		}
		if rendered := got.render(); rendered != "1 iron-plate" {
			t.Errorf("a 2000-character text rendered as |%s|", rendered)
		}
		// 2001, one over.
		over := at + " "
		if _, problem := parseIngredientList(over, listRecipe, "crafting", "mymod-parts", w); problem != want {
			t.Errorf("2001 characters\n got:  |%s|\nwant:  |%s|", problem, want)
		}
		// SCALARS, NOT BYTES. 2000 characters of a three-byte character is 6000
		// bytes and is still 2000 characters; a byte count would refuse this
		// with the wrong sentence, and would refuse a different number of them
		// in each language.
		wide := strings.Repeat("あ", 2000)
		if _, problem := parseIngredientList(wide, listRecipe, "crafting", "mymod-parts", w); problem == want {
			t.Error("2000 wide characters were refused for length; the count is reading bytes")
		}
		if _, problem := parseIngredientList(wide+"あ", listRecipe, "crafting", "mymod-parts", w); problem != want {
			t.Errorf("2001 wide characters\n got:  |%s|\nwant:  |%s|", problem, want)
		}
		// The rule is counted on the text AS STORED, and nothing is removed
		// from it beforehand: a player who pasted 2001 zero-width spaces did
		// not write a list, and they are told so by the length rather than by
		// an entry quoting 2001 code points back at them.
		zeroWidth := strings.Repeat("\u200b", 2001)
		if _, problem := parseIngredientList(zeroWidth, listRecipe, "crafting", "mymod-parts", w); problem != want {
			t.Errorf("2001 invisible characters\n got:  |%s|\nwant:  |%s|", problem, want)
		}
		// THE WORST MESSAGE THE CEILING LETS THROUGH, measured here rather than
		// asserted in a comment: a text of exactly 2000 characters passes the
		// length rule and is quoted back whole, with every invisible character in
		// it written as its token, so the token is what sets the bound. The two
		// numbers below are the ones the comment on maxListChars cites, and this
		// is the command that re-takes them:
		//
		//	cd go && go test -run 'TestWholeTextRules/length' -v
		for _, c := range []struct {
			name  string
			r     rune
			bytes int
		}{
			{"U+E0001", '\U000e0001', 14129},
			{"U+200B", '\u200b', 12128},
		} {
			_, problem := parseIngredientList(strings.Repeat(string(c.r), 2000), listRecipe, "crafting", "mymod-parts", w)
			t.Logf("2000 x %s refuses in %d bytes", c.name, len(problem))
			if len(problem) != c.bytes {
				t.Errorf("2000 x %s refuses in %d bytes, not the %d maxListChars is documented with; re-measure and update BOTH halves' comments",
					c.name, len(problem), c.bytes)
			}
		}
	})

	// The whole-text rules run in ORDER, and the order is visible only where a
	// text breaks two of them at once.
	t.Run("order", func(t *testing.T) {
		long := "2 iron-\xffplate" + strings.Repeat(" ", 2001)
		want := "fkrecipes: mymod-parts contains characters that are not text; retype the list"
		if _, problem := parseIngredientList(long, listRecipe, "crafting", "mymod-parts", w); problem != want {
			t.Errorf("a text that is both too long and not text\n got:  |%s|\nwant:  |%s|", problem, want)
		}
	})

	// THE INVISIBLE SET AND THE WHITESPACE SET, EVERY MEMBER OF BOTH.
	//
	// ONE POLICY: a character the player cannot see is refused by its code
	// point, wherever it sits, and nothing is deleted from the text on their
	// behalf. The whitespace set is the exception and it is asked FIRST, which
	// is why the six members the two sets share (tab, LF, CR, U+2007, U+202F
	// and U+3000) separate words rather than refuse.
	//
	// THE SET IS SPELLED HERE AND NOT READ OUT OF THE PREDICATE, which is the
	// whole difference between a table that guards and a walk that asks its own
	// subject what to ask about. An earlier shape of this test iterated every
	// rune and skipped the ones isInvisibleRune said no to, so a range NARROWED
	// in the predicate simply stopped being tested. PROVED: narrowing
	// 0x115f..0x1160 to 0x115f and 0xfe00..0xfe0f to 0xfe0f drops 15 code
	// points, and the whole Go suite stays green while the two halves disagree.
	//
	// So the table below is an INDEPENDENT spelling of the same set, and it is
	// asked twice about every member: of the PARSER, which is the behaviour a
	// player meets, and of the PREDICATE directly, which is the behaviour the
	// renderer and quotable share. The code point either side of every range is
	// asked both questions too and must answer the other way, which is the half
	// of a boundary a list of members cannot state: a range widened by one would
	// otherwise pass here.
	//
	// The Rust twin carries the same table for the same reason, and the corpus
	// pins one member of every range in both languages.
	t.Run("invisible and whitespace", func(t *testing.T) {
		// Range by range, low to high. Derived rather than recalled; the
		// predicate's own comment says from what and how.
		invisible := [][2]rune{
			{0x0000, 0x001f},
			{0x007f, 0x009f},
			{0x00ad, 0x00ad},
			{0x034f, 0x034f},
			{0x0600, 0x0605},
			{0x061c, 0x061c},
			{0x06dd, 0x06dd},
			{0x070f, 0x070f},
			{0x0890, 0x0891},
			{0x08e2, 0x08e2},
			{0x115f, 0x1160},
			{0x17b4, 0x17b5},
			{0x180b, 0x180f},
			{0x2000, 0x200f},
			{0x2028, 0x202f},
			{0x205f, 0x206f},
			{0x3000, 0x3000},
			{0x3164, 0x3164},
			{0xfe00, 0xfe0f},
			{0xfeff, 0xfeff},
			{0xffa0, 0xffa0},
			{0xfff0, 0xfffb},
			{0x110bd, 0x110bd},
			{0x110cd, 0x110cd},
			{0x13430, 0x1343f},
			{0x1bca0, 0x1bca3},
			{0x1d173, 0x1d17a},
			{0xe0000, 0xe0fff},
		}
		inTable := func(r rune) bool {
			for _, g := range invisible {
				if r >= g[0] && r <= g[1] {
					return true
				}
			}
			return false
		}

		members, refused := 0, 0
		for _, g := range invisible {
			for r := g[0]; r <= g[1]; r++ {
				members++
				if !utf8.ValidRune(r) {
					t.Fatalf("the table covers U+%04X, which is not a scalar value and can be in no text", r)
				}
				if !isInvisibleRune(r) {
					t.Errorf("U+%04X is in the table and isInvisibleRune says it is not in the set", r)
				}
				// The six the whitespace set takes first are members of this
				// set and separate words anyway; the loop below is where they
				// are put to the parser.
				if isListSpace(r) {
					continue
				}
				refused++
				in := "2 iron" + string(r) + "-plate"
				token := "U+" + codePointHex(r)
				want := `fkrecipes: mymod-parts, entry 1 ("2 iron` + token + `-plate"): an invisible character (` +
					token + `) has no place here; retype the entry rather than pasting it`
				got, problem := parseIngredientList(in, listRecipe, "crafting", "mymod-parts", w)
				if problem != want {
					t.Errorf("%s inside a name\n got:  |%s|\nwant:  |%s|", token, problem, want)
				}
				if got.isDefault || got.entries != nil {
					t.Errorf("%s inside a name came back as %+v", token, got)
				}
			}
		}
		// EXACTLY, not a floor: a floor a deleted range still clears is a floor
		// that cannot notice. The tag block alone is 4096 of the 4287, and six of
		// them are whitespace first.
		if members != 4287 || refused != 4281 {
			t.Fatalf("the table walked %d members and put %d of them to the parser; the set is 4287 and 4281, and a table that no longer says so is a set somebody changed on one side",
				members, refused)
		}
		t.Logf("%d invisible characters refused by code point, out of %d in the set", refused, members)

		// THE CODE POINT EITHER SIDE OF EVERY RANGE, quoted as itself and denied
		// by the predicate. A neighbour that is itself a member of another range,
		// or a member of the whitespace set, answers a different question and is
		// stepped past.
		neighbours := 0
		for _, g := range invisible {
			for _, r := range []rune{g[0] - 1, g[1] + 1} {
				if r < 0 || r > 0x10ffff || !utf8.ValidRune(r) || inTable(r) || isListSpace(r) {
					continue
				}
				neighbours++
				if isInvisibleRune(r) {
					t.Errorf("U+%04X sits outside the table and isInvisibleRune says it is in the set", r)
				}
				in := "2 iron" + string(r) + "-plate"
				want := `fkrecipes: mymod-parts, entry 1 ("2 iron` + string(r) + `-plate"): "` + string(r) +
					`" has no place here; ` + charsetHint
				if _, problem := parseIngredientList(in, listRecipe, "crafting", "mymod-parts", w); problem != want {
					t.Errorf("U+%04X sits just outside the set\n got:  |%s|\nwant:  |%s|", r, problem, want)
				}
			}
		}
		if neighbours != 53 {
			t.Fatalf("%d neighbours were exercised, not the 53 the table has; a boundary went unasked", neighbours)
		}

		separated := 0
		for _, r := range listWhitespace {
			separated++
			in := string(r) + "2" + string(r) + "iron-plate" + string(r)
			got, problem := parseIngredientList(in, listRecipe, "crafting", "mymod-parts", w)
			if problem != "" {
				t.Errorf("U+%04X as whitespace was refused: %s", r, problem)
				continue
			}
			if rendered := got.render(); rendered != "2 iron-plate" {
				t.Errorf("U+%04X as whitespace rendered as |%s|", r, rendered)
			}
		}
		if separated != 8 {
			t.Errorf("the whitespace set has %d members, not the eight it is documented as", separated)
		}
		// THE FOUR THAT USED TO BE DELETED are in the set like everything else,
		// and this is the row that says so: a BOM, a zero-width space and the
		// two joiners are answered by their code points rather than silently
		// removed from a text nobody typed that way.
	})
}

// RENDERING THEN PARSING IS AN IDENTITY, over generated lists rather than over
// the corpus alone: the corpus pins the cases somebody thought of, and this
// pins the property they are examples of. It is what the settings layer rests
// on, because "the player left the setting alone" is a comparison between the
// stored text and the rendered default.
//
// The generator is a fixed-seed PRNG written out here rather than math/rand:
// the numbers have to be the same on every run and every toolchain, or a
// failure is one nobody else can reproduce.
// THE DECIMAL RULE, which is neither language's shortest-round-trip printer.
//
// Go's printer and Rust's Display break an exact tie in the last digit
// differently, so the renderer chooses its own digits: the first precision at
// which one of the three candidates reads back as the value, and among those
// the first with an even last digit. The corpus pins the two ties this table
// opens with, and cannot pin the rest: 5e-324 and the largest double render as
// hundreds of digits, which a corpus line would carry twice.
//
// EVERY EXPECTATION IS ALSO ASSERTED TO READ BACK, which is the property the
// settings layer rests on and the half of this test that cannot be satisfied by
// copying what the code happens to print today.
func TestFormatListAmountFollowsTheDecimalRule(t *testing.T) {
	zeros := func(n int) string { return strings.Repeat("0", n) }
	cases := []struct {
		v    float64
		want string
	}{
		// The two exact ties the corpus pins, here as well because this table
		// is where the rule itself is held up.
		{1.00000762939453125, "1.0000076293945312"},
		{1059438285926254.25, "1059438285926254.2"},
		{0.1, "0.1"},
		{0.25, "0.25"},
		{2.5, "2.5"},
		{1e-7, "0.0000001"},
		// Fixed notation whatever the magnitude, because the parser takes no
		// exponent form.
		{1e21, "1" + zeros(21)},
		{1e301, "1" + zeros(301)},
		// The smallest double there is. 4e-324, 5e-324 and 6e-324 all read back
		// as it, and the rule takes the even one, which is NOT what either
		// language's shortest printer writes.
		{5e-324, "0." + zeros(323) + "4"},
		// Not a double: the literal rounds to 9007199254740992 on the way in,
		// and that is what the renderer is asked about.
		{9007199254740993, "9007199254740992"},
		// The largest double. 1.7976931348623158e308 is above its exact value
		// and still reads back as it, and its last digit is even.
		{1.7976931348623157e308, "17976931348623158" + zeros(292)},
	}
	for _, c := range cases {
		got := formatListAmount(c.v)
		if got != c.want {
			t.Errorf("formatListAmount(%v):\n got: %s\nwant: %s", c.v, got, c.want)
			continue
		}
		back, err := strconv.ParseFloat(got, 64)
		if err != nil || back != c.v {
			t.Errorf("formatListAmount(%v) = %s, which reads back as %v (err %v)", c.v, got, back, err)
		}
		if strings.ContainsAny(got, "eE") {
			t.Errorf("formatListAmount(%v) = %s, which is exponent form the parser refuses", c.v, got)
		}
	}
}

func TestRenderThenParseIsIdentity(t *testing.T) {
	// Names chosen for the shapes that need a tag to survive: all digits, a
	// bare sign, the two reserved words, an amount glued to a sign on either
	// side, an x between digits, and a name in exponent form. "both" is an item
	// AND a fluid, which is the tie rule's witness; loader-1x1 is base's own
	// name that contains an x between digits and must stay PLAIN, which is the
	// half of the render rule a list of tagged shapes cannot prove.
	w := &corpusWorld{
		items: []string{
			"iron-plate", "copper-cable", "42", "x", "X", "none", "default",
			"None", "Default", "defaults",
			"2x", "X2", "2x4", "loader-1x1", "1e3", "both",
			"automation-science-pack", "logistic-science-pack",
		},
		fluids: []string{"water", "steam", "both", "42"},
		tools:  []string{"automation-science-pack", "logistic-science-pack"},
	}
	// THE RENDER RULE, both halves of it, asserted before a single round runs: a
	// generator whose fixture quietly lost one of these names would still pass
	// every round. A slice rather than a map, because a map's iteration order
	// is a different failure list on every run.
	renderRule := []struct {
		name    string
		wantTag bool
	}{
		{"iron-plate", false}, {"loader-1x1", false}, {"copper-cable", false},
		{"42", true}, {"x", true}, {"X", true}, {"2x", true},
		{"X2", true}, {"2x4", true}, {"1e3", true},
		{"none", true}, {"default", true},
		// THE RESERVED WORDS ARE MATCHED WITHOUT CASE, so an item called
		// Default takes its tag exactly as one called default does. The fold
		// is an equality: "defaults" is a name and stays plain.
		{"None", true}, {"Default", true}, {"defaults", false},
	}
	for _, c := range renderRule {
		if got := nameNeedsTag(c.name); got != c.wantTag {
			t.Errorf("nameNeedsTag(%q) = %v, want %v", c.name, got, c.wantTag)
		}
	}
	fluidAmounts := []float64{0.1, 2.5, 1e-7, 1e21, 1, 0.25, 1000000000, 3.5, 0.0000001, 65536.5}

	rng := newCorpusRNG(20260907)
	rounds := 2000
	tagged := 0
	for i := 0; i < rounds; i++ {
		var list ingredientList
		// Items and fluids both, in a category that takes fluids, so one
		// generated list exercises both arms of the renderer.
		for _, name := range w.items {
			if rng.next()%3 != 0 {
				continue
			}
			list = append(list, listEntry{kind: kindItem, name: name, amount: int64(rng.next()%uint64(maxItemAmount)) + 1})
		}
		for _, name := range w.fluids {
			if rng.next()%3 != 0 {
				continue
			}
			list = append(list, listEntry{kind: kindFluid, name: name, fluid: fluidAmounts[rng.next()%uint64(len(fluidAmounts))]})
		}
		if len(list) == 0 {
			// The empty list is a list too, and it renders as the one word.
			list = ingredientList{}
		}
		text := renderIngredientList(list)
		if strings.Contains(text, "[item=") {
			tagged++
		}
		back, problem := parseIngredientList(text, listRecipe, "crafting-with-fluid", "mymod-parts", w)
		if problem != "" {
			t.Fatalf("round %d: the rendering |%s| does not parse: %s", i, text, problem)
		}
		if back.isDefault {
			t.Fatalf("round %d: the rendering |%s| parsed back as the default marker", i, text)
		}
		if !sameList(list, back.entries) {
			t.Fatalf("round %d: |%s| parsed back as |%s|\n  want: %s\n  got:  %s",
				i, text, back.render(), showList(list), showList(back.entries))
		}
		if again := back.render(); again != text {
			t.Fatalf("round %d: |%s| rendered again as |%s|", i, text, again)
		}
	}
	// Anti-vacuity again: a generator that never produced a name needing a tag
	// would prove nothing about the half of the renderer that writes one.
	if tagged < rounds/10 {
		t.Fatalf("only %d of %d generated lists carried an item tag; the generator is not reaching the names that need one", tagged, rounds)
	}

	// THE TWO ENDS OF THE RANGE round-trip too, one list at a time, which the
	// random amounts above deliberately leave out: the ceiling renders as 302
	// digits and the smallest double as 326, and several of either in one
	// generated list would run into the 2000-character rule and prove something
	// about the length check instead.
	//
	// 5e-324 is also the value where the decimal rule writes something OTHER
	// than the shortest decimal (4e-324, the even last digit), so it is the
	// witness that the rule's tie break does not cost the identity.
	for _, amount := range []float64{maxFluidAmount, 5e-324} {
		one := ingredientList{{kind: kindFluid, name: "water", fluid: amount}}
		text := renderIngredientList(one)
		back, problem := parseIngredientList(text, listRecipe, "crafting-with-fluid", "mymod-parts", w)
		if problem != "" {
			t.Fatalf("%v rendered as a text that does not parse: %s", amount, problem)
		}
		if !sameList(one, back.entries) {
			t.Fatalf("%v parsed back as %s", amount, showList(back.entries))
		}
	}
}

// The same property for a pack list, which resolves through ToolExists alone.
// A pack whose name would lex as an amount still needs its tag, and reading it
// back asks ToolExists about the tagged name, so the tag arm is exercised here
// too rather than only in the recipe arm.
func TestRenderThenParseIsIdentityForPacks(t *testing.T) {
	w := &corpusWorld{
		items:  []string{"automation-science-pack", "logistic-science-pack", "42", "iron-plate", "default"},
		fluids: []string{"water"},
		tools:  []string{"automation-science-pack", "logistic-science-pack", "42", "X2", "loader-1x1", "default"},
	}
	rng := newCorpusRNG(1234567)
	for i := 0; i < 500; i++ {
		var list ingredientList
		for _, name := range w.tools {
			if rng.next()%2 != 0 {
				continue
			}
			list = append(list, listEntry{kind: kindItem, name: name, amount: int64(rng.next()%uint64(maxItemAmount)) + 1})
		}
		if len(list) == 0 {
			// A pack list is never empty: research refuses none, so the empty
			// rendering would not parse back and the property does not apply.
			continue
		}
		text := renderIngredientList(list)
		back, problem := parseIngredientList(text, listPacks, "", "mymod-packs", w)
		if problem != "" {
			t.Fatalf("round %d: the rendering |%s| does not parse: %s", i, text, problem)
		}
		if back.isDefault {
			t.Fatalf("round %d: the rendering |%s| parsed back as the default marker", i, text)
		}
		if !sameList(list, back.entries) {
			t.Fatalf("round %d: |%s| parsed back as %s, want %s", i, text, showList(back.entries), showList(list))
		}
	}
}

func sameList(a, b ingredientList) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func showList(l ingredientList) string {
	parts := make([]string, 0, len(l))
	for _, e := range l {
		kind := "item"
		amount := strconv.FormatInt(e.amount, 10)
		if e.kind == kindFluid {
			kind, amount = "fluid", formatListAmount(e.fluid)
		}
		parts = append(parts, kind+" "+e.name+" x"+amount)
	}
	return "[" + strings.Join(parts, ", ") + "]"
}

// corpusRNG is a 64-bit xorshift, written out so the generated lists are the
// same numbers on every toolchain and every run. A failure a reviewer cannot
// reproduce is not a failure they can fix.
type corpusRNG struct{ state uint64 }

func newCorpusRNG(seed uint64) *corpusRNG {
	if seed == 0 {
		seed = 1
	}
	return &corpusRNG{state: seed}
}

func (r *corpusRNG) next() uint64 {
	r.state ^= r.state << 13
	r.state ^= r.state >> 7
	r.state ^= r.state << 17
	return r.state
}
