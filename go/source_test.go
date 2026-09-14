package fkrecipes

import (
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

// A SOURCE-TEXT PROPERTY OVER EVERY MESSAGE THIS PACKAGE CAN BUILD.
//
// The behavioural tests below compose real refusals through real plans, which
// is what proves the composition is right, but each one covers the templates
// its own six plans happen to reach. This package has some eighty message
// chunks and a stage put back into any single one of them ships a sentence the
// player reads twice:
//
//	fklua: at the data stage, fkrecipes: at the data stage, two technologies ...
//
// A sample cannot catch that; only a property over the whole source can. So
// this reads the package's own files and holds every string literal in them to
// the rule, which makes a template added next year covered on the day it is
// written rather than on the day somebody remembers to extend a table.
//
// SCOPED TO STRING LITERALS, and that scoping is load-bearing rather than
// tidiness: " stage, " occurs in ordinary wrapped prose all over the comments
// in this package, so a raw scan of the source text would be all false
// positives and would be deleted within the week. Parsing is the go/ast walk
// below, which needs no heuristics: each operand of a concatenation is its own
// BasicLit, so a message built from six pieces is six independent checks.

// forbiddenInMessages is what a refusal may not carry, and why.
//
// fkdata.Raise routes a message through the host's own failure path, and
// fk_data.lua's fail() prefixes "fklua: at the <stage> stage, " to it. The
// stage therefore belongs to the host at every stage, and a message built here
// carries the library's attribution and goes straight into the diagnosis.
var forbiddenInMessages = []struct {
	needle string
	why    string
}{
	{"fkrecipes: at the", "the old stage-prefixed refusal shape; the host prefixes the stage now"},
	{" stage, ", "a stage the host will name again, giving the player one sentence with two of them"},
}

// packageSources is the package's own files, test files excluded: a test
// asserts what a message looks like and so quotes the very shapes these
// properties forbid, and this file quotes them twice over. Both source
// properties read the same list, because a file one of them stopped covering
// is the failure mode either of them has.
//
// A GLOB THAT MATCHED NOTHING would be a test that passes by scanning an empty
// set, which is the one outcome worse than failing, so the floor is here too.
func packageSources(t *testing.T) []string {
	t.Helper()
	names, err := filepath.Glob("*.go")
	if err != nil {
		t.Fatal(err)
	}
	var sources []string
	for _, n := range names {
		if !strings.HasSuffix(n, "_test.go") {
			sources = append(sources, n)
		}
	}
	if len(sources) < 5 {
		t.Fatalf("only %d non-test source files found; the scan would prove nothing", len(sources))
	}
	return sources
}

func TestNoMessageCarriesItsOwnStage(t *testing.T) {
	sources := packageSources(t)

	fset := token.NewFileSet()
	checked := 0
	for _, path := range sources {
		src, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("the source is the thing under test and it is not readable: %v", err)
		}
		// ParseComments is not asked for: comments are exactly what must stay
		// out of this, and the build tag on the emit layer does not matter
		// here because the parser reads the file either way. That is what puts
		// guest.go, where Raise is actually called, inside the scan.
		file, err := parser.ParseFile(fset, path, src, 0)
		if err != nil {
			t.Fatalf("%s does not parse: %v", path, err)
		}
		ast.Inspect(file, func(n ast.Node) bool {
			lit, ok := n.(*ast.BasicLit)
			if !ok || lit.Kind != token.STRING {
				return true
			}
			text, err := strconv.Unquote(lit.Value)
			if err != nil {
				return true
			}
			checked++
			for _, f := range forbiddenInMessages {
				if strings.Contains(text, f.needle) {
					t.Errorf("%s: a string literal contains %q, %s\n  literal: %q",
						fset.Position(lit.Pos()), f.needle, f.why, text)
				}
			}
			return true
		})
	}
	if checked < 50 {
		t.Fatalf("only %d string literals were scanned across %d files; the walk is not reaching the messages",
			checked, len(sources))
	}
	t.Logf("scanned %d string literals across %d source files", checked, len(sources))
}

// THE CEILING IS NEVER TYPED INTO A MESSAGE, which is what makes "one number
// by construction" a fact rather than a hope.
//
// textFormatLine builds the sentence the player reads from maxListChars, and
// the parser's own too-long refusal does the same. Nothing in a value test can
// see the difference between that and a typed 2000: both render the same bytes
// while the constant happens to be 2000, so a hand-typed digit run would sit
// there green until somebody changed the ceiling and shipped a description
// that lied about it. Only a property over the source can see it, so this
// forbids the ceiling's own decimal rendering in every string literal the
// package builds a message out of.
//
// IT TRACKS THE CONSTANT. The needle is strconv.Itoa(maxListChars) taken at
// run time, so raising the ceiling moves what is forbidden with it. A literal
// that merely CONTAINS those digits is caught too (12000 would be), and that
// is the right side to err on: a number near the ceiling in a player-facing
// sentence is one somebody should have to re-read.
func TestTheCeilingIsNeverTypedIntoAMessage(t *testing.T) {
	sources := packageSources(t)
	needle := strconv.Itoa(maxListChars)

	fset := token.NewFileSet()
	checked := 0
	for _, path := range sources {
		src, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("the source is the thing under test and it is not readable: %v", err)
		}
		file, err := parser.ParseFile(fset, path, src, 0)
		if err != nil {
			t.Fatalf("%s does not parse: %v", path, err)
		}
		ast.Inspect(file, func(n ast.Node) bool {
			lit, ok := n.(*ast.BasicLit)
			if !ok || lit.Kind != token.STRING {
				return true
			}
			text, err := strconv.Unquote(lit.Value)
			if err != nil {
				return true
			}
			checked++
			if strings.Contains(text, needle) {
				t.Errorf("%s: a string literal types the ceiling %s instead of building it from maxListChars\n  literal: %q",
					fset.Position(lit.Pos()), needle, text)
			}
			return true
		})
	}
	if checked < 50 {
		t.Fatalf("only %d string literals were scanned across %d files; the walk is not reaching the messages",
			checked, len(sources))
	}
}

// ---------------------------------------------------------------------------
// The language seam, as a source property.
// ---------------------------------------------------------------------------

