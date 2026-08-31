package fkrecipes

import (
	"errors"
	"strconv"
	"strings"
)

// checkCycles walks the technology tree the plan is ABOUT to produce: every
// existing technology's prerequisites, with the splices this plan makes
// applied, plus the technologies it adds.
//
// This is the flagship. The engine's own diagnostic for a cycle is eight
// words with no name, no path and no mod, so the library owns the check and
// names the whole ring.
//
// An arrow points from a technology to one of its prerequisites: "a -> b"
// reads "a requires b".
//
// Every planned name is guaranteed NEW here: validate refuses a plan whose
// prefixed technology name already exists in data.raw. Without that refusal
// the overlay would carry two nodes with one name, the incoming edges would
// bind to the stale one, and the walk would step straight past the ring.
func (l *Lib) checkCycles(w World, res resolution, prefix, stage string) error {
	names := w.TechNames()
	nodes := make([]string, 0, len(names)+len(l.techs))
	lists := make([][]string, 0, len(names)+len(l.techs))
	for _, n := range names {
		nodes = append(nodes, n)
		lists = append(lists, res.currentPrereqs(w, n))
	}
	for i, t := range l.techs {
		nodes = append(nodes, prefix+t.name)
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

	if path, found := walkForCycle(nodes, edges); found {
		return errors.New("fkrecipes: at the " + stage + " stage, a prerequisite cycle: " + renderCyclePath(path))
	}
	return nil
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
