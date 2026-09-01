use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::data::Resolution;
use crate::plan::Lib;
use crate::world::World;

impl Lib {
    /// Walks the technology tree the plan is ABOUT to produce: every existing
    /// technology's prerequisites, with the splices this plan makes applied,
    /// plus the technologies it adds.
    ///
    /// This is the flagship. The engine's own diagnostic for a cycle is eight
    /// words with no name, no path and no mod, so the library owns the check
    /// and names the whole ring.
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
        res: &Resolution,
        prefix: &str,
    ) -> Result<(), String> {
        let names = w.tech_names();
        let mut nodes: Vec<String> = Vec::with_capacity(names.len() + self.techs.len());
        let mut lists: Vec<Vec<String>> = Vec::with_capacity(names.len() + self.techs.len());
        for n in &names {
            lists.push(res.current_prereqs(w, n));
            nodes.push(n.clone());
        }
        for (i, t) in self.techs.iter().enumerate() {
            nodes.push(format!("{}{}", prefix, t.name));
            lists.push(res.techs[i].prereqs.clone());
        }

        // Edges by index, resolved with a linear scan: a hash map would be
        // faster and would also be the one thing this crate may not do. The
        // base game is 275 technologies and 479 edges, measured affordable at
        // load.
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

        match walk_for_cycle(&nodes, &edges) {
            Some(path) => Err(format!(
                "fkrecipes: a prerequisite cycle: {}",
                render_cycle_path(&path)
            )),
            None => Ok(()),
        }
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

fn render_cycle_path(path: &[String]) -> String {
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