// THE SEAM THE BEHAVIOURAL TESTS CANNOT REACH.
//
// TestTheConstructorsInstallTheLanguage proves that a plan declaring no text
// setting installs no language, and that is a property of the plans that test
// builds. It cannot prove that no FUTURE line names the parser or the renderer
// where a planner reaches it, and one such line is all it takes: a name on a
// reachable path is a reference whole-program elimination has to keep, so the
// whole language would be linked into every consumer's mod again, silently,
// with every test still green and the only evidence an fk_data_module.lua tens
// of kilobytes longer. That is a property over the source, so it is asserted
// over the source, and it is the one tripwire here that needs no wasm
// toolchain at all.
//
// ONE RULE, AND THE GUARDED SET IS READ OUT OF THE SOURCE. Every top-level
// function ingredientlist.go declares is guarded, resolveCustomCost with them,
// and no other file may NAME one of them outside the allowances below. A name
// is any *ast.Ident, which is what lets one rule cover a plain call, a method
// call, a method value, a method expression, a function value handed to
// another file, a parenthesised or instantiated callee, a closure, a go and a
// defer alike: every one of them keeps the reference alive, so every one of
// them is the same defect, and the AST spells them all as an identifier.
//
// AN EARLIER VERSION LISTED FIVE NAMES AND FORBADE CALLS, and three shapes
// walked through it with every gate green: (renderIngredientList)(nil), whose
// callee is a ParenExpr and so had no name for the call rule to read; a helper
// in customize.go returning l.resolveCustomCost for data.go to call through,
// which is a call at neither site; and a call to splitEntries, one of the
// forty three top-level functions in ingredientlist.go, of which that list
// named four. The last two grew the notext fixture's module by 87,364 and
// 54,204 bytes with every grep for the language's names still at zero.
//
// METHODS ARE OUT OF THE GUARDED SET on purpose: a method name is a selector
// any type may carry, language.render is one, and a walk with no type
// information cannot tell that field from parsedList.render. That method is
// the only one ingredientlist.go declares, and calling it needs a parsedList,
// which only the guarded parser hands back.
//
// COMMENTS AND STRING LITERALS ARE OUT BY CONSTRUCTION rather than by a rule.
// go/parser is asked for no comments, so a doc comment naming the parser (this
// package has several, and they are how the seam is explained) is not in the
// tree at all; a name inside a string literal is a BasicLit and never an
// Ident. A substring scan would fail on both and would be deleted within the
// week, which is the same reason the message property above walks the AST.

// languageFile is the language's own file, and the guarded set is read out of
// it: a function added there tomorrow is guarded on the day it is written
// rather than on the day somebody remembers a table.
const languageFile = "ingredientlist.go"

// customCostFunc is the one guarded name that lives somewhere else. The data
// planner reaches it through Lib.customCost, which packsSetting installs.
const customCostFunc = "resolveCustomCost"

// languageAllowance is one place a guarded name may still be written. The list
// of them can only SHRINK, which is what separates it from the table this
// property used to be: a new language function needs no row here.
type languageAllowance struct {
	// name is the guarded function and file the base name that may write it.
	name string
	file string
	// funcs are the top-level declarations inside whose bodies it may appear.
	// Empty means anywhere in that file.
	funcs []string
	// definition allows the name of the FuncDecl itself and nothing else, not
	// even a mention inside that function's own body.
	definition bool
}

var languageAllowances = []languageAllowance{
	// The two text-setting constructors are where the language table is
	// built, and that is the whole seam: a consumer who calls neither drops
	// all three references with them. Scoped to the two BODIES rather than to
	// lib.go, because the same name anywhere else in that file, a
	// package-level var initialised at load most of all, is reachable from
	// every plan ever built.
	{name: "parseIngredientList", file: "lib.go", funcs: []string{"ingredientsSetting", "packsSetting"}},
	{name: "renderIngredientList", file: "lib.go", funcs: []string{"ingredientsSetting", "packsSetting"}},
	{name: "formatListAmount", file: "lib.go", funcs: []string{"ingredientsSetting", "packsSetting"}},
	// packsSetting alone installs the custom-cost resolver: a CustomCost holds
	// a PacksSettingRef and only that constructor issues one.
	{name: customCostFunc, file: "lib.go", funcs: []string{"packsSetting"}},
	// Its definition, and nothing else in the file that holds it. customize.go
	// reaches every text path through Lib.lang, so even the file the resolver
	// lives in never writes the name a second time.
	{name: customCostFunc, file: "customize.go", definition: true},
	// THE ONE THAT IS NOT THE LANGUAGE. categoryTakesItemsOnly is the engine's
	// fluid rule as a comparison over two strings, written once so the
	// declared path and the typed path ask one question rather than two that
	// can drift; it names nothing else in ingredientlist.go, so a caller links
	// a comparison and not a language. It is guarded anyway, because the set
	// is read out of the file and not chosen, and this row is what says the
	// exception was decided rather than overlooked.
	{name: "categoryTakesItemsOnly", file: "data.go", funcs: []string{"validateIngredients"}},
}

func TestOnlyTheLanguageFilesNameTheLanguage(t *testing.T) {
	sources := packageSources(t)

	// The set, in the language file's own declaration order.
	guarded := languageFunctions(t)
	if len(guarded) == 0 {
		t.Fatalf("%s declares no top-level function; the guarded set would be empty and this property would pass by asking nothing",
			languageFile)
	}
	home := make(map[string]string, len(guarded)+1)
	for _, name := range guarded {
		home[name] = languageFile
	}
	// The resolver has no home file that may write its name freely.
	home[customCostFunc] = ""

	fset := token.NewFileSet()
	// Counted so the floors below are answered by the walk rather than by
	// hand: a resolver declared nowhere is a name this package no longer has,
	// and an allowance nothing reaches is a hole nobody is watching.
	definedResolver := false
	used := make([]int, len(languageAllowances))
	for _, path := range sources {
		src, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("the source is the thing under test and it is not readable: %v", err)
		}
		// No comments, for the reason the message property gives: they are
		// exactly what must stay out of this. The build tag on the emit layer
		// does not matter here either, so guest.go is inside the scan.
		file, err := parser.ParseFile(fset, path, src, 0)
		if err != nil {
			t.Fatalf("%s does not parse: %v", path, err)
		}
		base := filepath.Base(path)
		// Declaration by declaration rather than over the whole file at once,
		// because WHERE a name is written is half the rule: the enclosing
		// top-level function is what the allowances are scoped to, and a name
		// written outside every function body carries the empty enclosing name
		// and matches no allowance.
		for _, decl := range file.Decls {
			fn, isFunc := decl.(*ast.FuncDecl)
			enclosing := ""
			if isFunc {
				enclosing = fn.Name.Name
				if enclosing == customCostFunc {
					definedResolver = true
				}
			}
			ast.Inspect(decl, func(n ast.Node) bool {
				id, ok := n.(*ast.Ident)
				if !ok {
					return true
				}
				where, isGuarded := home[id.Name]
				if !isGuarded || base == where {
					return true
				}
				if i := allowanceFor(id.Name, base, enclosing, isFunc && id == fn.Name); i >= 0 {
					used[i]++
					return true
				}
				t.Errorf("%s: names %s, which links the ingredient language into every consumer; only %s may name it",
					fset.Position(id.Pos()), id.Name, allowedPlaces(id.Name, where))
				return true
			})
		}
	}
	if !definedResolver {
		t.Errorf("%s is declared in no source file; the seam guards a name this package no longer has", customCostFunc)
	}
	for i, a := range languageAllowances {
		if used[i] == 0 {
			t.Errorf("nothing in %s names %s any more; the allowance outlived the line it was written for, and one nobody uses is a hole nobody is watching",
				allowancePlace(a), a.name)
		}
	}
	t.Logf("seam checked over %d source files: %d guarded names", len(sources), len(guarded)+1)
}

