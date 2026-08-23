//! Placing a row of things as close as possible to where they want to be.
//!
//! Entities within a level and anchors along one border are the same problem:
//! an ordered list of items, each with a position it would like to occupy, that
//! must keep its order, a minimum distance between neighbours, and stay within
//! bounds. Solved exactly rather than by spreading everything out evenly.

/// Positions for items that must keep their order and a minimum distance from
/// one another, as close as possible to `wanted`.
///
/// `span[i]` is the distance item `i` must keep from item `i + 1`; the last
/// entry is unused. `low` and `high` bound the positions themselves, so a
/// caller placing boxes passes the left edges and subtracts the last box's
/// width from `high`.
pub fn fit_in_order(wanted: &[f64], span: &[f64], low: f64, high: f64) -> Vec<f64> {
    debug_assert_eq!(wanted.len(), span.len());

    // Rewrite `pos[i] + span[i] <= pos[i + 1]` as `u[i] <= u[i + 1]` by
    // subtracting the room every earlier item takes up. What is left is a
    // nearest non-decreasing sequence.
    let mut offsets = Vec::with_capacity(wanted.len());
    let mut offset = 0.0;
    for gap in span {
        offsets.push(offset);
        offset += gap;
    }

    let target: Vec<f64> = wanted
        .iter()
        .zip(&offsets)
        .map(|(want, offset)| want - offset)
        .collect();

    let mut pos: Vec<f64> = isotonic(&target)
        .into_iter()
        .zip(&offsets)
        .map(|(u, offset)| u + offset)
        .collect();

    // Pull anything past the far end back, then push anything past the near end
    // forward. The two sweeps together fit the row into the bounds whenever it
    // fits at all.
    let mut ceiling = high;
    for i in (0..pos.len()).rev() {
        pos[i] = pos[i].min(ceiling);
        if i > 0 {
            ceiling = pos[i] - span[i - 1];
        }
    }
    let mut floor = low;
    for i in 0..pos.len() {
        pos[i] = pos[i].max(floor);
        floor = pos[i] + span[i];
    }

    pos
}

/// Nearest non-decreasing sequence to `target`, by pool adjacent violators.
fn isotonic(target: &[f64]) -> Vec<f64> {
    // Each block holds a run of positions that share one value: their mean.
    let mut blocks: Vec<(f64, usize)> = Vec::with_capacity(target.len());
    for &t in target {
        blocks.push((t, 1));
        while blocks.len() >= 2 {
            let (value, count) = blocks[blocks.len() - 1];
            let (prev_value, prev_count) = blocks[blocks.len() - 2];
            if prev_value <= value {
                break;
            }
            blocks.pop();
            blocks.pop();
            let merged = (prev_value * prev_count as f64 + value * count as f64)
                / (prev_count + count) as f64;
            blocks.push((merged, prev_count + count));
        }
    }

    let mut out = Vec::with_capacity(target.len());
    for (value, count) in blocks {
        out.extend(std::iter::repeat_n(value, count));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fit_in_order;

    #[test]
    fn leaves_items_where_they_ask_when_there_is_room() {
        let pos = fit_in_order(&[10.0, 50.0, 90.0], &[10.0; 3], 0.0, 100.0);
        assert_eq!(pos, vec![10.0, 50.0, 90.0]);
    }

    #[test]
    fn separates_items_that_want_the_same_place() {
        let pos = fit_in_order(&[50.0, 50.0], &[10.0; 2], 0.0, 100.0);
        assert_eq!(pos, vec![45.0, 55.0]);
    }

    #[test]
    fn keeps_the_order_it_was_given() {
        let pos = fit_in_order(&[80.0, 20.0], &[10.0; 2], 0.0, 100.0);
        assert!(pos[0] + 10.0 <= pos[1] + f64::EPSILON, "{pos:?}");
    }

    #[test]
    fn stays_within_the_bounds() {
        let pos = fit_in_order(&[-30.0, 200.0], &[10.0; 2], 0.0, 40.0);
        assert_eq!(pos, vec![0.0, 40.0]);
    }
}
