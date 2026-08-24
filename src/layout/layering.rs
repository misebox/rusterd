//! Working out which row each entity belongs on.
//!
//! An entity goes below the ones it references, so relations read downwards.
//! Which of the many rankings satisfying that to choose is the question: the
//! one where relations reach as short a distance as possible. A relation that
//! skips rows has to find its way past whatever stands in between, so keeping
//! them short is what makes the rest of the drawing simple.
//!
//! That is a linear program — minimise the total length subject to each
//! relation reaching down at least one row — and it is solved here the way
//! Gansner and co. solve it for `dot`: with the network simplex method on a
//! spanning tree of relations already drawn as short as they can be.

use crate::ast::Cardinality;
use crate::ir::GraphIR;
use std::collections::{HashMap, HashSet};

/// Guard against a cycle of exchanges that never settles. Reaching it means the
/// ranking is merely good rather than best, which is not worth failing over.
const MAX_EXCHANGES: usize = 500;

/// One relation, pointing from the row above to the row below.
#[derive(Clone, Copy)]
struct Arc {
    tail: usize,
    head: usize,
}

/// Which row each entity belongs on, counting from zero at the top.
///
/// Entities the source placed by hand keep their place: an arrangement or a
/// level hint is an instruction, not a suggestion. Automatic ranking only steps
/// in when the source says nothing at all.
pub fn assign_levels(ir: &GraphIR) -> HashMap<&str, i64> {
    if ir.nodes.iter().any(|node| node.level.is_some()) {
        return ir
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.level.unwrap_or(0)))
            .collect();
    }

    let index: HashMap<&str, usize> = ir
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    let arcs = directed_arcs(ir, &index);
    let ranks = rank(ir.nodes.len(), &arcs);

    ir.nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), ranks[i]))
        .collect()
}

/// Point every relation from parent to child, and break any loop of them.
///
/// The end that holds one row of the other table is the parent: `A 1 -- * B`
/// puts A above B. Where both ends say the same thing the relation is taken as
/// written. A cycle of references has no consistent direction, so the relations
/// closing the loops are turned around for the purpose of ranking.
fn directed_arcs(ir: &GraphIR, index: &HashMap<&str, usize>) -> Vec<Arc> {
    let single = |c: Cardinality| matches!(c, Cardinality::One | Cardinality::ZeroOrOne);

    let mut arcs: Vec<Arc> = ir
        .edges
        .iter()
        .filter(|edge| edge.from != edge.to)
        .filter_map(|edge| {
            let from = *index.get(edge.from.as_str())?;
            let to = *index.get(edge.to.as_str())?;
            let parent_leads = single(edge.from_cardinality) || !single(edge.to_cardinality);
            Some(if parent_leads {
                Arc { tail: from, head: to }
            } else {
                Arc { tail: to, head: from }
            })
        })
        .collect();

    // Depth-first search: an arc reaching a node still on the stack closes a
    // loop, so it is the one to turn around.
    let mut onto: Vec<Vec<usize>> = vec![Vec::new(); ir.nodes.len()];
    for (i, arc) in arcs.iter().enumerate() {
        onto[arc.tail].push(i);
    }

    let mut visiting = vec![false; ir.nodes.len()];
    let mut done = vec![false; ir.nodes.len()];
    let mut backwards: HashSet<usize> = HashSet::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();

    for start in 0..ir.nodes.len() {
        if done[start] {
            continue;
        }
        stack.push((start, 0));
        visiting[start] = true;

        while let Some((node, next)) = stack.pop() {
            match onto[node].get(next) {
                Some(&arc) => {
                    stack.push((node, next + 1));
                    let head = arcs[arc].head;
                    if visiting[head] {
                        backwards.insert(arc);
                    } else if !done[head] {
                        visiting[head] = true;
                        stack.push((head, 0));
                    }
                }
                None => {
                    visiting[node] = false;
                    done[node] = true;
                }
            }
        }
    }

    for &arc in &backwards {
        arcs[arc] = Arc {
            tail: arcs[arc].head,
            head: arcs[arc].tail,
        };
    }

    arcs
}