// languageFunctions is every top-level function the language's own file
// declares, in declaration order. Reading them out of the AST is what makes
// this property cover the whole file instead of the handful of names somebody
// happened to think of.
func languageFunctions(t *testing.T) []string {
	t.Helper()
	src, err := os.ReadFile(languageFile)
	if err != nil {
		t.Fatalf("the guarded set is read out of %s and it is not readable: %v", languageFile, err)
	}
	file, err := parser.ParseFile(token.NewFileSet(), languageFile, src, 0)
	if err != nil {
		t.Fatalf("%s does not parse: %v", languageFile, err)
	}
	var names []string
	for _, decl := range file.Decls {
		// Recv nil: methods are out of the set, for the reason the property's
		// own comment gives.
		if fn, ok := decl.(*ast.FuncDecl); ok && fn.Recv == nil {
			names = append(names, fn.Name.Name)
		}
	}
	return names
}

// allowanceFor is the index of the allowance that permits this name in this
// place, or -1. Declaration order, so a name two rows permit reports the same
// row on every run.
func allowanceFor(name, file, enclosing string, isDefinition bool) int {
	for i, a := range languageAllowances {
		if a.name != name || a.file != file {
			continue
		}
		if a.definition {
			if isDefinition {
				return i
			}
			continue
		}
		if len(a.funcs) == 0 || contains(a.funcs, enclosing) {
			return i
		}
	}
	return -1
}

// allowedPlaces is every place a guarded name may be written, as a phrase for
// the refusal. home is the file that owns it, empty for a name no file may
// write freely.
func allowedPlaces(name, home string) string {
	var places []string
	if home != "" {
		places = append(places, home)
	}
	for _, a := range languageAllowances {
		if a.name == name {
			places = append(places, allowancePlace(a))
		}
	}
	if len(places) == 0 {
		return "no file at all"
	}
	return strings.Join(places, " and ")
}

// allowancePlace is one allowance as a phrase.
func allowancePlace(a languageAllowance) string {
	switch {
	case a.definition:
		return a.file + " as its own definition"
	case len(a.funcs) == 0:
		return a.file
	default:
		return a.file + " inside " + strings.Join(a.funcs, " or ")
	}
}

func contains(list []string, want string) bool {
	for _, s := range list {
		if s == want {
			return true
		}
	}
	return false
}

// ---------------------------------------------------------------------------
// The resolved-recipes hand-over, as a source property.
// ---------------------------------------------------------------------------

// ONE DOOR INTO res.recipes, AND THIS IS WHAT KEEPS IT ONE.
//
// resolve answers a recipe's ingredients through four arms: IngredientsFrom, a
// dropdown's chosen preset, a text that takes it over, and a plain list. A
// check written into one of them is missing from three, and from whichever arm
// is added next; resolution.addRecipe exists so there is one place that sees
// every resolved list, and the self-product line lives there.
//
// A FIFTH ARM THAT APPENDS DIRECTLY WOULD PASS EVERY BEHAVIOURAL TEST, because
// a test can only assert about the arms it happens to build a plan through. The
// defect is a property of the source, so it is asserted over the source, and it
// costs no toolchain at all.
//
// A WRITE REFERENCE TO A FIELD NAMED recipes IS THE RULE, and it needs no type
// information to do it. Two slices in the scanned files carry that name: the
// resolution's, which only the hand-over may write, and the Lib's, which only
// the declaration constructor may. A third, world_test.go's w.recipes, is out
// of scope only because packageSources drops every _test.go file, so a fixture
// is free to keep its own.
//
// FIVE SHAPES, BECAUSE ONE OF THEM IS NOT ENOUGH. A plain `res.recipes = ...`
// is the obvious write and the other four are what a refactor reaches for:
// `resolution{recipes: lists}` builds the whole struct at the end, `&res.recipes`
// hands the slice to a helper that appends through the pointer,
// `res.recipes[i] = list` rewrites one list AFTER the hand-over already
// checked it, and `for _, res.recipes = range ...` assigns on every turn.
// Parentheses are stripped first, because gofmt keeps `(res.recipes) = ...`.
// Each of those five was injected and watched go red; a plain READ is
// untouched, because reading one is what the prototype loop does on every
// plan. What is NOT covered is a write that neither assigns nor takes an
// address, `copy(res.recipes, ...)` being the example, and that one cannot
// grow the slice so it cannot be how a fifth arm adds a recipe.
type recipesWriter struct {
	// file is the base name that may write it, fn the top-level declaration
	// inside whose body it may appear.
	file, fn string
	// why is the phrase the refusal ends with.
	why string
}

var recipesWriters = []recipesWriter{
	{
		file: "data.go", fn: "addRecipe",
		why: "the one hand-over every resolved list goes through, so a check written there covers every arm",
	},
	{
		file: "lib.go", fn: "recipe",
		why: "the declaration constructor, which is the plan's own list and not the resolution's",
	},
}

func TestOnlyTheHandOverWritesTheResolvedRecipes(t *testing.T) {
	sources := packageSources(t)

	fset := token.NewFileSet()
	// Counted, so a writer that moved or was inlined away fails here rather
	// than leaving a row nobody reaches: a rule with a stale allowance is a
	// rule that has stopped guarding something.
	used := make([]int, len(recipesWriters))
	for _, path := range sources {
		src, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("the source is the thing under test and it is not readable: %v", err)
		}
		file, err := parser.ParseFile(fset, path, src, 0)
		if err != nil {
			t.Fatalf("%s does not parse: %v", path, err)
		}
		base := filepath.Base(path)
		// Declaration by declaration, because WHERE the write is written is
		// the whole rule: a write outside every function body carries the
		// empty enclosing name and matches no row.
		for _, decl := range file.Decls {
			enclosing := ""
			if fn, ok := decl.(*ast.FuncDecl); ok {
				enclosing = fn.Name.Name
			}
			for _, w := range recipesWritesIn(decl) {
				i := recipesWriterFor(base, enclosing)
				if i < 0 {
					t.Errorf("%s: %s .recipes inside %s; only %s may",
						fset.Position(w.pos), w.how, enclosingName(enclosing),
						recipesWritersPhrase())
					continue
				}
				used[i]++
			}
		}
	}
	for i, wr := range recipesWriters {
		if used[i] != 1 {
			t.Errorf("%s writes .recipes %d times; it is meant to write it exactly once, being %s",
				wr.file+" inside "+wr.fn, used[i], wr.why)
		}
	}
}

