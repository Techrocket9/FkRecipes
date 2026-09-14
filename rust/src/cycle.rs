use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::data::{
    cycle_prereq_line, cycle_prereq_note, cycle_splice_line, cycle_splice_note, NoteTarget,
    Resolution,
};
use crate::plan::Lib;
use crate::world::World;

impl Lib {
    /// RESOLVES the technology tree the plan is ABOUT to produce: every
    /// existing technology's prerequisites, with the splices this plan makes
    /// applied, plus the technologies it adds. Where a ring closes through an
    /// edge THIS PLAN MADE, the edge is dropped with a line and a tooltip;
    /// where it closes without one, the load stops.
    ///
    /// This is the flagship. The engine's own diagnostic for a cycle is eight
    /// words with no name, no path and no mod, so the library owns the check
    /// and names the whole ring.
    ///
    /// WHY IT RESOLVES RATHER THAN REFUSES. A ring this plan closed is the mod
    /// set's doing as often as the author's: another mod that adds a
    /// prerequisite to the technology this plan splices into can close a loop
    /// through a tree neither mod could see. Refusing stops the load with an
    /// "Error loading mods" dialog the client cannot reach the Mod Settings
    /// screen from (measured on 2.0.77: see `Resolution::fallback_fact`), so a
    /// modpack the player did not assemble locks them out of a game this
    /// library could have loaded. Dropping ONE edge this plan made leaves the
    /// game playable, leaves every other mod's tree alone, and says in the
    /// technology's own tooltip what it lost.
    ///
    /// AND IT STILL REFUSES A RING WITH NO EDGE OF OURS IN IT. That is the
    /// game's own technology tree looping without this mod in it, which the
    /// engine refuses on its own and which the threat model puts OUT of scope
    /// (I, a pack that breaks the base game itself). There is no edge of ours
    /// to take back, so the only alternative to the sentence would be rewriting
    /// somebody else's tree.
    ///
    /// TERMINATION IS BY CONSTRUCTION. Every pass either drops one edge this
    /// plan made, and the plan has finitely many, or refuses and returns. A
    /// pass that finds a ring it owns no edge in refuses rather than walking
    /// again, which is the guard against a ring the drop cannot touch.
    ///
    /// An arrow points from a technology to one of its prerequisites:
    /// "a -> b" reads "a requires b".
    ///
    /// Every planned name is guaranteed NEW here: validate refuses a plan
    /// whose prefixed technology name already exists in data.raw. Without
    /// that refusal the overlay would carry two nodes with one name, the
    /// incoming edges would bind to the stale one, and the walk would step
    /// straight past the ring.
    pub(crate) fn check_cycles(
        &self,
        w: &dyn World,
        res: &mut Resolution,
        prefix: &str,
    ) -> Result<(), String> {
        let names = w.tech_names();

        // THE WORLD IS ASKED ONCE AND THE PLAN IS ASKED EVERY PASS, because
        // only one of the two moves. `current_prereqs` is two halves: the last
        // non-dropped rewrite record for a technology, which changes on every
        // pass because that is what a pass does, and the game's own list, which
        // cannot: no code in this crate writes to data.raw between passes, and
        // a World that answered `tech_prereqs` differently on a second call
        // would already have made the first pass's overlay a fiction.
        //
        // IT IS A HOST CALL AND NOT A MAP LOOKUP, which is why it is worth
        // hoisting. Measured by the adversarial review on a 275-technology
        // world: 275 calls across the wasm boundary became 14,025 at 50 plan
        // technologies and 75,900 at 400. The graph each pass computes is
        // unchanged.
        let world_prereqs: Vec<Vec<String>> = names.iter().map(|n| w.tech_prereqs(n)).collect();

        loop {
            let mut nodes: Vec<String> = Vec::with_capacity(names.len() + self.techs.len());
            let mut lists: Vec<Vec<String>> = Vec::with_capacity(names.len() + self.techs.len());
            for (i, n) in names.iter().enumerate() {
                // Cloned for the reason the plan's own lists are: the overlay
                // must not alias anything it was built from, and the World's
                // answer is now shared across passes rather than fetched fresh.
                lists.push(
                    res.planned_prereqs(n)
                        .unwrap_or_else(|| world_prereqs[i].clone()),
                );
                nodes.push(n.clone());
            }
            for (i, t) in self.techs.iter().enumerate() {
                nodes.push(t.emitted_name(prefix));
                lists.push(res.techs[i].prereqs.clone());
            }

            // Edges by index, resolved with a linear scan: a hash map would be
            // faster and would also be the one thing this crate may not do. The
            // base game is 275 technologies and 479 edges, measured affordable
            // at load.
            let mut edges: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
            for (i, list) in lists.iter().enumerate() {
                for name in list {
                    // A prerequisite naming something the game does not have is
                    // the engine's own hard failure, not a cycle; it has no
                    // outgoing edges here and cannot close a ring.
                    if let Some(j) = node_index(&nodes, name) {
                        edges[i].push(j);
                    }
                }
            }

            let Some(path) = walk_for_cycle(&nodes, &edges) else {
                return Ok(());
            };
            if self.drop_owned_edge(res, prefix, &path) {
                continue;
            }
            return Err(format!(
                "fkrecipes: a prerequisite cycle: {}",
                render_cycle_path(&path)
            ));
        }
    }

