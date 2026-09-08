// The size-measurement guest: BetterBeltBalancer's pre-customizer plan shape,
// with no text setting. See main.go. Its own module for the reason the
// datastage example is one: it is a consumer-shaped project, and the replace
// is the in-repo arrangement so it sees the working tree.
module github.com/Techrocket9/fkrecipes/go/examples/notext

go 1.24

require github.com/Techrocket9/fkrecipes/go v0.0.0

require github.com/Techrocket9/fklua/guest/go v0.2.0 // indirect

replace github.com/Techrocket9/fkrecipes/go => ../..