func recipesWriterFor(file, enclosing string) int {
	for i, wr := range recipesWriters {
		if wr.file == file && wr.fn == enclosing {
			return i
		}
	}
	return -1
}

// recipesWrite is one write reference to a recipes field: where it is, and
// which of the five shapes it took, because the refusal reads very differently
// for an assignment and for an address handed to a helper.
type recipesWrite struct {
	pos token.Pos
	how string
}

// recipesWritesIn walks one top-level declaration and reports every write
// reference to a field named recipes inside it.
func recipesWritesIn(decl ast.Decl) []recipesWrite {
	var found []recipesWrite
	ast.Inspect(decl, func(n ast.Node) bool {
		switch v := n.(type) {
		case *ast.AssignStmt:
			for _, lhs := range v.Lhs {
				if sel, ok := recipesTarget(lhs); ok {
					found = append(found, recipesWrite{sel.Pos(), "writes"})
				}
			}
		case *ast.RangeStmt:
			// `for _, res.recipes = range xs {}` assigns on every turn, and
			// the assignment is written nowhere a statement walk would see it.
			for _, e := range []ast.Expr{v.Key, v.Value} {
				if e == nil {
					continue
				}
				if sel, ok := recipesTarget(e); ok {
					found = append(found, recipesWrite{sel.Pos(), "writes"})
				}
			}
		case *ast.UnaryExpr:
			// `&res.recipes` is a write the moment somebody appends through
			// the pointer, and the append is then in another function
			// entirely.
			if v.Op != token.AND {
				return true
			}
			if sel, ok := recipesTarget(v.X); ok {
				found = append(found, recipesWrite{sel.Pos(), "takes the address of"})
			}
		case *ast.CompositeLit:
			// `resolution{recipes: lists}` is the shape a build-it-at-the-end
			// refactor produces, and it never assigns to a selector at all.
			for _, elt := range v.Elts {
				kve, ok := elt.(*ast.KeyValueExpr)
				if !ok {
					continue
				}
				key, ok := kve.Key.(*ast.Ident)
				if !ok || key.Name != "recipes" {
					continue
				}
				found = append(found, recipesWrite{key.Pos(), "fills in"})
			}
		}
		return true
	})
	return found
}

// recipesTarget is the selector a write lands on, with the parentheses and the
// indexes stripped: `(res.recipes)` and `res.recipes[i]` are both writes to the
// resolution's slice, and gofmt keeps either as written.
func recipesTarget(e ast.Expr) (*ast.SelectorExpr, bool) {
	for {
		switch v := e.(type) {
		case *ast.ParenExpr:
			e = v.X
		case *ast.IndexExpr:
			e = v.X
		default:
			sel, ok := e.(*ast.SelectorExpr)
			if !ok || sel.Sel.Name != "recipes" {
				return nil, false
			}
			return sel, true
		}
	}
}

// enclosingName is the top-level declaration a write sits in, as a phrase: a
// write outside every function body has no name to report.
func enclosingName(enclosing string) string {
	if enclosing == "" {
		return "no function body"
	}
	return enclosing
}

// recipesWritersPhrase names every row WITH ITS OWN REASON, so a refusal about
// one row cannot be handed the other row's why and reordering the table cannot
// silently make every message wrong.
func recipesWritersPhrase() string {
	places := make([]string, 0, len(recipesWriters))
	for _, wr := range recipesWriters {
		places = append(places, wr.file+"'s "+wr.fn+" ("+wr.why+")")
	}
	return strings.Join(places, " and ")
}

// ---------------------------------------------------------------------------
// Every refusal site, classified, as a source property.
// ---------------------------------------------------------------------------

// THE TWO-SIDED RULE, MADE MECHANICAL.
//
// A CHECK THAT ASKS THE World ANYTHING IS ENVIRONMENTAL AND DEGRADES LOUDLY; A
// CHECK THAT READS ONLY THE DECLARATION IS AN AUTHOR BUG AND REFUSES BY NAME.
// An environmental check stays a refusal only where degrading would overwrite
// another mod's prototype, hand the engine something it refuses anyway, or
// invent a value the author never declared; and one more, which the threat
// model rather than the rule supplies: a site the game cannot reach at all.
//
// THE CLASSIFICATION KEYS ON WHAT THE CHECK ASKS, not on where an author would
// have noticed it. A fixture World that answers ToolExists true for a pack the
// shipped mod set demotes is green in development and red in play, so "visible
// in development" is a property of the author's fixture and this library cannot
// see a fixture.
//
// WHY A SOURCE PROPERTY. Nothing in a behavioural test can see a refusal ADDED
// to a World-asking function: the new sentence has its own new test, every old
// test stays green, and the policy has changed with no reader. So the rule is
// asserted over the source, and it costs no toolchain at all.
//
// THE COUNT IS PART OF THE RULE, and only where the function can see the game.
// A function that never names a World cannot ask one anything, so every refusal
// in it reads the declaration by construction and a new one needs no thought;
// a function that DOES name one is pinned at its site count, so a refusal added
// beside the environmental ones goes red until somebody classifies it.
const (
	// classDeclaration: every refusal in this function reads only what the
	// consumer declared. Enforced, not asserted: such a function may not name
	// a World.
	classDeclaration = "reads only the declaration"
	// classEnvironmental: every refusal in this function is an answer the game
	// gave, and the row says which enumerated exception keeps it a refusal.
	classEnvironmental = "asks the game"
	// classMixed: the function holds both, and its environmental sites are
	// listed one by one below.
	classMixed = "holds both kinds"
	// classNotARefusal: the function builds no sentence of its own. It
	// decorates a message some other site composed, and that site is where the
	// policy question was answered. The Rust mirror needs no such row because
	// its needles are the Err constructors and a decorator never reaches one.
	classNotARefusal = "carries no sentence of its own"
)

// The enumerated exceptions, and nothing else may appear in a row. Each is the
// reason degrading would be WORSE than refusing.
const (
	exceptOverwrites    = "degrading silently clobbers a stranger's prototype"
	exceptEngineRefuses = "degrading hands the engine something it refuses with a worse message"
	exceptInvents       = "there is nothing declared to degrade to, so the library would have to invent a value"
	exceptUnreachable   = "the game cannot reach it: the engine resets an unoffered stored value before any stage runs (measured), so only a hand-edited file arrives here, which the threat model puts out of scope"
)

