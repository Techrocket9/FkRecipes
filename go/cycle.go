package fkrecipes

import (
	"errors"
	"strconv"
	"strings"
)

// checkCycles RESOLVES the technology tree the plan is ABOUT to produce: every
// existing technology's prerequisites, with the splices this plan makes
// applied, plus the technologies it adds. Where a ring closes through an edge
// THIS PLAN MADE, the edge is dropped with a line and a tooltip; where it
// closes without one, the load stops.
//
// This is the flagship. The engine's own diagnostic for a cycle is eight
// words with no name, no path and no mod, so the library owns the check and
// names the whole ring.
//
// WHY IT RESOLVES RATHER THAN REFUSES. A ring this plan closed is the mod set's
// doing as often as the author's: another mod that adds a prerequisite to the
// technology this plan splices into can close a loop through a tree neither mod
// could see. Refusing stops the load with an "Error loading mods" dialog the
// client cannot reach the Mod Settings screen from (measured on 2.0.77: see
// fallbackFact), so a modpack the player did not assemble locks them out of a
// game this library could have loaded. Dropping ONE edge this plan made leaves
// the game playable, leaves every other mod's tree alone, and says in the
// technology's own tooltip what it lost.
//
// AND IT STILL REFUSES A RING WITH NO EDGE OF OURS IN IT. That is the game's
// own technology tree looping without this mod in it, which the engine refuses
// on its own and which the threat model puts OUT of scope (I, a pack that
// breaks the base game itself). There is no edge of ours to take back, so the
// only alternative to the sentence would be rewriting somebody else's tree.
//
// TERMINATION IS BY CONSTRUCTION. Every pass either drops one edge this plan
// made, and the plan has finitely many, or refuses and returns. A pass that
// finds a ring it owns no edge in refuses rather than walking again, which is
// the guard against a ring the drop cannot touch.
//
// An arrow points from a technology to one of its prerequisites: "a -> b"
// reads "a requires b".
//
// Every planned name is guaranteed NEW here: validate refuses a plan whose
// prefixed technology name already exists in data.raw. Without that refusal
// the overlay would carry two nodes with one name, the incoming edges would
// bind to the stale one, and the walk would step straight past the ring.
func (l *Lib) checkCycles(w World, res *resolution, prefix string) error {
	names := w.TechNames()

	// THE WORLD IS ASKED ONCE AND THE PLAN IS ASKED EVERY PASS, because only
	// one of the two moves. currentPrereqs is two halves: the last non-dropped
	// rewrite record for a technology, which changes on every pass because that
	// is what a pass does, and the game's own list, which cannot: no code in
	// this library writes to data.raw between passes, and a World that answered
	// TechPrereqs differently on a second call would already have made the
	// first pass's overlay a fiction.
	//
	// IT IS A HOST CALL AND NOT A MAP LOOKUP, which is why it is worth
	// hoisting. Measured by the adversarial review on a 275-technology world:
	// 275 calls across the wasm boundary became 14,025 at 50 plan technologies
	// and 75,900 at 400. The graph each pass computes is unchanged.
	worldPrereqs := make([][]string, len(names))
	for i, n := range names {
		worldPrereqs[i] = w.TechPrereqs(n)
	}

	for {
		nodes := make([]string, 0, len(names)+len(l.techs))
		lists := make([][]string, 0, len(names)+len(l.techs))
		for i, n := range names {
			nodes = append(nodes, n)
			list, planned := res.plannedPrereqs(n)
			if !planned {
				// Cloned for the reason the plan's own lists are: the overlay
				// must not alias anything it was built from, and the World's
				// answer is now shared across passes rather than fetched fresh.
				list = copyStrings(worldPrereqs[i])
			}
			lists = append(lists, list)
		}
		for i, t := range l.techs {
			nodes = append(nodes, t.emittedName(prefix))
			// Cloned because the Rust mirror clones: the overlay must not alias
			// the resolution it was built from.
			lists = append(lists, copyStrings(res.techs[i].prereqs))
		}

		// Edges by index, resolved with a linear scan: a map would be faster and
		// would also be the one thing this package may not do. The base game is
		// 275 technologies and 479 edges, measured affordable at load.
		edges := make([][]int, len(nodes))
		for i, list := range lists {
			for _, name := range list {
				// A prerequisite naming something the game does not have is the
				// engine's own hard failure, not a cycle; it has no outgoing
				// edges here and cannot close a ring.
				if j := nodeIndex(nodes, name); j >= 0 {
					edges[i] = append(edges[i], j)
				}
			}
		}

		path, found := walkForCycle(nodes, edges)
		if !found {
			return nil
		}
		if l.dropOwnedEdge(res, prefix, path) {
			continue
		}
		return errors.New("fkrecipes: a prerequisite cycle: " + renderCyclePath(path))
	}
}

