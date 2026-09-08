// Package modsettings reads and writes Factorio's mod-settings.dat.
//
// WHY A LIBRARY REPOSITORY SHIPS A BINARY FORMAT READER. This library's whole
// value is a setting a player edits, and the gate that says a setting works is
// the engine itself. --dump-data runs the settings and data stages against
// whatever mod-settings.dat sits in the mod directory, so the only way to run
// the in-game gate on a FLIPPED row is to write that file. There is no CLI for
// it: the engine writes the file and reads it back, and nothing else does.
//
// IT IS HOST-ONLY AND IT IS internal/. Nothing under here is imported by the
// library, in either build; the pin-transparency rule (fkdata and nothing else)
// is about what a packaged mod carries, and this is a harness tool that happens
// to live in the library module so `go run` can reach it without a second
// module and without a wasm toolchain.
//
// THE LAYOUT IS MEASURED, not read out of a wiki page. agents/customizer-
// design.md's second table is the probe: the install's own 196-byte file was
// decoded by a 40-line reader that consumed exactly 196 of 196 bytes, and the
// engine was then handed an encoded file and read it. What that measurement
// says:
//
//	version   four u16 little-endian, then one u8
//	node      a type byte, then an any-type byte, then the payload
//	1 bool    one byte
//	2 double  eight bytes, little-endian IEEE 754
//	3 string  an empty flag byte; when it is 0, a space-optimised length
//	          (one u8, or 255 followed by a u32 little-endian) and then the
//	          UTF-8 bytes
//	5 dict    a u32 little-endian count, then that many (string key, node)
//	          pairs, the key written as a string payload with no node header
//	6 int     eight bytes, little-endian, signed
//
// The root is a dictionary carrying startup, runtime-global and
// runtime-per-user, each a dictionary of setting name to a dictionary holding
// one key, value.
//
// EVERYTHING HERE IS ORDERED. A Dict is a slice of pairs and never a map,
// because the file this writes is compared byte for byte against a committed
// golden and Go's map iteration is deliberately random. That is the same rule
// the rest of this repository runs on.
package modsettings

import (
	"encoding/binary"
	"errors"
	"math"
	"strconv"
)

// Kind is a property-tree node type, with the engine's own numbering.
type Kind uint8

// The node types this format uses. 0 (none) and 4 (list) exist in the engine's
// property tree and never appear in a mod-settings.dat, so they are not
// implemented: Decode names an unknown type rather than guessing at it.
const (
	KindBool   Kind = 1
	KindNumber Kind = 2
	KindString Kind = 3
	KindDict   Kind = 5
	KindInt    Kind = 6
)

// Value is one property-tree node.
//
// THE ANY-TYPE BYTE IS CARRIED RATHER THAN ASSUMED. It sits on every node in
// the file and the engine writes 0 for all of them, but a reader that dropped
// it could not round trip a file the engine wrote differently, and a round trip
// that quietly normalises is a round trip that proves nothing.
type Value struct {
	Kind    Kind
	AnyType bool

	Bool   bool    // KindBool
	Number float64 // KindNumber
	Str    string  // KindString
	Int    int64   // KindInt
	Dict   []Entry // KindDict, in file order
}

// Entry is one key and node of a dictionary, in the order it appears.
type Entry struct {
	Key   string
	Value Value
}

// Document is a whole file: the version header and the root node.
type Document struct {
	// Version is the four u16 the header opens with, in file order.
	Version [4]uint16
	// Flag is the u8 that closes the header. The engine writes 0.
	Flag byte
	Root Value
}

// Bool, Number, Str and Int build the leaves; Dict builds a node from pairs.
func Bool(b bool) Value      { return Value{Kind: KindBool, Bool: b} }
func Number(f float64) Value { return Value{Kind: KindNumber, Number: f} }
func Str(s string) Value     { return Value{Kind: KindString, Str: s} }
func Int(i int64) Value      { return Value{Kind: KindInt, Int: i} }
func Dict(entries ...Entry) Value {
	return Value{Kind: KindDict, Dict: entries}
}