    /// Takes back the FIRST edge in the ring's OWN ORDER that this plan made,
    /// and answers whether it found one.
    ///
    /// THE RING'S ORDER AND NOT THE PLAN'S, because the ring is what the two
    /// languages both have in front of them: `walk_for_cycle` starts in the
    /// World's sorted technology order and then in plan declaration order, so
    /// both halves are handed the same path and both drop the same edge. A rule
    /// that picked the earliest DECLARED technology instead would need each
    /// half to agree about a second ordering it did not compute.
    ///
    /// AN EDGE THIS PLAN OWNS IS ONE OF EXACTLY TWO THINGS. One out of a
    /// technology this plan emits, which is an entry in its own resolved
    /// prerequisite list; or a SPLICE, which is this plan's own emitted name
    /// sitting inside another technology's rewritten list. Nothing else in the
    /// overlay came from here: the rest is the game's own tree.
    fn drop_owned_edge(&self, res: &mut Resolution, prefix: &str, path: &[String]) -> bool {
        for k in 0..path.len().saturating_sub(1) {
            let (from, to) = (path[k].as_str(), path[k + 1].as_str());
            if let Some(i) = self.plan_index(prefix, from) {
                let Some(at) = index_of_name(&res.techs[i].prereqs, to) else {
                    continue;
                };
                res.techs[i].prereqs.remove(at);
                res.logs
                    .push(cycle_prereq_line(&self.techs[i].name, to, path));
                res.note_on(
                    NoteTarget {
                        tech: true,
                        index: i,
                    },
                    cycle_prereq_note(to),
                );
                return true;
            }
            // A SPLICE, WHICH IS AN EDGE OUT OF SOMEBODY ELSE'S TECHNOLOGY. The
            // node it points AT is what says whose it is: only a splice puts a
            // name this plan issued into another technology's prerequisite
            // list.
            let Some(i) = self.plan_index(prefix, to) else {
                continue;
            };
            if res.techs[i].rewrite == 0 {
                continue;
            }
            let at = res.techs[i].rewrite - 1;
            if res.rewrites[at].before.as_str() != from || res.rewrites[at].dropped {
                continue;
            }
            drop_splice(res, at, from, to);
            res.logs
                .push(cycle_splice_line(&self.techs[i].name, from, path));
            res.note_on(
                NoteTarget {
                    tech: true,
                    index: i,
                },
                cycle_splice_note(from),
            );
            return true;
        }
        false
    }

    /// The declaration index of the plan technology a node name belongs to, or
    /// `None` for a name the game owns. Planned names are guaranteed new, so a
    /// hit here is never another mod's technology: see `check_cycles`.
    fn plan_index(&self, prefix: &str, name: &str) -> Option<usize> {
        self.techs
            .iter()
            .position(|t| t.emitted_name(prefix) == name)
    }
}

fn index_of_name(list: &[String], name: &str) -> Option<usize> {
    list.iter().position(|have| have.as_str() == name)
}

