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

func TestNoMessageCarriesItsOwnStage(t *testing.T) {
	// The package's own files, test files excluded: a test asserts what a
	// message looks like and so quotes the very shapes this forbids, and this
	// file quotes them twice over.
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
	// A glob that matched nothing would be a test that passes by scanning an
	// empty set, which is the one outcome worse than failing.
	if len(sources) < 5 {
		t.Fatalf("only %d non-test source files found; the scan would prove nothing", len(sources))
	}

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