// Pair is one dictionary entry.
func Pair(key string, v Value) Entry { return Entry{Key: key, Value: v} }

// Encode writes a document. It cannot fail: every Value this package builds is
// writable, and a Value with a Kind this package does not know is written as
// nothing at all rather than as a wrong node, which Decode then names.
func Encode(doc Document) []byte {
	buf := make([]byte, 0, 256)
	var head [2]byte
	for _, v := range doc.Version {
		binary.LittleEndian.PutUint16(head[:], v)
		buf = append(buf, head[0], head[1])
	}
	buf = append(buf, doc.Flag)
	return appendValue(buf, doc.Root)
}

func appendValue(buf []byte, v Value) []byte {
	buf = append(buf, byte(v.Kind))
	buf = append(buf, boolByte(v.AnyType))
	switch v.Kind {
	case KindBool:
		buf = append(buf, boolByte(v.Bool))
	case KindNumber:
		buf = appendU64(buf, math.Float64bits(v.Number))
	case KindString:
		buf = appendString(buf, v.Str)
	case KindInt:
		buf = appendU64(buf, uint64(v.Int))
	case KindDict:
		var n [4]byte
		binary.LittleEndian.PutUint32(n[:], uint32(len(v.Dict)))
		buf = append(buf, n[:]...)
		for _, e := range v.Dict {
			buf = appendString(buf, e.Key)
			buf = appendValue(buf, e.Value)
		}
	}
	return buf
}

// appendString writes the empty flag and, when there is anything to write, the
// space-optimised length and the bytes.
//
// THE EMPTY FLAG IS NOT A LENGTH OF ZERO. An empty string is one byte and
// nothing else: writing a zero length after the flag would push every later
// byte along by one and the engine would read the file as corrupt.
func appendString(buf []byte, s string) []byte {
	if s == "" {
		return append(buf, 1)
	}
	buf = append(buf, 0)
	// The space optimisation: one byte up to 254, and 255 as the escape into a
	// u32. 255 itself therefore takes the long form.
	if len(s) < 255 {
		buf = append(buf, byte(len(s)))
	} else {
		var n [4]byte
		binary.LittleEndian.PutUint32(n[:], uint32(len(s)))
		buf = append(buf, 255)
		buf = append(buf, n[:]...)
	}
	return append(buf, s...)
}

func appendU64(buf []byte, u uint64) []byte {
	var n [8]byte
	binary.LittleEndian.PutUint64(n[:], u)
	return append(buf, n[:]...)
}

func boolByte(b bool) byte {
	if b {
		return 1
	}
	return 0
}

// Decode reads a whole file and REFUSES anything it does not understand: a
// short read, an unknown node type, a length that runs off the end, and bytes
// left over at the end.
//
// THE TRAILING-BYTES CHECK IS THE ONE THAT CAUGHT THE FORMAT. The probe that
// established this layout is "the reader consumed exactly 196 of 196 bytes";
// a reader that stops early is a reader that has misread a length somewhere
// upstream and would otherwise report success on a file it does not
// understand.
func Decode(b []byte) (Document, error) {
	d := &reader{buf: b}
	var doc Document
	for i := range doc.Version {
		v, err := d.u16()
		if err != nil {
			return Document{}, err
		}
		doc.Version[i] = v
	}
	flag, err := d.byteAt()
	if err != nil {
		return Document{}, err
	}
	doc.Flag = flag
	root, err := d.value(0)
	if err != nil {
		return Document{}, err
	}
	doc.Root = root
	if d.at != len(d.buf) {
		return Document{}, errors.New("modsettings: " + strconv.Itoa(len(d.buf)-d.at) +
			" bytes are left over after the root node; the file is not the shape this reader knows")
	}
	return doc, nil
}

// maxDepth stops a hand-edited or corrupt file from turning a cycle of
// dictionaries into a stack overflow. A real file is three deep.
const maxDepth = 32

