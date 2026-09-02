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

/// How many times to draw the un-pinned entities towards their relations. Each
/// pass can only move one further, so a few are enough for the depths a schema
/// reaches, and it stops early when nothing moves.
const ROUNDS: usize = 16;

/// One relation, pointing from the row above to the row below.
#[derive(Clone, Copy)]
struct Arc {
    tail: usize,
    head: usize,
}

/// Which row each entity belongs on, counting from zero at the top.
///
/// An entity the source placed by hand keeps its place: a level hint is an
/// instruction, not a suggestion. Everything else is worked out around those —
/// being told where one table goes is no reason to stop thinking about the
/// rest, which is what putting them all on row zero amounted to.
pub fn assign_levels(ir: &GraphIR) -> HashMap<&str, i64> {
    let index: HashMap<&str, usize> = ir
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    let arcs = directed_arcs(ir, &index);
    let pins: Vec<Option<i64>> = ir.nodes.iter().map(|node| node.level).collect();
    let ranks = if pins.iter().all(Option::is_none) {
        rank(ir.nodes.len(), &arcs)
    } else {
        rank_around(&pins, &arcs)
    };

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
                Arc {
                    tail: from,
                    head: to,
                }
            } else {
                Arc {
                    tail: to,
                    head: from,
                }
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

/// Rank the nodes with some of them already placed.
///
/// The pinned ones do not move. The rest are placed below their parents and
/// above their children, and then pulled towards whichever of the two they
/// have more of, which is what shortens the relations between them.
fn rank_around(pins: &[Option<i64>], arcs: &[Arc]) -> Vec<i64> {
    let mut ranks: Vec<i64> = pins.iter().map(|pin| pin.unwrap_or(0)).collect();
    settle(pins, arcs, &mut ranks);
    for _ in 0..ROUNDS {
        if !tighten(pins, arcs, &mut ranks) {
            break;
        }
    }

    let lowest = ranks.iter().copied().min().unwrap_or(0);
    for rank in ranks.iter_mut() {
        *rank -= lowest;
    }
    ranks
}

/// Move the free entities until every relation reaches downwards: a child
/// below its parents, a parent above its children. Pinned entities stay put,
/// so pins that contradict each other simply leave a relation reaching the
/// wrong way rather than moving what the source asked for.
fn settle(pins: &[Option<i64>], arcs: &[Arc], ranks: &mut [i64]) {
    for _ in 0..ranks.len().max(1) {
        let mut moved = false;
        for arc in arcs {
            if slack(arc, ranks) >= 0 {
                continue;
            }
            if pins[arc.head].is_none() {
                ranks[arc.head] = ranks[arc.tail] + 1;
                moved = true;
            } else if pins[arc.tail].is_none() {
                ranks[arc.tail] = ranks[arc.head] - 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

/// Draw each free entity towards the end it has more relations to, as far as
/// its parents and children allow. Reports whether anything moved.
fn tighten(pins: &[Option<i64>], arcs: &[Arc], ranks: &mut [i64]) -> bool {
    let mut moved = false;

    for node in 0..ranks.len() {
        if pins[node].is_some() {
            continue;
        }

        // As high as the parents allow, and as low as the children do.
        let below = arcs
            .iter()
            .filter(|arc| arc.head == node)
            .map(|arc| ranks[arc.tail] + 1)
            .max();
        let above = arcs
            .iter()
            .filter(|arc| arc.tail == node)
            .map(|arc| ranks[arc.head] - 1)
            .min();

        // The total reach of an entity's relations falls as it moves towards
        // whichever end it has more of, so it goes as far that way as it can.
        let parents = arcs.iter().filter(|arc| arc.head == node).count();
        let children = arcs.iter().filter(|arc| arc.tail == node).count();
        let want = match parents.cmp(&children) {
            std::cmp::Ordering::Greater => below.or(above),
            std::cmp::Ordering::Less => above.or(below),
            // Anywhere between them is as short as any other; staying put
            // keeps the ranking settled rather than oscillating.
            std::cmp::Ordering::Equal => None,
        };

        let Some(want) = want else { continue };
        // A pin above a child of it can ask for the impossible. Reaching down
        // is the more important of the two, so the parents win.
        let placed = match (below, above) {
            (Some(below), Some(above)) if below > above => below,
            _ => want.clamp(below.unwrap_or(want), above.unwrap_or(want)),
        };

        if placed != ranks[node] {
            ranks[node] = placed;
            moved = true;
        }
    }

    moved
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
        let shift = if reached.contains(&arc.head) {
            -shift
        } else {
            shift
        };
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
            .map(
                |arc| match (tail_side.contains(&arc.tail), tail_side.contains(&arc.head)) {
                    (true, false) => 1,
                    (false, true) => -1,
                    _ => 0,
                },
            )
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

/// How many entities may be moved before giving up on narrowing a drawing.
/// Each pass moves at least one, so a schema runs out of entities long before
/// this; reaching it means the levels will not settle, which is not worth
/// failing over.
const MAX_FOLDS: usize = 500;

/// Fold the levels that have grown wider than the drawing wants to be.
///
/// Rows are not what a reader takes from the drawing — reading downwards is.
/// A row of sixteen leaves says nothing that two rows of eight do not, and
/// costs a diagram nobody can see at once. So the surplus is sent down a row,
/// childless entities first, since moving one with children pushes them too.
///
/// An entity the source pinned never moves: `@hint.level` is an instruction,
/// and a hand-written arrangement is followed as written even when it is wide.
pub fn fold_levels<'a>(
    ir: &'a GraphIR,
    levels: &mut HashMap<&'a str, i64>,
    widths: &[f64],
    cap: f64,
) {
    let index: HashMap<&str, usize> = ir
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    let arcs = directed_arcs(ir, &index);
    let pins: Vec<Option<i64>> = ir.nodes.iter().map(|node| node.level).collect();
    let mut ranks: Vec<i64> = ir
        .nodes
        .iter()
        .map(|node| levels.get(node.id.as_str()).copied().unwrap_or(0))
        .collect();

    let mut bears_children = vec![false; ir.nodes.len()];
    let mut relations = vec![0usize; ir.nodes.len()];
    for arc in &arcs {
        bears_children[arc.tail] = true;
        relations[arc.tail] += 1;
        relations[arc.head] += 1;
    }

    // A row nothing can be taken out of stays wide. Remembering which ones
    // those are is what stops the search picking the same row forever.
    let mut settled: HashSet<i64> = HashSet::new();

    for _ in 0..MAX_FOLDS {
        let Some(row) = widest_over(&ranks, widths, cap, &settled) else {
            break;
        };
        let keep = Keep {
            pins: &pins,
            widths,
            bears_children: &bears_children,
            relations: &relations,
        };
        if !thin_out(row, &keep, cap, &mut ranks) {
            settled.insert(row);
            continue;
        }
        push_children_down(&pins, &arcs, &mut ranks);
    }

    for (node, rank) in ir.nodes.iter().zip(&ranks) {
        levels.insert(node.id.as_str(), *rank);
    }
}

/// The topmost row that is over the cap and has more than one entity on it.
/// Topmost, so that what it sheds falls onto rows not yet looked at.
fn widest_over(ranks: &[i64], widths: &[f64], cap: f64, settled: &HashSet<i64>) -> Option<i64> {
    let mut across: HashMap<i64, (f64, usize)> = HashMap::new();
    for (i, &rank) in ranks.iter().enumerate() {
        let row = across.entry(rank).or_insert((0.0, 0));
        row.0 += widths[i];
        row.1 += 1;
    }

    across
        .iter()
        .filter(|(rank, (width, count))| *width > cap && *count > 1 && !settled.contains(rank))
        .map(|(rank, _)| *rank)
        .min()
}

/// What decides which entities keep their row when it is thinned.
struct Keep<'a> {
    pins: &'a [Option<i64>],
    widths: &'a [f64],
    bears_children: &'a [bool],
    relations: &'a [usize],
}

/// Send the surplus of one row down to the next, and say whether anything went.
///
/// What stays is chosen before what goes: the pinned entities, which cannot
/// move at all, then the ones bearing children, since moving one of those
/// pushes its children after it, and then whichever have the most relations.
/// The last is what keeps the drawing readable: a row further from the entities
/// it references is a row whose lines have more to cross on the way, so the
/// ones sent down are the ones with the fewest lines to drag along.
fn thin_out(row: i64, keep: &Keep, cap: f64, ranks: &mut [i64]) -> bool {
    let pins = keep.pins;
    let widths = keep.widths;

    let mut on_row: Vec<usize> = (0..ranks.len()).filter(|&i| ranks[i] == row).collect();
    on_row.sort_by(|&a, &b| {
        let first = |i: usize| (pins[i].is_none(), !keep.bears_children[i]);
        first(a)
            .cmp(&first(b))
            .then(keep.relations[b].cmp(&keep.relations[a]))
            .then(widths[b].total_cmp(&widths[a]))
    });

    let mut kept = 0.0;
    let mut moved = false;
    for i in on_row {
        // The first entity stays whatever it measures: a row cannot be
        // narrower than one entity, and an empty row helps nobody.
        if kept == 0.0 || pins[i].is_some() || kept + widths[i] <= cap {
            kept += widths[i];
            continue;
        }
        ranks[i] = row + 1;
        moved = true;
    }
    moved
}

/// Restore the rule the folding may have broken: a child sits below its
/// parents. Only downwards, so that nothing climbs back into the row that was
/// just thinned.
fn push_children_down(pins: &[Option<i64>], arcs: &[Arc], ranks: &mut [i64]) {
    for _ in 0..ranks.len().max(1) {
        let mut moved = false;
        for arc in arcs {
            if pins[arc.head].is_none() && ranks[arc.head] <= ranks[arc.tail] {
                ranks[arc.head] = ranks[arc.tail] + 1;
                moved = true;
            }
        }
        if !moved {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Arc, rank};

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

    /// One entity naming its level used to send every other entity to row
    /// zero, which drew a chain of four as a line of four.
    #[test]
    fn works_out_the_entities_no_hint_mentions() {
        let source = r#"
            entity Team { @hint.level = 0 id int pk }
            entity Member { id int pk }
            entity Session { id int pk }
            entity Token { id int pk }
            rel {
                Team 1 -- * Member
                Member 1 -- * Session
                Session 1 -- * Token
            }
        "#;
        let levels = levels_of(source);
        assert_eq!(levels["Team"], 0);
        assert_eq!(levels["Member"], 1);
        assert_eq!(levels["Session"], 2);
        assert_eq!(levels["Token"], 3);
    }

    #[test]
    fn leaves_a_pinned_entity_where_it_was_put() {
        let source = r#"
            entity Team { id int pk }
            entity Member { id int pk }
            entity Session { @hint.level = 3 id int pk }
            rel {
                Team 1 -- * Member
                Member 1 -- * Session
            }
        "#;
        let levels = levels_of(source);
        assert_eq!(levels["Session"], 3, "the hint is an instruction");
        assert_eq!(levels["Team"], 0);
        assert_eq!(levels["Member"], 1);
    }

    /// Pinning a child above its parent asks for something the drawing cannot
    /// have. Both hints are still obeyed; the relation between them is what
    /// gives, and the entity with no hint finds a place in between.
    #[test]
    fn obeys_hints_that_contradict_each_other() {
        let source = r#"
            entity Team { @hint.level = 2 id int pk }
            entity Member { @hint.level = 0 id int pk }
            entity Session { id int pk }
            rel {
                Team 1 -- * Member
                Member 1 -- * Session
            }
        "#;
        let levels = levels_of(source);
        assert_eq!(levels["Member"], 0);
        assert_eq!(levels["Team"], 2);
        assert_eq!(levels["Session"], 1);
    }

    /// A row of leaves wider than the drawing wants is split, and what is sent
    /// down is the entity with the fewest relations, not the widest one.
    #[test]
    fn folds_a_row_that_grew_too_wide() {
        let source = r#"
            entity Team { id int pk }
            entity Root { id int pk }
            entity Alpha { id int pk }
            entity Beta { id int pk }
            entity Gamma { id int pk }
            rel {
                Team 1 -- * Alpha
                Team 1 -- * Beta
                Team 1 -- * Gamma
                Root 1 -- * Alpha
                Root 1 -- * Beta
            }
        "#;
        let levels = folded_levels(source, 2.0);
        assert_eq!(levels["Team"], 0);
        assert_eq!(levels["Root"], 0);
        assert_eq!(levels["Alpha"], 1, "two relations, so it keeps its row");
        assert_eq!(levels["Beta"], 1, "two relations, so it keeps its row");
        assert_eq!(
            levels["Gamma"], 2,
            "one relation, so it is the one sent down"
        );
    }

    /// An arrangement written by hand is followed as written, however wide it
    /// comes out: `@hint.level` is an instruction, and folding is a guess.
    #[test]
    fn never_folds_what_the_source_pinned() {
        let source = r#"
            entity Team { @hint.level = 0 id int pk }
            entity Alpha { @hint.level = 1 id int pk }
            entity Beta { @hint.level = 1 id int pk }
            entity Gamma { @hint.level = 1 id int pk }
            rel {
                Team 1 -- * Alpha
                Team 1 -- * Beta
                Team 1 -- * Gamma
            }
        "#;
        let levels = folded_levels(source, 1.0);
        assert_eq!(levels["Alpha"], 1);
        assert_eq!(levels["Beta"], 1);
        assert_eq!(levels["Gamma"], 1);
    }

    /// Levels after folding, with every entity measured as one unit wide so
    /// that `cap` reads as "how many fit on a row".
    fn folded_levels(source: &str, cap: f64) -> std::collections::HashMap<String, i64> {
        let schema = crate::parser::Parser::new(source).unwrap().parse().unwrap();
        let ir = crate::ir::GraphIR::from_schema(&schema, None, crate::ir::DetailLevel::All);
        let mut levels = super::assign_levels(&ir);
        super::fold_levels(&ir, &mut levels, &vec![1.0; ir.nodes.len()], cap);
        levels
            .into_iter()
            .map(|(name, level)| (name.to_string(), level))
            .collect()
    }

    fn levels_of(source: &str) -> std::collections::HashMap<String, i64> {
        let schema = crate::parser::Parser::new(source).unwrap().parse().unwrap();
        let ir = crate::ir::GraphIR::from_schema(&schema, None, crate::ir::DetailLevel::All);
        super::assign_levels(&ir)
            .into_iter()
            .map(|(name, level)| (name.to_string(), level))
            .collect()
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
