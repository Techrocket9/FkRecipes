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