// environmentalSite is one refusal inside a mixed function: a fragment of the
// sentence distinctive enough to name exactly one site, and the exception it
// claims.
type environmentalSite struct {
	fragment string
	except   string
}

// refusalClass is one function's row.
type refusalClass struct {
	file, fn string
	class    string
	// except is the exception an all-environmental row claims; empty for the
	// other two classes.
	except string
	// sites are the environmental refusals inside a mixed function.
	sites []environmentalSite
	// n is the number of refusal sites the function holds, pinned for every
	// function that can see the game so that one added goes red. Zero for a
	// classDeclaration row, which needs no pin: such a function names no World
	// and so cannot hold an environmental refusal at all.
	n   int
	why string
}

var refusalClasses = []refusalClass{
	{
		file: "customize.go", fn: "refuseCostNumbers", class: classDeclaration, n: 2,
		why: "it answers about a DECLARED default and never about a stored value: a number the player's setting answered has already fallen back by the time it runs",
	},
	{file: "customize.go", fn: "researchNumberMaximum", class: classDeclaration},
	{file: "customize.go", fn: "validateBindings", class: classDeclaration},
	{file: "customize.go", fn: "validateCustomCost", class: classDeclaration},
	{file: "customize.go", fn: "validateDeclaredPacks", class: classDeclaration},
	{file: "customize.go", fn: "validateTextSettings", class: classDeclaration},
	{
		file: "cycle.go", fn: "checkCycles", class: classEnvironmental, n: 1,
		except: exceptEngineRefuses,
		why:    "the one ring it still refuses holds NO edge this plan made: every edge of ours is dropped with a line and a tooltip first, so what is left is the game's own tree looping without this mod in it, which the engine refuses on its own and which the threat model puts out of scope",
	},
	{
		file: "data.go", fn: "PlanData", class: classMixed, n: 3,
		sites: []environmentalSite{
			{fragment: "was given a nil World", except: exceptInvents},
			{fragment: "the mod name is empty", except: exceptInvents},
		},
		why: "two host-wiring faults that ask the World whether it is there at all, and one declaration fault (a Lib built without New); the carried refusal is raised again in afterResolution, which resolve already classified",
	},
	{
		file: "data.go", fn: "afterResolution", class: classNotARefusal, n: 1,
		why: "it raises again the sentence resolve composed and classified; the three checks it runs are classified where they are written",
	},
	{file: "data.go", fn: "checkExtra", class: classDeclaration},
	{
		file: "data.go", fn: "fallbackFact", class: classNotARefusal, n: 1,
		why: "it re-raises the sentence a check above it composed with one added fact, that a stored value was set aside; nothing here decides whether a load stops",
	},
	{
		file: "data.go", fn: "checkResolvedCraftTimes", class: classDeclaration, n: 1,
		why: "the stored value it is about is gone by the time it runs, so the number left is the plan's own declared default",
	},
	{file: "data.go", fn: "matchesAllowedValues", class: classDeclaration},
	{
		file: "data.go", fn: "readDropdown", class: classEnvironmental, n: 1,
		except: exceptUnreachable,
	},
	{
		file: "data.go", fn: "validate", class: classMixed, n: 47,
		sites: []environmentalSite{
			{fragment: "the item  already exists in data.raw", except: exceptOverwrites},
			{fragment: "the recipe  already exists in data.raw", except: exceptOverwrites},
			{fragment: "the technology  already exists in data.raw", except: exceptOverwrites},
			{fragment: "names a place_result  that does not exist", except: exceptInvents},
			{fragment: " produces , which does not exist", except: exceptInvents},
			{fragment: "no technology of that name exists", except: exceptInvents},
			{fragment: "is a research_trigger technology", except: exceptInvents},
			{fragment: "carries no unit to copy", except: exceptInvents},
			{fragment: "has a unit that is not a dictionary", except: exceptEngineRefuses},
			{fragment: "the unit of  holds a table", except: exceptEngineRefuses},
			{fragment: "the max_level of  holds a table", except: exceptEngineRefuses},
		},
		why: "the three data.raw collisions, the two named-prototype probes and the six CostOf sentences are what it asks the game; the other thirty-six read the declaration",
	},
	{file: "data.go", fn: "validateIngredients", class: classDeclaration},
	{file: "data.go", fn: "validateNoDuplicatePacks", class: classDeclaration},
	{file: "data.go", fn: "validateNoDuplicates", class: classDeclaration},
	{file: "data.go", fn: "validateUnit", class: classDeclaration},
	{
		file: "settings.go", fn: "PlanSettings", class: classDeclaration,
		why: "the settings stage is handed a Named and not a World: data.raw does not exist yet, so there is no game here to ask",
	},
	{file: "settings.go", fn: "validateSettings", class: classDeclaration},
}

// ---------------------------------------------------------------------------
// The same table, written the other way round.
// ---------------------------------------------------------------------------

// refusalPolicy pins every row's CLASS and every exception it claims, and it is
// the answer to the one thing the table above cannot check about itself: a
// relabel.
//
// WHY A SECOND LIST AT ALL. refusalClasses is a table a reader edits while
// looking at the function they just changed, and the only thing stopping a row
// being moved from "asks the game" to "reads only the declaration" was the
// World check, which every function that takes a resolution rather than a World
// slips past: checkResolvedCraftTimes takes res alone, so one word could make
// the whole rule stop guarding such a function with the suite green. Now a relabel needs TWO edits in two places, and the second
// place is grouped by CLASS, so the row has to be carried out of one group and
// into another where a reviewer reading the diff sees it.
//
// IT PINS THE EXCEPTIONS TOO, both the row's own and its sites', in order. The
// table above only ever asked whether an exception was ONE OF the enumerated
// four; which of the four a site claims is the whole content of the
// classification, and it was unpinned.
type policyRow struct {
	file, fn string
	class    string
	except   string
	// sites is the row's environmental site exceptions, in the order the table
	// above lists them, joined by " | ". Empty where there are none.
	sites string
}

