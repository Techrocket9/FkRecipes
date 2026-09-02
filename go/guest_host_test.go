package fkrecipes

import "testing"

// THE HOST STUBS ARE WHAT LET A CONSUMER VET THEIR GUEST WITHOUT A BUILD TAG,
// so their message is a contract and this holds them to it.
//
// The pilot had to add -tags tinygo.wasm to its make check because these did
// not exist and the standard toolchain reported Emit undefined on its own data
// guest. That this test COMPILES at all is half the point: it is a host build
// naming all three methods.
func TestEmitPanicsOnTheHost(t *testing.T) {
	cases := []struct {
		name string
		call func(*Lib)
	}{
		{"Emit", func(l *Lib) { l.Emit() }},
		{"EmitSettings", func(l *Lib) { l.EmitSettings() }},
		{"EmitData", func(l *Lib) { l.EmitData() }},
	}
	want := "fkrecipes: Emit runs only inside a wasm guest; this build is for the host"
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			defer func() {
				r := recover()
				if r == nil {
					t.Fatalf("%s returned on the host; it must not be mistaken for a working guest", c.name)
				}
				if got, _ := r.(string); got != want {
					t.Errorf("\n got: %v\nwant: %s", r, want)
				}
			}()
			c.call(New())
		})
	}
}
