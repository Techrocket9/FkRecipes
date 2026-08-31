module github.com/Techrocket9/fkrecipes/go

go 1.24

// The FkLua guest substrate: fkdata (the settings and data stages) and the
// packages it pulls in. Resolved through the real channel, the guest/go/v0.1.0
// tag; no replace directive belongs here.
require github.com/Techrocket9/fklua/guest/go v0.1.0