var refusalPolicy = []policyRow{
	// Reads only the declaration. None of these may name a World at all.
	{file: "customize.go", fn: "refuseCostNumbers", class: classDeclaration},
	{file: "customize.go", fn: "researchNumberMaximum", class: classDeclaration},
	{file: "customize.go", fn: "validateBindings", class: classDeclaration},
	{file: "customize.go", fn: "validateCustomCost", class: classDeclaration},
	{file: "customize.go", fn: "validateDeclaredPacks", class: classDeclaration},
	{file: "customize.go", fn: "validateTextSettings", class: classDeclaration},
	{file: "data.go", fn: "checkExtra", class: classDeclaration},
	{file: "data.go", fn: "checkResolvedCraftTimes", class: classDeclaration},
	{file: "data.go", fn: "matchesAllowedValues", class: classDeclaration},
	{file: "data.go", fn: "validateIngredients", class: classDeclaration},
	{file: "data.go", fn: "validateNoDuplicatePacks", class: classDeclaration},
	{file: "data.go", fn: "validateNoDuplicates", class: classDeclaration},
	{file: "data.go", fn: "validateUnit", class: classDeclaration},
	{file: "settings.go", fn: "PlanSettings", class: classDeclaration},
	{file: "settings.go", fn: "validateSettings", class: classDeclaration},

	// Asks the game, and every refusal in it is kept by the one exception named
	// here.
	{file: "cycle.go", fn: "checkCycles", class: classEnvironmental, except: exceptEngineRefuses},
	{file: "data.go", fn: "readDropdown", class: classEnvironmental, except: exceptUnreachable},

	// Holds both, and the exceptions are its sites', in the order the table
	// above lists them.
	{file: "data.go", fn: "PlanData", class: classMixed, sites: exceptInvents + " | " + exceptInvents},
	{
		file: "data.go", fn: "validate", class: classMixed,
		sites: exceptOverwrites + " | " + exceptOverwrites + " | " + exceptOverwrites + " | " +
			exceptInvents + " | " + exceptInvents + " | " + exceptInvents + " | " +
			exceptInvents + " | " + exceptInvents + " | " +
			exceptEngineRefuses + " | " + exceptEngineRefuses + " | " + exceptEngineRefuses,
	},

	// Builds no sentence of its own.
	{file: "data.go", fn: "afterResolution", class: classNotARefusal},
	{file: "data.go", fn: "fallbackFact", class: classNotARefusal},
}

// TestTheTwoClassificationListsAgree is the relabel guard. It says nothing
// about the source; it holds the two tables to each other, which is what makes
// a one-word class change a red suite.
func TestTheTwoClassificationListsAgree(t *testing.T) {
	for _, rc := range refusalClasses {
		i := policyFor(rc.file, rc.fn)
		if i < 0 {
			t.Errorf("%s in %s is classified %q and refusalPolicy does not carry it; every row is pinned in both lists so a relabel cannot be one edit",
				rc.fn, rc.file, rc.class)
			continue
		}
		pr := refusalPolicy[i]
		if pr.class != rc.class {
			t.Errorf("%s in %s: refusalClasses says %q and refusalPolicy says %q; one of the two is a relabel",
				rc.fn, rc.file, rc.class, pr.class)
		}
		if pr.except != rc.except {
			t.Errorf("%s in %s: refusalClasses keeps it a refusal because %q and refusalPolicy says %q",
				rc.fn, rc.file, rc.except, pr.except)
		}
		if got := siteExcepts(rc.sites); got != pr.sites {
			t.Errorf("%s in %s: the site exceptions are\n %s\nand refusalPolicy pins\n %s", rc.fn, rc.file, got, pr.sites)
		}
	}
	for _, pr := range refusalPolicy {
		if refusalClassFor(pr.file, pr.fn) < 0 {
			t.Errorf("%s in %s is pinned in refusalPolicy and refusalClasses does not carry it; a row deleted from one list is deleted from both",
				pr.fn, pr.file)
		}
	}
	if len(refusalPolicy) != len(refusalClasses) {
		t.Errorf("the two classification lists hold %d and %d rows; they are the same rows written twice",
			len(refusalPolicy), len(refusalClasses))
	}
}

func policyFor(file, fn string) int {
	for i, pr := range refusalPolicy {
		if pr.file == file && pr.fn == fn {
			return i
		}
	}
	return -1
}

func siteExcepts(sites []environmentalSite) string {
	out := make([]string, 0, len(sites))
	for _, s := range sites {
		out = append(out, s.except)
	}
	return strings.Join(out, " | ")
}

func TestEveryRefusalSiteIsClassified(t *testing.T) {
	sources := packageSources(t)

	fset := token.NewFileSet()
	seen := make([]int, len(refusalClasses))
	// Per row, per site: how many refusals each fragment matched, so a
	// fragment that names nothing and one that names two are both failures.
	matched := make([][]int, len(refusalClasses))
	for i, rc := range refusalClasses {
		matched[i] = make([]int, len(rc.sites))
	}

	for _, path := range sources {
		src, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("the source is the thing under test and it is not readable: %v", err)
		}
		file, err := parser.ParseFile(fset, path, src, 0)
		if err != nil {
			t.Fatalf("%s does not parse: %v", path, err)
		}
		base := filepath.Base(path)
		for _, decl := range file.Decls {
			fn, ok := decl.(*ast.FuncDecl)
			if !ok {
				continue
			}
			refusals := refusalsIn(fn)
			if len(refusals) == 0 {
				continue
			}
			i := refusalClassFor(base, fn.Name.Name)
			if i < 0 {
				t.Errorf("%s: %s builds %d refusal(s) and no row classifies it; add one to refusalClasses saying whether it reads only the declaration or asks the game, and if it asks the game which of the enumerated exceptions keeps it a refusal",
					fset.Position(fn.Pos()), fn.Name.Name, len(refusals))
				continue
			}
			rc := refusalClasses[i]
			seen[i] = len(refusals)
			if rc.class == classDeclaration && namesAWorld(fn) {
				t.Errorf("%s: %s is classified %q and names a World; a check that can ask the game is environmental and needs an exception",
					fset.Position(fn.Pos()), fn.Name.Name, classDeclaration)
			}
			if rc.class != classMixed {
				continue
			}
			for _, r := range refusals {
				hits := 0
				for j, s := range rc.sites {
					if strings.Contains(r.message, s.fragment) {
						matched[i][j]++
						hits++
					}
				}
				if hits > 1 {
					t.Errorf("%s: the refusal %q matches %d of %s's environmental fragments; each fragment has to name exactly one site",
						fset.Position(r.pos), r.message, hits, rc.fn)
				}
			}
		}
	}

	for i, rc := range refusalClasses {
		if seen[i] == 0 {
			t.Errorf("%s in %s builds no refusal any more; a row with a stale allowance is a rule that has stopped guarding something",
				rc.fn, rc.file)
			continue
		}
		if rc.n != 0 && seen[i] != rc.n {
			t.Errorf("%s in %s builds %d refusals and the row pins %d; classify the one that moved (%s)",
				rc.fn, rc.file, seen[i], rc.n, rc.why)
		}
		if rc.n == 0 && rc.class != classDeclaration {
			t.Errorf("%s in %s is classified %q with no site count; only a row that reads the declaration may go unpinned",
				rc.fn, rc.file, rc.class)
		}
		// A MIXED ROW WITH NO SITES ASSERTS NOTHING, which is the third way a
		// relabel could have got past this test: classMixed skips the World
		// check and the site walk both, so an all-environmental function moved
		// into it with an empty list would be unconstrained. A mixed row says
		// which of its refusals ask the game, and by construction it holds at
		// least one that does not.
		if rc.class == classMixed && len(rc.sites) == 0 {
			t.Errorf("%s in %s is classified %q and lists no environmental site; a mixed row names the refusals that ask the game, one by one",
				rc.fn, rc.file, classMixed)
		}
		if rc.class == classMixed && rc.n <= len(rc.sites) {
			t.Errorf("%s in %s is classified %q with %d refusals and %d of them environmental; a row whose refusals ALL ask the game is %q with one exception, not mixed",
				rc.fn, rc.file, classMixed, rc.n, len(rc.sites), classEnvironmental)
		}
		if (rc.class == classEnvironmental) != (rc.except != "") {
			t.Errorf("%s in %s is classified %q and %s an exception; exactly one of the two is right",
				rc.fn, rc.file, rc.class, map[bool]string{true: "carries", false: "carries no"}[rc.except != ""])
		}
		if rc.except != "" && !contains(allowedExceptions, rc.except) {
			t.Errorf("%s in %s claims an exception that is not one of the enumerated ones", rc.fn, rc.file)
		}
		for j, s := range rc.sites {
			if !contains(allowedExceptions, s.except) {
				t.Errorf("%s in %s: the site %q claims an exception that is not one of the enumerated ones", rc.fn, rc.file, s.fragment)
			}
			if matched[i][j] != 1 {
				t.Errorf("%s in %s: the environmental fragment %q matches %d refusals; it has to name exactly one",
					rc.fn, rc.file, s.fragment, matched[i][j])
			}
		}
	}
}

