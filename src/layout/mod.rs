//! Layout engine for ERD diagram generation.
//!
//! This module computes positions and routing for nodes and edges
//! in an Entity-Relationship diagram.

mod align;
mod analysis;
mod anchors;
mod corridor;
mod descent;
mod engine;
mod fit;
mod lanes;
mod layering;
mod placement;
mod routing;
mod straighten;
mod types;
mod waypoints;

pub use engine::LayoutEngine;
pub use types::{Layout, LayoutEdge, LayoutNode};

/// Read a shape written as `width:height` — `1:1`, `16:9`, `210:297`.
///
/// Two numbers rather than one, because the shape of a thing to put the diagram
/// in is what a reader has: a slide, a column, a sheet of paper. Dividing it
/// out is this function's job, not theirs.
pub fn aspect_from_name(text: &str) -> Option<f64> {
    let (width, height) = text.split_once(':')?;
    let width: f64 = width.trim().parse().ok()?;
    let height: f64 = height.trim().parse().ok()?;
    let ratio = width / height;
    (ratio > 0.0 && ratio.is_finite()).then_some(ratio)
}

#[cfg(test)]
mod aspect_tests {
    use super::aspect_from_name;

    #[test]
    fn reads_a_shape_as_two_numbers() {
        assert_eq!(aspect_from_name("1:1"), Some(1.0));
        assert_eq!(aspect_from_name("16:9"), Some(16.0 / 9.0));
        assert_eq!(aspect_from_name(" 4 : 3 "), Some(4.0 / 3.0));
    }

    #[test]
    fn refuses_what_is_not_a_shape() {
        for text in ["", "1", "1:0", "0:1", "-1:1", "a:b", "1:1:1"] {
            assert_eq!(aspect_from_name(text), None, "{text} is not a shape");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{DetailLevel, GraphIR};
    use crate::parser::Parser;

    #[test]
    fn test_basic_layout() {
        let input = r#"
            entity User { id int pk }
            entity Order { id int pk }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let engine = LayoutEngine::default();
        let layout = engine.layout(&ir);

        assert_eq!(layout.nodes.len(), 2);
        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
    }

    #[test]
    fn test_layout_with_levels() {
        let input = r#"
            entity User {
                @hint.level = 0
                id int pk
            }
            entity Order {
                @hint.level = 1
                id int pk
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let engine = LayoutEngine::default();
        let layout = engine.layout(&ir);

        let user = layout.nodes.iter().find(|n| n.id == "User").unwrap();
        let order = layout.nodes.iter().find(|n| n.id == "Order").unwrap();
        assert!(user.y < order.y);
    }

    #[test]
    fn test_layout_edges() {
        let input = r#"
            entity User { id int pk }
            entity Order { id int pk }
            rel { User 1 -- * Order }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let engine = LayoutEngine::default();
        let layout = engine.layout(&ir);

        assert_eq!(layout.edges.len(), 1);
    }
}