type reader struct {
	buf []byte
	at  int
}

func (r *reader) short(what string, n int) error {
	return errors.New("modsettings: the file ends in the middle of " + what +
		"; " + strconv.Itoa(n) + " bytes were wanted at offset " + strconv.Itoa(r.at) +
		" and " + strconv.Itoa(len(r.buf)-r.at) + " are left")
}

func (r *reader) take(what string, n int) ([]byte, error) {
	if n < 0 || len(r.buf)-r.at < n {
		return nil, r.short(what, n)
	}
	out := r.buf[r.at : r.at+n]
	r.at += n
	return out, nil
}

func (r *reader) byteAt() (byte, error) {
	b, err := r.take("a byte", 1)
	if err != nil {
		return 0, err
	}
	return b[0], nil
}

func (r *reader) u16() (uint16, error) {
	b, err := r.take("a 16-bit number", 2)
	if err != nil {
		return 0, err
	}
	return binary.LittleEndian.Uint16(b), nil
}

func (r *reader) u32() (uint32, error) {
	b, err := r.take("a 32-bit number", 4)
	if err != nil {
		return 0, err
	}
	return binary.LittleEndian.Uint32(b), nil
}

func (r *reader) u64() (uint64, error) {
	b, err := r.take("a 64-bit number", 8)
	if err != nil {
		return 0, err
	}
	return binary.LittleEndian.Uint64(b), nil
}

func (r *reader) str() (string, error) {
	empty, err := r.byteAt()
	if err != nil {
		return "", err
	}
	if empty != 0 {
		return "", nil
	}
	n, err := r.byteAt()
	if err != nil {
		return "", err
	}
	length := int(n)
	if n == 255 {
		long, err := r.u32()
		if err != nil {
			return "", err
		}
		length = int(long)
		if length < 0 {
			return "", errors.New("modsettings: a string claims a length no reader can hold")
		}
	}
	b, err := r.take("a string of "+strconv.Itoa(length)+" bytes", length)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

func (r *reader) value(depth int) (Value, error) {
	if depth > maxDepth {
		return Value{}, errors.New("modsettings: the property tree nests deeper than " +
			strconv.Itoa(maxDepth) + " levels; a mod-settings.dat is three deep")
	}
	kind, err := r.byteAt()
	if err != nil {
		return Value{}, err
	}
	anyType, err := r.byteAt()
	if err != nil {
		return Value{}, err
	}
	v := Value{Kind: Kind(kind), AnyType: anyType != 0}
	switch v.Kind {
	case KindBool:
		b, err := r.byteAt()
		if err != nil {
			return Value{}, err
		}
		v.Bool = b != 0
	case KindNumber:
		u, err := r.u64()
		if err != nil {
			return Value{}, err
		}
		v.Number = math.Float64frombits(u)
	case KindString:
		s, err := r.str()
		if err != nil {
			return Value{}, err
		}
		v.Str = s
	case KindInt:
		u, err := r.u64()
		if err != nil {
			return Value{}, err
		}
		v.Int = int64(u)
	case KindDict:
		n, err := r.u32()
		if err != nil {
			return Value{}, err
		}
		// The count is read before the entries and a corrupt one could claim
		// four billion, so nothing is preallocated from it: each entry is
		// appended as it is actually read and a short file runs out first.
		for i := uint32(0); i < n; i++ {
			key, err := r.str()
			if err != nil {
				return Value{}, err
			}
			child, err := r.value(depth + 1)
			if err != nil {
				return Value{}, err
			}
			v.Dict = append(v.Dict, Entry{Key: key, Value: child})
		}
	default:
		return Value{}, errors.New("modsettings: node type " + strconv.Itoa(int(kind)) +
			" at offset " + strconv.Itoa(r.at-2) + " is not one this reader knows (1 bool, 2 double, 3 string, 5 dictionary, 6 signed 64-bit)")
	}
	return v, nil
}