var allowedExceptions = []string{exceptOverwrites, exceptEngineRefuses, exceptInvents, exceptUnreachable}

func refusalClassFor(file, fn string) int {
	for i, rc := range refusalClasses {
		if rc.file == file && rc.fn == fn {
			return i
		}
	}
	return -1
}

// refusalSite is one refusal a function builds: where it is, and the string
// literals its message is made of, joined. A message built from six pieces is
// six BasicLits in one call, and joining them is what lets a fragment name a
// site without the test knowing how the sentence was assembled.
type refusalSite struct {
	pos     token.Pos
	message string
}

// refusalsIn walks one function and reports every refusal it builds:
// errors.New, fmt.Errorf and resolution.refuse alike, because all three stop a
// load and nothing else in this package does.
func refusalsIn(fn *ast.FuncDecl) []refusalSite {
	var out []refusalSite
	ast.Inspect(fn, func(node ast.Node) bool {
		call, ok := node.(*ast.CallExpr)
		if !ok {
			return true
		}
		sel, ok := call.Fun.(*ast.SelectorExpr)
		if !ok {
			return true
		}
		id, isIdent := sel.X.(*ast.Ident)
		switch {
		case sel.Sel.Name == "refuse":
		case isIdent && id.Name == "errors" && sel.Sel.Name == "New":
		case isIdent && id.Name == "fmt" && sel.Sel.Name == "Errorf":
		default:
			return true
		}
		out = append(out, refusalSite{pos: call.Pos(), message: literalsIn(call)})
		return true
	})
	return out
}

func literalsIn(n ast.Node) string {
	var b strings.Builder
	ast.Inspect(n, func(node ast.Node) bool {
		lit, ok := node.(*ast.BasicLit)
		if !ok || lit.Kind != token.STRING {
			return true
		}
		if s, err := strconv.Unquote(lit.Value); err == nil {
			b.WriteString(s)
		}
		return true
	})
	return b.String()
}

// namesAWorld reports whether a function can ask the game anything, which is
// exactly whether a World reaches it: the receiver, a parameter, or a
// parameter's own type. It is the one channel by which a fact about data.raw
// enters this package.
func namesAWorld(fn *ast.FuncDecl) bool {
	fields := []*ast.Field{}
	if fn.Recv != nil {
		fields = append(fields, fn.Recv.List...)
	}
	if fn.Type.Params != nil {
		fields = append(fields, fn.Type.Params.List...)
	}
	for _, f := range fields {
		if strings.Contains(typeName(f.Type), "World") {
			return true
		}
	}
	return false
}

func typeName(e ast.Expr) string {
	var b strings.Builder
	ast.Inspect(e, func(node ast.Node) bool {
		if id, ok := node.(*ast.Ident); ok {
			b.WriteString(id.Name + " ")
		}
		return true
	})
	return b.String()
}

// ---------------------------------------------------------------------------
// Every note call site, accounted for, as a source property.
// ---------------------------------------------------------------------------

// THE NOTE SET, MADE MECHANICAL.
//
// THE LOG IS NOT A DISCLOSURE, so every degradation this library makes leaves a
// trailing line in the prototype's own description. The defect that rule exists
// to prevent is a degradation that writes NO note, and until this test the only
// thing standing against it was four hand-maintained prose enumerations that
// nothing read: the block comment over the composers in each half, the fixture
// list in localised_test.go, docs/usage.md's fenced block and the design
// record. A prose list is a list somebody forgets, which is exactly how the arm
// this round repaired went silent for a whole cycle.
//
// WHAT IS GUARDED IS THE CALL SITES AND NOT THE SENTENCES. Whether a given
// sentence is the right sentence is what the behavioural tests and the corpus
// are for. This asks one narrower question the behavioural tests cannot: is
// every composer the source hands to noteOn accounted for, and is every
// composer the table carries still handed to it. A note added without a row
// goes red naming the composer, and a row left behind by a deleted call goes
// red naming it too.
//
// THE PLAYER'S FALLBACK IS IN THE SET. fallbackNote is not an environmental
// degradation at all, and it is here anyway: the property is "every call site
// is accounted for", and a set with a hand-written exclusion in it is a set
// with a hole where the exclusion is. The row says which kind it is instead.
//
// THE BYTE LENGTH IS PINNED AT AN EMPTY ARGUMENT SLOT, which is the one length
// a sentence has that does not depend on a mod set: the composer is called with
// nothing in the slot its name goes in. It is what the chunker's budget and
// appendLocalised's arithmetic are stated against, and a sentence that grows
// past 180 bytes with an empty slot is one whose every composition is chunked
// on every mod set. The number moving is a sentence moving, which is a corpus
// change and a documents change, so it is pinned here to make that one failure
// rather than a silent drift.
type noteComposer struct {
	// name is the identifier the source calls, exactly.
	name string
	// bytes is len(empty()), pinned.
	bytes int
	// player marks the one composer that is a PLAYER's fallback rather than an
	// environmental degradation: something the player typed was set aside.
	player bool
	// what is one line saying what the degradation is.
	what string
	// empty is the sentence with nothing in the slot a name goes in.
	empty func() string
}

