package fkrecipes

import (
	"math"
	"strconv"
	"strings"
	"testing"
)

// The transcript is how a test reads an Op stream: one line per op, values
// rendered in the order the planner built them. The Rust mirror renders the
// same lines from the same plan, which is what "the two halves agree" means
// before the packaged mirror harness exists to say it in Lua.

func transcript(ops []Op) []string {
	lines := make([]string, 0, len(ops))
	for _, op := range ops {
		switch op.Kind {
		case OpExtend:
			lines = append(lines, "extend "+renderValue(op.Proto))
		case OpSet:
			lines = append(lines, "set "+renderPath(op.Path)+" = "+renderValue(op.Val))
		case OpLog:
			lines = append(lines, "log "+op.Line)
		default:
			lines = append(lines, "?")
		}
	}
	return lines
}

func renderPath(path []PathEl) string {
	var b strings.Builder
	for i, el := range path {
		if el.IsNum {
			b.WriteString("[" + formatNum(el.Num) + "]")
			continue
		}
		if i > 0 {
			b.WriteString(".")
		}
		b.WriteString(el.Str)
	}
	return b.String()
}

func renderValue(v Value) string {
	switch v.Kind {
	case KindNil:
		return "nil"
	case KindBool:
		if v.Bool {
			return "true"
		}
		return "false"
	case KindNum:
		return formatNum(v.Num)
	case KindStr:
		return `"` + v.Str + `"`
	case KindArr:
		parts := make([]string, 0, len(v.Arr))
		for _, item := range v.Arr {
			parts = append(parts, renderValue(item))
		}
		return "[" + strings.Join(parts, ", ") + "]"
	default:
		parts := make([]string, 0, len(v.Map))
		for _, p := range v.Map {
			parts = append(parts, p.Key+"="+renderValue(p.Val))
		}
		return "{" + strings.Join(parts, ", ") + "}"
	}
}

// formatNum is the ONE rendering rule the two halves share. Shortest-round-trip
// printing was the obvious choice and it was wrong: Go's strconv and Rust's
// Display break ties differently, so real values print as
// 3109032256237.6562 on one side and 3109032256237.6563 on the other, and the
// transcripts they are compared through diverge on numbers neither planner
// rejected. This rule has no ties to break.
//
//   - NaN and the infinities keep the Go spellings.
//   - An integral value the double holds exactly prints as plain digits.
//   - Everything else prints at seventeen significant digits in scientific
//     form. Both languages round the same way at a fixed precision, and the
//     exponent is normalised to Rust's shape: no plus, no leading zeros.
func formatNum(n float64) string {
	if math.IsNaN(n) {
		return "NaN"
	}
	if math.IsInf(n, 1) {
		return "+Inf"
	}
	if math.IsInf(n, -1) {
		return "-Inf"
	}
	if n == math.Trunc(n) && math.Abs(n) <= float64(maxExactInt) {
		return strconv.FormatInt(int64(n), 10)
	}
	s := strconv.FormatFloat(n, 'e', 16, 64)
	at := strings.IndexByte(s, 'e')
	mantissa, exponent := s[:at], s[at+1:]
	sign := ""
	if exponent[0] == '+' {
		exponent = exponent[1:]
	} else if exponent[0] == '-' {
		sign = "-"
		exponent = exponent[1:]
	}
	exponent = strings.TrimLeft(exponent, "0")
	if exponent == "" {
		exponent = "0"
	}
	return mantissa + "e" + sign + exponent
}

func assertLines(t *testing.T, got, want []string) {
	t.Helper()
	for i := 0; i < len(got) && i < len(want); i++ {
		if got[i] != want[i] {
			t.Errorf("line %d\n got: %s\nwant: %s", i, got[i], want[i])
		}
	}
	if len(got) != len(want) {
		t.Errorf("got %d lines, want %d", len(got), len(want))
		for i := len(want); i < len(got); i++ {
			t.Errorf("extra line %d: %s", i, got[i])
		}
		for i := len(got); i < len(want); i++ {
			t.Errorf("missing line %d: %s", i, want[i])
		}
	}
}

func assertNoError(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatalf("plan refused: %s", err)
	}
}

// The two halves must render a number the same way or the transcripts they
// are compared through diverge on a value neither planner rejected. The Rust
// mirror runs this table verbatim; the two must come out byte for byte alike.
func TestFormatNumMatchesTheRustMirror(t *testing.T) {
	cases := []struct {
		in   float64
		want string
	}{
		// Integers, including the negative zero the integer path flattens.
		{0, "0"},
		{math.Copysign(0, -1), "0"},
		{2, "2"},
		{-30, "-30"},
		{1e15, "1000000000000000"},
		// The exact-integer boundary the validator blesses, and the first
		// value past it, which no longer holds exactly.
		{9007199254740991, "9007199254740991"},
		{9007199254740992, "9007199254740992"},
		{-9007199254740992, "-9007199254740992"},
		{9007199254740994, "9.0071992547409940e15"},
		// Fractions, and the samples a shortest-round-trip renderer split
		// between the two languages.
		{2.5, "2.5000000000000000e0"},
		{-0.75, "-7.5000000000000000e-1"},
		{3109032256237.6562, "3.1090322562376562e12"},
		{2766224557440.7812, "2.7662245574407812e12"},
		{-791902786.7695312, "-7.9190278676953125e8"},
		// Small magnitudes, where the exponent carries its own sign.
		{1e-7, "9.9999999999999995e-8"},
		{2.5e-10, "2.5000000000000002e-10"},
		{5e-324, "4.9406564584124654e-324"},
		{1.7976931348623157e308, "1.7976931348623157e308"},
		{2.2250738585072014e-308, "2.2250738585072014e-308"},
		{0.1, "1.0000000000000001e-1"},
		{6.02214076e23, "6.0221407599999999e23"},
		// The non-finite values, which the planner refuses long before an op
		// carries one; this pins the spelling anyway.
		{math.Inf(1), "+Inf"},
		{math.Inf(-1), "-Inf"},
		{math.NaN(), "NaN"},
	}
	for _, c := range cases {
		if got := formatNum(c.in); got != c.want {
			t.Errorf("formatNum(%v) = %q, want %q", c.in, got, c.want)
		}
	}
}