/// Rank the nodes so that every arc reaches down at least one row and the total
/// reach is as short as possible.
fn rank(nodes: usize, arcs: &[Arc]) -> Vec<i64> {
    let mut ranks = longest_path(nodes, arcs);
    if arcs.is_empty() {
        return ranks;
    }

    // Each connected group of entities is ranked on its own: nothing relates
    // one group to another, so nothing says how they should line up.
    for group in groups(nodes, arcs) {
        let inside: HashSet<usize> = group.iter().copied().collect();
        let within: Vec<Arc> = arcs
            .iter()
            .copied()
            .filter(|arc| inside.contains(&arc.tail))
            .collect();
        if within.is_empty() {
            continue;
        }
        simplex(&group, &within, &mut ranks);
    }

    let lowest = ranks.iter().copied().min().unwrap_or(0);
    for rank in ranks.iter_mut() {
        *rank -= lowest;
    }
    ranks
}

/// A first ranking: everything as high as its parents allow.
fn longest_path(nodes: usize, arcs: &[Arc]) -> Vec<i64> {
    let mut ranks = vec![0i64; nodes];
    // The arcs form no loops by now, so repeated relaxation settles.
    for _ in 0..nodes {
        let mut moved = false;
        for arc in arcs {
            if ranks[arc.head] < ranks[arc.tail] + 1 {
                ranks[arc.head] = ranks[arc.tail] + 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    ranks
}

/// Entities reachable from one another, ignoring which way the relations point.
fn groups(nodes: usize, arcs: &[Arc]) -> Vec<Vec<usize>> {
    let mut beside: Vec<Vec<usize>> = vec![Vec::new(); nodes];
    for arc in arcs {
        beside[arc.tail].push(arc.head);
        beside[arc.head].push(arc.tail);
    }

    let mut seen = vec![false; nodes];
    let mut found = Vec::new();
    for start in 0..nodes {
        if seen[start] {
            continue;
        }
        let mut group = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(node) = stack.pop() {
            group.push(node);
            for &next in &beside[node] {
                if !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        found.push(group);
    }
    found
}

/// How much slack an arc has: how much further than the one row it must reach.
fn slack(arc: &Arc, ranks: &[i64]) -> i64 {
    ranks[arc.head] - ranks[arc.tail] - 1
}

/// Shorten the total reach of the arcs by exchanging one tree relation for
/// another, until no exchange helps.
fn simplex(group: &[usize], arcs: &[Arc], ranks: &mut [i64]) {
    let mut tree = tight_tree(group, arcs, ranks);

    for _ in 0..MAX_EXCHANGES {
        let Some((leaving, tail_side)) = worst_tree_arc(arcs, &tree) else {
            break;
        };
        // The arc to bring in must reach back across the cut the leaving one
        // made, and the tightest such arc keeps the ranking feasible.
        let Some(entering) = arcs
            .iter()
            .enumerate()
            .filter(|(i, arc)| {
                *i != leaving && tail_side.contains(&arc.head) && !tail_side.contains(&arc.tail)
            })
            .min_by_key(|(_, arc)| slack(arc, ranks))
            .map(|(i, _)| i)
        else {
            break;
        };

        tree.remove(&leaving);
        tree.insert(entering);
        rerank(group, arcs, &tree, ranks);
    }
}

/// A spanning tree of arcs that already reach exactly one row, growing the tree
/// by pulling the rest of the drawing towards it.
fn tight_tree(group: &[usize], arcs: &[Arc], ranks: &mut [i64]) -> HashSet<usize> {
    loop {
        let mut reached: HashSet<usize> = HashSet::new();
        let mut tree: HashSet<usize> = HashSet::new();
        reached.insert(group[0]);

        let mut grew = true;
        while grew {
            grew = false;
            for (i, arc) in arcs.iter().enumerate() {
                if tree.contains(&i) || slack(arc, ranks) != 0 {
                    continue;
                }
                let has_tail = reached.contains(&arc.tail);
                let has_head = reached.contains(&arc.head);
                if has_tail == has_head {
                    continue;
                }
                reached.insert(if has_tail { arc.head } else { arc.tail });
                tree.insert(i);
                grew = true;
            }
        }

        if reached.len() == group.len() {
            return tree;
        }

        // Pull the whole tree towards the nearest entity outside it.
        let Some((arc, shift)) = arcs
            .iter()
            .filter(|arc| reached.contains(&arc.tail) != reached.contains(&arc.head))
            .map(|arc| (arc, slack(arc, ranks)))
            .min_by_key(|&(_, slack)| slack)
        else {
            return tree;
        };
        let shift = if reached.contains(&arc.head) { -shift } else { shift };
        for &node in &reached {
            ranks[node] += shift;
        }
    }
}

/// The tree arc worth replacing, with the entities left on its tail side.
///
/// Cutting a tree arc splits the entities in two. The arc is worth replacing
/// when more relations reach back across that split than run along it: moving
/// the two halves together would then shorten the drawing.
fn worst_tree_arc(arcs: &[Arc], tree: &HashSet<usize>) -> Option<(usize, HashSet<usize>)> {
    for &cut in tree {
        let tail_side = side_of(arcs, tree, cut);
        let value: i64 = arcs
            .iter()
            .map(|arc| {
                match (tail_side.contains(&arc.tail), tail_side.contains(&arc.head)) {
                    (true, false) => 1,
                    (false, true) => -1,
                    _ => 0,
                }
            })
            .sum();
        if value < 0 {
            return Some((cut, tail_side));
        }
    }
    None
}

/// The entities still joined to the tail of `cut` once that arc is taken out.
fn side_of(arcs: &[Arc], tree: &HashSet<usize>, cut: usize) -> HashSet<usize> {
    let mut beside: HashMap<usize, Vec<usize>> = HashMap::new();
    for &i in tree {
        if i == cut {
            continue;
        }
        beside.entry(arcs[i].tail).or_default().push(arcs[i].head);
        beside.entry(arcs[i].head).or_default().push(arcs[i].tail);
    }

    let mut side = HashSet::new();
    let mut stack = vec![arcs[cut].tail];
    side.insert(arcs[cut].tail);
    while let Some(node) = stack.pop() {
        for &next in beside.get(&node).into_iter().flatten() {
            if side.insert(next) {
                stack.push(next);
            }
        }
    }
    side
}

/// Read the ranks back off the tree, where every arc reaches exactly one row.
fn rerank(group: &[usize], arcs: &[Arc], tree: &HashSet<usize>, ranks: &mut [i64]) {
    let mut beside: HashMap<usize, Vec<(usize, i64)>> = HashMap::new();
    for &i in tree {
        let arc = &arcs[i];
        beside.entry(arc.tail).or_default().push((arc.head, 1));
        beside.entry(arc.head).or_default().push((arc.tail, -1));
    }

    let root = group[0];
    let mut placed: HashMap<usize, i64> = HashMap::from([(root, 0)]);
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let here = placed[&node];
        for &(next, step) in beside.get(&node).into_iter().flatten() {
            if let std::collections::hash_map::Entry::Vacant(slot) = placed.entry(next) {
                slot.insert(here + step);
                stack.push(next);
            }
        }
    }

    for (&node, &rank) in &placed {
        ranks[node] = rank;
    }
}

#[cfg(test)]
mod tests {
    use super::{rank, Arc};

    #[test]
    fn puts_a_child_below_its_parent() {
        let ranks = rank(2, &[Arc { tail: 0, head: 1 }]);
        assert_eq!(ranks, vec![0, 1]);
    }

    #[test]
    fn pulls_a_lone_parent_down_to_its_child() {
        // 0 and 1 both point at 2; 1 has nothing above it, so leaving it on the
        // top row would stretch its relation over two rows for no reason.
        let arcs = [
            Arc { tail: 0, head: 3 },
            Arc { tail: 3, head: 2 },
            Arc { tail: 1, head: 2 },
        ];
        let ranks = rank(4, &arcs);
        assert_eq!(ranks[2], 2, "the child sits below both parents");
        assert_eq!(ranks[1], 1, "the lone parent comes down beside the chain");
    }

    #[test]
    fn survives_a_loop_of_references() {
        let source = r#"
            entity A { id int pk }
            entity B { id int pk }
            entity C { id int pk }
            rel {
                A 1 -- * B
                B 1 -- * C
                C 1 -- * A
            }
        "#;
        let schema = crate::parser::Parser::new(source).unwrap().parse().unwrap();
        let ir = crate::ir::GraphIR::from_schema(&schema, None, crate::ir::DetailLevel::All);
        let levels = super::assign_levels(&ir);
        assert_eq!(levels.len(), 3);
        assert!(levels.values().all(|&level| level >= 0));
    }
}