var noteComposers = []noteComposer{
	{
		name: "fallbackNote", bytes: 128, player: true,
		what:  "a stored value the player typed could not be used, so the field behaved as though it had been left alone",
		empty: func() string { return fallbackNote("", false) },
	},
	{
		name: "packDroppedNote", bytes: 84,
		what:  "the chosen source lost SOME of its science packs to the tool probe",
		empty: func() string { return packDroppedNote("") },
	},
	{
		name: "packlessSourceNote", bytes: 132,
		what:  "the chosen source lost EVERY pack to the tool probe, and a declared cost sits behind it",
		empty: func() string { return packlessSourceNote("") },
	},
	{
		name: "unpricedSourceNote", bytes: 168,
		what:  "no source in the chosen ladder handed the library a cost it could copy, so the research has no prerequisite either",
		empty: unpricedSourceNote,
	},
	{
		name: "packlessNote", bytes: 122,
		what:  "every pack the research names was put to the game and the game had none of them",
		empty: packlessNote,
	},
	{
		name: "unreadableSourceNote", bytes: 135,
		what:  "the chosen source's pack list is in neither engine form, and a declared cost sits behind it",
		empty: func() string { return unreadableSourceNote("") },
	},
	{
		name: "unreadableCopyNote", bytes: 121,
		what:  "the same list with nothing declared behind it, so the unit is emitted with no packs at all",
		empty: func() string { return unreadableCopyNote("") },
	},
	{
		name: "clampedItemNote", bytes: 128,
		what:  "two ladders landed on one item above the engine's 65535",
		empty: func() string { return clampedItemNote("") },
	},
	{
		name: "clampedFluidNote", bytes: 145,
		what:  "two ladders landed on one fluid above the engine's 1e301",
		empty: func() string { return clampedFluidNote("") },
	},
	{
		name: "cyclePrereqNote", bytes: 129,
		what:  "a prerequisite this plan made would loop the technology tree, so it was dropped",
		empty: func() string { return cyclePrereqNote("") },
	},
	{
		name: "cycleSpliceNote", bytes: 131,
		what:  "a splice this plan made would loop the technology tree, so it was dropped",
		empty: func() string { return cycleSpliceNote("") },
	},
}

// noteDecorators are the wrappers a composer's sentence may be handed to on the
// way into noteOn. They compose no sentence of their own, so they are looked
// THROUGH rather than counted: see withDestruction, which joins the engine's
// own permanent cost to a note whose ingredient list moved.
var noteDecorators = []string{"withDestruction"}

// TestEveryNoteCallSiteIsAccountedFor is the property. It reads the package's
// own files, finds every noteOn call, names the composer each one hands over,
// and holds that set to the table above in both directions.
func TestEveryNoteCallSiteIsAccountedFor(t *testing.T) {
	sources := packageSources(t)

	fset := token.NewFileSet()
	seen := make(map[string]int)
	sites := 0
	for _, path := range sources {
		src, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("the source is the thing under test and it is not readable: %v", err)
		}
		file, err := parser.ParseFile(fset, path, src, 0)
		if err != nil {
			t.Fatalf("%s does not parse: %v", path, err)
		}
		ast.Inspect(file, func(n ast.Node) bool {
			call, ok := n.(*ast.CallExpr)
			if !ok {
				return true
			}
			sel, ok := call.Fun.(*ast.SelectorExpr)
			if !ok || sel.Sel.Name != "noteOn" || len(call.Args) != 2 {
				return true
			}
			sites++
			named := composersIn(call.Args[1])
			switch len(named) {
			case 1:
				seen[named[0]]++
			case 0:
				t.Errorf("%s: a noteOn call hands over a sentence no composer built;"+
					" every note is one named composer so this test can account for it",
					fset.Position(call.Pos()))
			default:
				t.Errorf("%s: a noteOn call names %v; exactly one of them is the composer and"+
					" the rest have to be decorators listed in noteDecorators",
					fset.Position(call.Pos()), named)
			}
			return true
		})
	}

	// A WALK THAT FOUND NOTHING WOULD PASS EVERY ASSERTION BELOW that is not
	// about the table, so the floor is here as it is in the other properties.
	if sites < len(noteComposers) {
		t.Fatalf("only %d noteOn call sites were found across %d files; the table carries %d composers"+
			" and the walk is not reaching them", sites, len(sources), len(noteComposers))
	}

	for _, nc := range noteComposers {
		if seen[nc.name] == 0 {
			t.Errorf("noteComposers carries %s (%s) and no noteOn call hands it over any more;"+
				" a composer nothing records is a row that has stopped guarding something, so delete both or restore the call",
				nc.name, nc.what)
		}
		if got := len(nc.empty()); got != nc.bytes {
			t.Errorf("%s composes %d bytes with an empty argument slot and the table pins %d;"+
				" the sentence moved, so move testdata, docs/usage.md and the design record with it\n  %q",
				nc.name, got, nc.bytes, nc.empty())
		}
	}
	for name := range seen {
		if noteComposerFor(name) < 0 {
			t.Errorf("%s is handed to noteOn and noteComposers does not carry it;"+
				" add a row with the sentence's byte length at an empty argument slot and one line saying what the degradation is,"+
				" and add the composition to worstCasePlan, docs/usage.md and the design record in the same commit",
				name)
		}
	}

	// EXACTLY ONE PLAYER'S FALLBACK, which is the one asymmetry in the table
	// and the one worth a check of its own: a second composer marked that way
	// would mean an environmental degradation had been relabelled as something
	// the player typed, which is the confusion the two voices exist to prevent.
	players := 0
	for _, nc := range noteComposers {
		if nc.player {
			players++
		}
	}
	if players != 1 {
		t.Errorf("%d composers are marked a PLAYER's fallback; there is one, and everything else"+
			" this library writes into a description is an ENVIRONMENTAL degradation nobody typed", players)
	}
	t.Logf("accounted for %d noteOn call sites across %d composers", sites, len(noteComposers))
}

func noteComposerFor(name string) int {
	for i, nc := range noteComposers {
		if nc.name == name {
			return i
		}
	}
	return -1
}

// composersIn reports every function a note argument calls, with the decorators
// looked through. A composer is what is left.
func composersIn(arg ast.Expr) []string {
	var out []string
	ast.Inspect(arg, func(n ast.Node) bool {
		call, ok := n.(*ast.CallExpr)
		if !ok {
			return true
		}
		id, ok := call.Fun.(*ast.Ident)
		if !ok {
			return true
		}
		if contains(noteDecorators, id.Name) {
			return true
		}
		out = append(out, id.Name)
		return true
	})
	return out
}
