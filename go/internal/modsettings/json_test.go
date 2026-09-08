package modsettings

import (
	"reflect"
	"strings"
	"testing"
)

// KEY ORDER IS THE FILE'S ORDER, and this is the test that says so. Decoding
// into a map would pass every other test in this package and produce a
// different byte string on every run, which is exactly the failure the byte
// golden could not diagnose.
func TestParseJSONKeepsTheOrderTheFileWroteIn(t *testing.T) {
	doc, err := ParseJSON([]byte(`{"version":[2,0,77,0],"startup":{
	  "z-last":1, "a-first":2, "m-middle":3, "b-second":4, "y-fourth":5,
	  "c-third":6, "x-fifth":7, "d-sixth":8, "w-seventh":9, "e-eighth":10
	}}`))
	if err != nil {
		t.Fatalf("it did not parse: %v", err)
	}
	want := []string{"z-last", "a-first", "m-middle", "b-second", "y-fourth",
		"c-third", "x-fifth", "d-sixth", "w-seventh", "e-eighth"}
	var got []string
	for _, e := range doc.Root.Dict[0].Value.Dict {
		got = append(got, e.Key)
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("the setting order moved\n got: %v\nwant: %v", got, want)
	}
}

// THE SPELLING PICKS THE TYPE. An int setting and a double setting are
// different prototypes, and the file says which one a row is for.
func TestParseJSONTypesANumberByItsSpelling(t *testing.T) {
	doc, err := ParseJSON([]byte(`{"version":[2,0,77,0],"startup":{
	  "whole":40, "fractional":20.5, "whole-with-a-point":20.0,
	  "exponent":1e3, "negative":-7, "text":"custom", "yes":true
	}}`))
	if err != nil {
		t.Fatalf("it did not parse: %v", err)
	}
	want := []Value{
		Int(40), Number(20.5), Number(20), Number(1000), Int(-7), Str("custom"), Bool(true),
	}
	entries := doc.Root.Dict[0].Value.Dict
	if len(entries) != len(want) {
		t.Fatalf("got %d settings, want %d", len(entries), len(want))
	}
	for i, e := range entries {
		// Each setting is stored as a dictionary holding one key, value.
		if len(e.Value.Dict) != 1 || e.Value.Dict[0].Key != "value" {
			t.Fatalf("%s is not a dictionary holding value: %#v", e.Key, e.Value)
		}
		if got := e.Value.Dict[0].Value; !reflect.DeepEqual(got, want[i]) {
			t.Errorf("%s came out as %#v, want %#v", e.Key, got, want[i])
		}
	}
}

// All three stages are always written, empty or not: that is the shape the
// engine's own file has, and a file missing one is a file this writer did not
// produce.
func TestParseJSONWritesEveryStage(t *testing.T) {
	doc, err := ParseJSON([]byte(`{"version":[2,0,77,0],"startup":{"a":1}}`))
	if err != nil {
		t.Fatalf("it did not parse: %v", err)
	}
	var got []string
	for _, e := range doc.Root.Dict {
		got = append(got, e.Key)
	}
	want := []string{"startup", "runtime-global", "runtime-per-user"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("the stages are %v, want %v", got, want)
	}
	if len(doc.Root.Dict[1].Value.Dict) != 0 || doc.Root.Dict[2].Value.Dict != nil {
		t.Errorf("an unnamed stage is not empty: %#v", doc.Root.Dict[1:])
	}
}

// A section may be given in any order and the file still comes out in the
// engine's, because the order of the STAGES is the format's and only the order
// of the settings inside one is the author's.
func TestParseJSONOrdersTheStagesItself(t *testing.T) {
	doc, err := ParseJSON([]byte(`{"runtime-per-user":{"a":1},"version":[2,0,77,0],"startup":{"b":2}}`))
	if err != nil {
		t.Fatalf("it did not parse: %v", err)
	}
	if doc.Root.Dict[0].Key != "startup" || doc.Root.Dict[2].Key != "runtime-per-user" {
		t.Errorf("the stages came out in the file's order rather than the engine's: %#v", doc.Root.Dict)
	}
}

func TestParseJSONRefusesWhatItDoesNotUnderstand(t *testing.T) {
	cases := []struct {
		name string
		in   string
		want string
	}{
		{"no version", `{"startup":{}}`, "names no version"},
		{"a short version", `{"version":[2,0],"startup":{}}`, "has 2 parts"},
		{"a long version", `{"version":[2,0,77,0,1],"startup":{}}`, "more than 4 parts"},
		{"a version part that is not a number", `{"version":[2,0,"77",0]}`, "every part of a version is a number"},
		{"a version part out of range", `{"version":[2,0,70000,0],"startup":{}}`, "from 0 to 65535"},
		{"a section nothing knows", `{"version":[2,0,77,0],"data":{}}`, `names "data"`},
		{"a section named twice", `{"version":[2,0,77,0],"startup":{},"startup":{}}`, "names the section startup twice"},
		{"a setting named twice", `{"version":[2,0,77,0],"startup":{"a":1,"a":2}}`, "names the setting a twice"},
		{"a setting holding a list", `{"version":[2,0,77,0],"startup":{"a":[1]}}`, "holds a list"},
		{"a setting holding an object", `{"version":[2,0,77,0],"startup":{"a":{"b":1}}}`, "holds a object"},
		{"a setting holding null", `{"version":[2,0,77,0],"startup":{"a":null}}`, "holds null"},
		{"a whole number too big for 64 bits", `{"version":[2,0,77,0],"startup":{"a":99999999999999999999}}`, "does not fit a signed 64-bit number"},
		{"a comment that is not a string", `{"comment":3,"version":[2,0,77,0]}`, "a comment is a string"},
		{"two documents", `{"version":[2,0,77,0],"startup":{}} {"version":[2,0,77,0]}`, "more than one document"},
		{"not an object", `[1,2]`, "the document begins with"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			_, err := ParseJSON([]byte(c.in))
			if err == nil {
				t.Fatalf("it was accepted")
			}
			if !strings.Contains(err.Error(), c.want) {
				t.Errorf("the refusal does not say why\n got: %s\nwant it to contain: %s", err, c.want)
			}
		})
	}
}

// The comment key exists so the committed row can explain itself, and it must
// reach no byte of the dat.
func TestTheCommentReachesNoByte(t *testing.T) {
	with, err := ParseJSON([]byte(`{"comment":"why this row is what it is","version":[2,0,77,0],"startup":{"a":1}}`))
	if err != nil {
		t.Fatalf("a commented document did not parse: %v", err)
	}
	without, err := ParseJSON([]byte(`{"version":[2,0,77,0],"startup":{"a":1}}`))
	if err != nil {
		t.Fatalf("it did not parse: %v", err)
	}
	if !reflect.DeepEqual(with, without) {
		t.Errorf("the comment changed the document")
	}
}
