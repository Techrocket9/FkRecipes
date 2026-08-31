package fkrecipes

// Op is one instruction in the plan's output stream. The planner produces the
// whole stream as ordinary values; the emit layer executes it against fkdata,
// in order, and stops at the first refusal fkdata raises.
//
// Three kinds cover everything v1 does: create a prototype, splice a field of
// somebody else's prototype, say out loud that something degraded.
type Op struct {
	Kind  OpKind
	Proto Value    // OpExtend: the whole prototype, one per op
	Path  []PathEl // OpSet: where in data.raw the write lands
	Val   Value    // OpSet: what it writes
	Line  string   // OpLog: the line, already carrying its "fkrecipes: " prefix
}

// OpKind says which arm of Op is live.
type OpKind uint8

// The kinds. Numbering starts at one so a zero Op is not a valid extend.
const (
	OpExtend OpKind = iota + 1
	OpSet
	OpLog
)

// PathEl is one step of an OpSet path. A path is a walk into data.raw, so a
// step is either a key or a sequence index.
type PathEl struct {
	IsNum bool
	Str   string
	Num   float64
}

func pathKey(s string) PathEl { return PathEl{Str: s} }

func extendOp(proto Value) Op { return Op{Kind: OpExtend, Proto: proto} }

func setOp(path []PathEl, val Value) Op { return Op{Kind: OpSet, Path: path, Val: val} }

func logOp(line string) Op { return Op{Kind: OpLog, Line: line} }