/// Takes one planned splice of `name` into `before` back out of every record
/// that carries it, and marks the record that MADE it as dropped.
///
/// IT PUTS THE ANCHOR BACK, and that is the whole difference between undoing a
/// splice and vandalising somebody else's technology tree. A splice REPLACES
/// the technology it was inserted after, so a SECOND splice into the same
/// target built its record on a list the anchor was already out of. Deleting
/// the name from that later record would emit a prerequisite list with a
/// base-game edge silently gone: measured on a plan where two technologies both
/// name Before logistics-3, the surviving record read `[mod-bronze-axes]` and
/// logistics-2 had vanished from a technology this plan does not own. What the
/// anchor restores is exactly the list a plan without this splice would have
/// produced. An APPENDED splice replaced nothing, carries no anchor, and is
/// deleted.
///
/// BOTH HALVES ARE LOAD-BEARING. A second splice into the same technology
/// builds on the first, so the name is in both records and purging only one
/// would leave the edge standing in whichever record emit writes last; and the
/// record that made the splice is marked rather than emptied and written,
/// because what is left in it is another mod's own prerequisite list and
/// Setting that back over its prototype is a write this library has no reason
/// to make.
///
/// MARKED RATHER THAN REMOVED, because `ResolvedTech::rewrite` is a 1-based
/// index into this vector and renumbering it would point every later technology
/// at somebody else's record.
fn drop_splice(res: &mut Resolution, at: usize, before: &str, name: &str) {
    res.rewrites[at].dropped = true;
    let anchor = res.rewrites[at].anchor.clone();
    for rw in res.rewrites.iter_mut() {
        if rw.before.as_str() != before {
            continue;
        }
        let Some(k) = index_of_name(&rw.list, name) else {
            continue;
        };
        if anchor.is_empty() {
            rw.list.remove(k);
            continue;
        }
        rw.list[k] = anchor.clone();
    }
}

fn node_index(nodes: &[String], name: &str) -> Option<usize> {
    for (i, n) in nodes.iter().enumerate() {
        if n.as_str() == name {
            return Some(i);
        }
    }
    None
}

const NODE_UNSEEN: u8 = 0;
const NODE_ON_STACK: u8 = 1;
const NODE_DONE: u8 = 2;

/// A depth-first search with an explicit stack: the same order a recursive
/// walk would take, without trusting either language's stack depth. Start
/// nodes come in the World's sorted technology order and then in plan
/// declaration order, so two runs, and the two languages, report the SAME
/// ring.
fn walk_for_cycle(nodes: &[String], edges: &[Vec<usize>]) -> Option<Vec<String>> {
    let mut state = vec![NODE_UNSEEN; nodes.len()];
    for start in 0..nodes.len() {
        if state[start] != NODE_UNSEEN {
            continue;
        }
        state[start] = NODE_ON_STACK;
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some(&(node, edge)) = stack.last() {
            if edge >= edges[node].len() {
                state[node] = NODE_DONE;
                stack.pop();
                continue;
            }
            let last = stack.len() - 1;
            stack[last].1 += 1;
            let next = edges[node][edge];
            if state[next] == NODE_ON_STACK {
                let mut at = 0;
                for (i, frame) in stack.iter().enumerate() {
                    if frame.0 == next {
                        at = i;
                        break;
                    }
                }
                let mut path = Vec::with_capacity(stack.len() - at + 1);
                for frame in &stack[at..] {
                    path.push(nodes[frame.0].clone());
                }
                path.push(nodes[next].clone());
                return Some(path);
            }
            if state[next] == NODE_UNSEEN {
                state[next] = NODE_ON_STACK;
                stack.push((next, 0));
            }
        }
    }
    None
}

/// How many technologies a refusal names before it stops. A pathological tree
/// can ring thousands of technologies together, and a megabyte of names in
/// one message helps nobody read the first hop.
pub(crate) const CYCLE_PATH_CAP: usize = 100;

pub(crate) fn render_cycle_path(path: &[String]) -> String {
    if path.len() <= CYCLE_PATH_CAP {
        return join_path(path);
    }
    let rest = path.len() - CYCLE_PATH_CAP;
    format!(
        "{} -> (and {} more before it closes)",
        join_path(&path[..CYCLE_PATH_CAP]),
        rest
    )
}

fn join_path(path: &[String]) -> String {
    let mut out = String::new();
    for (i, n) in path.iter().enumerate() {
        if i > 0 {
            out.push_str(" -> ");
        }
        out.push_str(n);
    }
    out
}
