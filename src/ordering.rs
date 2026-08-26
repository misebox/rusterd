//! Ordering the entities within each level so that few relations cross.
//!
//! Which entity sits where in a row decides most of the crossings in a diagram:
//! no amount of clever routing rescues a child placed at the far end of the row
//! from its parent. Levels are swept up and down, each entity moving to the
//! middle of whatever it relates to, and adjacent pairs are then swapped
//! wherever that removes a crossing. An entity related to only one other ends
//! up beside it, which is what makes a lone relation a straight line.

/// Sweeps of the whole diagram. Each one moves every level towards its
/// neighbours, which have themselves just moved, so a few are needed before the
/// arrangement settles; more than this rarely changes anything.
const ROUNDS: usize = 8;

/// Shuffle each row, so that the search below starts somewhere else. The same
/// seed always gives the same shuffle: a diagram must not change between runs.
pub fn shuffle_levels(rows: &mut [Vec<usize>], seed: u64) {
    let mut state = seed | 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for row in rows.iter_mut() {
        for i in (1..row.len()).rev() {
            let j = (next() % (i as u64 + 1)) as usize;
            row.swap(i, j);
        }
    }
}

/// Order the entities within each row so that as few relations as possible
/// cross. `rows` holds entity ids by level, in the order they currently sit;
/// `links` are the relations between them, in either direction.
pub fn order_levels(
    rows: &mut [Vec<usize>],
    links: &[(usize, usize)],
    attractions: &[(usize, usize)],
    lone_weight: usize,
) {
    if rows.len() < 2 {
        return;
    }

    let node_count = rows.iter().flatten().copied().max().unwrap_or(0) + 1;
    let mut level_of = vec![0usize; node_count];
    for (level, row) in rows.iter().enumerate() {
        for &node in row {
            level_of[node] = level;
        }
    }

    // Entities asked to be near one another pull on each other exactly as a
    // relationship does — they are simply never counted as crossing, since
    // there is no line to cross.
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    for &(a, b) in links.iter().chain(attractions) {
        if a == b || level_of[a] == level_of[b] {
            continue;
        }
        neighbours[a].push(b);
        neighbours[b].push(a);
    }

    let weights = weights(links, node_count, lone_weight);
    let cost = |rows: &[Vec<usize>]| {
        crossings(rows, links, &weights, &level_of, node_count)
            + apartness(rows, attractions, &level_of, node_count)
    };
    let mut best = rows.to_vec();
    let mut fewest = cost(rows);

    for round in 0..ROUNDS {
        let downwards = round % 2 == 0;
        let order: Vec<usize> = if downwards {
            (1..rows.len()).collect()
        } else {
            (0..rows.len() - 1).rev().collect()
        };

        for level in order {
            sort_by_median(rows, &neighbours, &level_of, level, downwards);
        }
        transpose(rows, &cost);

        let count = cost(rows);
        if count < fewest {
            fewest = count;
            best = rows.to_vec();
        }
    }

    rows.clone_from_slice(&best);
}

/// Move every entity in one row to the middle of what it relates to on the side
/// being swept from. An entity relating to nothing there keeps its place.
fn sort_by_median(
    rows: &mut [Vec<usize>],
    neighbours: &[Vec<usize>],
    level_of: &[usize],
    level: usize,
    downwards: bool,
) {
    let places = places(rows, level_of, neighbours.len());

    let mut ranked: Vec<(f64, usize, usize)> = rows[level]
        .iter()
        .enumerate()
        .map(|(index, &node)| {
            let mut seen: Vec<f64> = neighbours[node]
                .iter()
                .filter(|&&other| (level_of[other] < level) == downwards)
                .map(|&other| places[other])
                .collect();
            seen.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = match seen.len() {
                0 => places[node],
                n if n % 2 == 1 => seen[n / 2],
                n => (seen[n / 2 - 1] + seen[n / 2]) / 2.0,
            };
            (median, index, node)
        })
        .collect();

    ranked.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    rows[level] = ranked.into_iter().map(|(_, _, node)| node).collect();
}

/// Swap neighbouring entities wherever that improves the arrangement, until
/// none of the swaps left helps.
fn transpose(rows: &mut [Vec<usize>], cost: &impl Fn(&[Vec<usize>]) -> usize) {
    let mut best = cost(rows);
    let mut improved = true;

    while improved {
        improved = false;
        for level in 0..rows.len() {
            for i in 0..rows[level].len().saturating_sub(1) {
                rows[level].swap(i, i + 1);
                let count = cost(rows);
                if count < best {
                    best = count;
                    improved = true;
                } else {
                    rows[level].swap(i, i + 1);
                }
            }
        }
    }
}

/// What one place of separation between entities asked to be near one another
/// is worth against one crossing.
const APART: usize = 2;

