// The Go example guest: a real consumer of this library, packaged by `fklua
// mod` and driven by scripts/run-mirror.sh against its Rust twin.
//
// ITS OWN MODULE, and the replace is the IN-REPO arrangement: the example and
// the library it exercises move together, so the example must see the working
// tree rather than a published tag. A real consumer writes neither the replace
// nor a relative require; they require github.com/Techrocket9/fkrecipes/go at
// a version.
module github.com/Techrocket9/fkrecipes/go/examples/datastage

go 1.24

require github.com/Techrocket9/fkrecipes/go v0.0.0

require github.com/Techrocket9/fklua/guest/go v0.2.0 // indirect

replace github.com/Techrocket9/fkrecipes/go => ../..