// dropOwnedEdge takes back the FIRST edge in the ring's OWN ORDER that this
// plan made, and answers whether it found one.
//
// THE RING'S ORDER AND NOT THE PLAN'S, because the ring is what the two
// languages both have in front of them: walkForCycle starts in the World's
// sorted technology order and then in plan declaration order, so both halves
// are handed the same path and both drop the same edge. A rule that picked the
// earliest DECLARED technology instead would need each half to agree about a
// second ordering it did not compute.
//
// AN EDGE THIS PLAN OWNS IS ONE OF EXACTLY TWO THINGS. One out of a technology
// this plan emits, which is an entry in its own resolved prerequisite list; or
// a SPLICE, which is this plan's own emitted name sitting inside another
// technology's rewritten list. Nothing else in the overlay came from here: the
// rest is the game's own tree.
func (l *Lib) dropOwnedEdge(res *resolution, prefix string, path []string) bool {
	for k := 0; k+1 < len(path); k++ {
		from, to := path[k], path[k+1]
		if i := l.planIndex(prefix, from); i >= 0 {
			at := indexOfName(res.techs[i].prereqs, to)
			if at < 0 {
				continue
			}
			res.techs[i].prereqs = append(res.techs[i].prereqs[:at], res.techs[i].prereqs[at+1:]...)
			res.logs = append(res.logs, cyclePrereqLine(l.techs[i].name, to, path))
			res.noteOn(noteTarget{tech: true, index: i}, cyclePrereqNote(to))
			return true
		}
		// A SPLICE, WHICH IS AN EDGE OUT OF SOMEBODY ELSE'S TECHNOLOGY. The
		// node it points AT is what says whose it is: only a splice puts a name
		// this plan issued into another technology's prerequisite list.
		i := l.planIndex(prefix, to)
		if i < 0 {
			continue
		}
		at := res.techs[i].rewrite - 1
		if at < 0 || res.rewrites[at].before != from || res.rewrites[at].dropped {
			continue
		}
		res.dropSplice(at, from, to)
		res.logs = append(res.logs, cycleSpliceLine(l.techs[i].name, from, path))
		res.noteOn(noteTarget{tech: true, index: i}, cycleSpliceNote(from))
		return true
	}
	return false
}

// planIndex is the declaration index of the plan technology a node name
// belongs to, or -1 for a name the game owns. Planned names are guaranteed new,
// so a hit here is never another mod's technology: see checkCycles.
func (l *Lib) planIndex(prefix, name string) int {
	for i, t := range l.techs {
		if t.emittedName(prefix) == name {
			return i
		}
	}
	return -1
}

func indexOfName(list []string, name string) int {
	for i, have := range list {
		if have == name {
			return i
		}
	}
	return -1
}

// dropSplice takes one planned splice of `name` into `before` back out of every
// record that carries it, and marks the record that MADE it as dropped.
//
// IT PUTS THE ANCHOR BACK, and that is the whole difference between undoing a
// splice and vandalising somebody else's technology tree. A splice REPLACES the
// technology it was inserted after, so a SECOND splice into the same target
// built its record on a list the anchor was already out of. Deleting the name
// from that later record would emit a prerequisite list with a base-game edge
// silently gone: measured on a plan where two technologies both name Before
// logistics-3, the surviving record read [mod-bronze-axes] and logistics-2 had
// vanished from a technology this plan does not own. What the anchor restores
// is exactly the list a plan without this splice would have produced. An
// APPENDED splice replaced nothing, carries no anchor, and is deleted.
//
// BOTH HALVES ARE LOAD-BEARING. A second splice into the same technology builds
// on the first, so the name is in both records and purging only one would leave
// the edge standing in whichever record emit writes last; and the record that
// made the splice is marked rather than emptied and written, because what is
// left in it is another mod's own prerequisite list and Setting that back over
// its prototype is a write this library has no reason to make.
//
// MARKED RATHER THAN REMOVED, because resolvedTech.rewrite is a 1-based index
// into this slice and renumbering it would point every later technology at
// somebody else's record.
func (r *resolution) dropSplice(at int, before, name string) {
	r.rewrites[at].dropped = true
	anchor := r.rewrites[at].anchor
	for i := range r.rewrites {
		if r.rewrites[i].before != before {
			continue
		}
		k := indexOfName(r.rewrites[i].list, name)
		if k < 0 {
			continue
		}
		if anchor == "" {
			r.rewrites[i].list = append(r.rewrites[i].list[:k], r.rewrites[i].list[k+1:]...)
			continue
		}
		r.rewrites[i].list[k] = anchor
	}
}

