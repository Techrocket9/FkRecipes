// Command writesettings turns the harness's JSON description of a settings
// file into the mod-settings.dat the engine reads.
//
//	go run ./internal/modsettings/cmd/writesettings \
//	    -in ../testdata/ingame/flipped.json -out MODS/mod-settings.dat
//
// IT IS RUN FROM THE LIBRARY MODULE and imports nothing but the standard
// library and the package beside it, so scripts/run-ingame.sh can reach it with
// the Go it already needs for fklua and without a wasm toolchain, a second
// module or a network fetch.
package main

import (
	"flag"
	"fmt"
	"os"

	"github.com/Techrocket9/fkrecipes/go/internal/modsettings"
)

func main() {
	in := flag.String("in", "", "the JSON settings description to read")
	out := flag.String("out", "", "the mod-settings.dat to write")
	flag.Parse()
	if *in == "" || *out == "" {
		fail("writesettings needs both -in and -out")
	}
	if flag.NArg() != 0 {
		fail("writesettings takes no arguments beyond -in and -out; it was given " + flag.Arg(0))
	}

	data, err := os.ReadFile(*in)
	if err != nil {
		fail(err.Error())
	}
	doc, err := modsettings.ParseJSON(data)
	if err != nil {
		fail(err.Error())
	}
	encoded := modsettings.Encode(doc)

	// READ BACK BEFORE WRITING. The engine's failure mode for a file it cannot
	// parse is a refusal that names the file and not the row, and the gate that
	// runs this is a twenty-second engine run; a round trip here costs
	// microseconds and turns "Factorio would not start" into a sentence naming
	// the byte.
	if _, err := modsettings.Decode(encoded); err != nil {
		fail("the encoded file does not read back: " + err.Error())
	}

	// 0644 rather than 0600: the engine reads this as whatever user the gate
	// runs as, and a private userdir is what keeps it out of anybody's install.
	if err := os.WriteFile(*out, encoded, 0o644); err != nil {
		fail(err.Error())
	}
	fmt.Printf("writesettings: wrote %d bytes to %s\n", len(encoded), *out)
}

func fail(message string) {
	fmt.Fprintln(os.Stderr, "writesettings: "+message)
	os.Exit(1)
}
