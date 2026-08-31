package fkrecipes

import "testing"

// The pure half runs on the host, which is the property the file layout
// exists to keep: a library whose logic needs a wasm target to test is a
// library whose logic does not get tested.
func TestOnTickCounts(t *testing.T) {
	var l Lib
	if l.OnTick() != 1 || l.OnTick() != 2 {
		t.Fatal("OnTick does not count")
	}
}