// The two lines and the two notes a dropped edge earns. They are ERROR lines
// because the technology is not placed the way anybody declared it, and they
// are NOT a player's fallback: nothing was stored and nothing was typed, so
// there is no field on the settings screen to send anybody to.
//
// THE WHOLE RING IS IN THE LINE AND NOT IN THE NOTE. An author reading the log
// needs the path to see which mod closed it; a player hovering a technology
// cannot act on a hundred prototype names, and the ceiling every composition is
// held to (200 bytes per element, measured on 2.0.77) is a further reason not
// to put one there.
func cyclePrereqLine(tech, name string, path []string) string {
	return messagePrefix + "ERROR: " + tech + ": requiring " + name +
		" would loop this game's technology tree (" + renderCyclePath(path) +
		"), so the prerequisite is dropped"
}

func cyclePrereqNote(name string) string {
	return "Requiring " + name + " would loop this game's technology tree," +
		" so this research was left without that prerequisite. The reason is in the log."
}

func cycleSpliceLine(tech, before string, path []string) string {
	return messagePrefix + "ERROR: " + tech + ": making it a prerequisite of " + before +
		" would loop this game's technology tree (" + renderCyclePath(path) +
		"), so the splice is dropped"
}

func cycleSpliceNote(before string) string {
	return "Making this research a prerequisite of " + before +
		" would loop this game's technology tree, so it was left out of it. The reason is in the log."
}

// cyclePathCap is how many technologies a refusal names before it stops. A
// pathological tree can ring thousands of technologies together, and a
// megabyte of names in one message helps nobody read the first hop.
const cyclePathCap = 100

func renderCyclePath(path []string) string {
	if len(path) <= cyclePathCap {
		return strings.Join(path, " -> ")
	}
	rest := len(path) - cyclePathCap
	return strings.Join(path[:cyclePathCap], " -> ") + " -> (and " + strconv.Itoa(rest) + " more before it closes)"
}

func nodeIndex(nodes []string, name string) int {
	for i, n := range nodes {
		if n == name {
			return i
		}
	}
	return -1
}

const (
	nodeUnseen uint8 = iota
	nodeOnStack
	nodeDone
)

// walkForCycle is a depth-first search with an explicit stack: the same order
// a recursive walk would take, without trusting either language's stack depth.
// Start nodes come in the World's sorted technology order and then in plan
// declaration order, so two runs, and the two languages, report the SAME ring.
func walkForCycle(nodes []string, edges [][]int) ([]string, bool) {
	type frame struct {
		node int
		next int
	}
	state := make([]uint8, len(nodes))
	for start := range nodes {
		if state[start] != nodeUnseen {
			continue
		}
		state[start] = nodeOnStack
		stack := []frame{{node: start}}
		for len(stack) > 0 {
			top := &stack[len(stack)-1]
			if top.next >= len(edges[top.node]) {
				state[top.node] = nodeDone
				stack = stack[:len(stack)-1]
				continue
			}
			next := edges[top.node][top.next]
			top.next++
			if state[next] == nodeOnStack {
				at := 0
				for i, f := range stack {
					if f.node == next {
						at = i
						break
					}
				}
				path := make([]string, 0, len(stack)-at+1)
				for _, f := range stack[at:] {
					path = append(path, nodes[f.node])
				}
				path = append(path, nodes[next])
				return path, true
			}
			if state[next] == nodeUnseen {
				state[next] = nodeOnStack
				stack = append(stack, frame{node: next})
			}
		}
	}
	return nil, false
}