/// How far apart those entities sit. Neighbours in a row cost nothing.
fn apartness(
    rows: &[Vec<usize>],
    attractions: &[(usize, usize)],
    level_of: &[usize],
    nodes: usize,
) -> usize {
    if attractions.is_empty() {
        return 0;
    }
    let places = places(rows, level_of, nodes);

    attractions
        .iter()
        .map(|&(a, b)| {
            if level_of[a] == level_of[b] {
                let row = &rows[level_of[a]];
                let seat = |node| row.iter().position(|&x| x == node).unwrap_or(0);
                seat(a).abs_diff(seat(b)).saturating_sub(1) * APART
            } else {
                ((places[a] - places[b]).abs() * (APART * 2) as f64) as usize
            }
        })
        .sum()
}

/// Each entity's place in its row, as a fraction of the row's width, so that
/// rows of different lengths can be compared.
fn places(rows: &[Vec<usize>], level_of: &[usize], nodes: usize) -> Vec<f64> {
    let mut places = vec![0.0; nodes];
    for row in rows {
        let last = row.len().saturating_sub(1);
        for (index, &node) in row.iter().enumerate() {
            places[node] = if last == 0 {
                0.5
            } else {
                index as f64 / last as f64
            };
            debug_assert!(level_of[node] < rows.len());
        }
    }
    places
}

/// How much each relation costs to cross: `lone` for the only relation an
/// entity has, one for the rest.
fn weights(links: &[(usize, usize)], nodes: usize, lone: usize) -> Vec<usize> {
    let mut degree = vec![0usize; nodes];
    for &(a, b) in links.iter().filter(|(a, b)| a != b) {
        degree[a] += 1;
        degree[b] += 1;
    }
    links
        .iter()
        .map(|&(a, b)| {
            if a != b && (degree[a] == 1 || degree[b] == 1) {
                lone
            } else {
                1
            }
        })
        .collect()
}

/// What the relations that cross cost: two of them run over each other when
/// their ends are in the opposite order at the top and at the bottom, and the
/// levels they span overlap.
fn crossings(
    rows: &[Vec<usize>],
    links: &[(usize, usize)],
    weights: &[usize],
    level_of: &[usize],
    nodes: usize,
) -> usize {
    let places = places(rows, level_of, nodes);

    let spans: Vec<(usize, usize, f64, f64, usize)> = links
        .iter()
        .zip(weights)
        .filter(|((a, b), _)| a != b && level_of[*a] != level_of[*b])
        .map(|(&(a, b), &weight)| {
            let (top, bottom) = if level_of[a] < level_of[b] {
                (a, b)
            } else {
                (b, a)
            };
            (
                level_of[top],
                level_of[bottom],
                places[top],
                places[bottom],
                weight,
            )
        })
        .collect();

    let mut cost = 0;
    for (i, first) in spans.iter().enumerate() {
        for second in spans.iter().skip(i + 1) {
            let overlapping = first.0 < second.1 && second.0 < first.1;
            if !overlapping {
                continue;
            }
            let at_top = first.2 - second.2;
            let at_bottom = first.3 - second.3;
            if at_top * at_bottom < 0.0 {
                cost += first.4 * second.4;
            }
        }
    }
    cost
}

#[cfg(test)]
mod tests {
    use super::order_levels;

    #[test]
    fn uncrosses_a_swapped_pair() {
        let mut rows = vec![vec![0, 1], vec![2, 3]];
        order_levels(&mut rows, &[(0, 3), (1, 2)], &[], 1);
        assert_eq!(rows, vec![vec![0, 1], vec![3, 2]]);
    }

    #[test]
    fn puts_a_lone_relation_beside_what_it_relates_to() {
        // Two hubs, each with two children, handed over interleaved.
        let mut rows = vec![vec![0, 1], vec![2, 3, 4, 5]];
        order_levels(&mut rows, &[(0, 2), (1, 3), (0, 4), (1, 5)], &[], 1);
        let children = &rows[1];
        let of_first: Vec<usize> = children
            .iter()
            .enumerate()
            .filter(|(_, n)| **n == 2 || **n == 4)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(of_first, vec![0, 1], "children of one hub should sit together");
    }

    #[test]
    fn draws_entities_asked_to_be_near_side_by_side() {
        // Two hubs with a child each, plus a request to keep the two children
        // together even though nothing relates them.
        let mut rows = vec![vec![0, 1], vec![2, 3, 4, 5]];
        let links = [(0, 2), (0, 4), (1, 3), (1, 5)];
        order_levels(&mut rows, &links, &[(2, 3)], 1);
        let place = |n: usize| rows[1].iter().position(|&x| x == n).unwrap();
        assert_eq!(
            place(2).abs_diff(place(3)),
            1,
            "asked to be near: {:?}",
            rows[1]
        );
    }

    #[test]
    fn leaves_a_single_row_alone() {
        let mut rows = vec![vec![2, 0, 1]];
        order_levels(&mut rows, &[(0, 1)], &[], 1);
        assert_eq!(rows, vec![vec![2, 0, 1]]);
    }
}
