//! Strict geometry admission for captured SVG plates used by the PNG provider.
//!
//! The shared loader limits the XML/resource vocabulary. This extra raster
//! boundary parses complete path and number-list syntax before a permissive SVG
//! renderer can silently discard a malformed shape or substitute a viewport.

use anyhow::{ensure, Context, Result};
use svgtypes::{NumberListParser, PathParser, PathSegment};

const MAX_COORDINATE: f64 = 10_000_000.0;
const MIN_VIEWPORT_DIMENSION: f64 = 0.000_001;
const MAX_PATH_SEGMENTS: usize = 65_536;
const MAX_DEPTH: usize = 16;

pub(crate) fn inspect(bytes: &[u8]) -> Result<()> {
    super::load::validate_svg(bytes)
        .context("composition.raster.svg_profile: captured SVG is outside the inert plate profile")?;
    let text = std::str::from_utf8(bytes)?;
    let document = roxmltree::Document::parse(text)?;
    let root = document.root_element();
    let mut segments = 0usize;
    let mut viewport = false;
    for node in document.descendants().filter(|node| node.is_element()) {
        ensure!(node.ancestors().filter(|ancestor| ancestor.is_element()).count() <= MAX_DEPTH,
            "composition.raster.svg_depth: captured SVG nesting exceeds 16 elements");
        let tag = node.tag_name().name();
        for attribute in node.attributes() {
            let name = attribute.name();
            let value = attribute.value();
            ensure!(attribute_allowed(tag, name),
                "composition.raster.svg_attribute: attribute {name} does not belong to supported {tag} geometry");
            match name {
                "d" => {
                    for segment in PathParser::from(value) {
                        segments = segments.saturating_add(1);
                        ensure!(segments <= MAX_PATH_SEGMENTS,
                            "composition.raster.svg_complexity: captured SVG exceeds 65536 path segments or coordinate pairs");
                        inspect_segment(segment.context("composition.raster.svg_path: malformed path command")?)?;
                    }
                }
                "viewBox" => {
                    let values = number_list(value, 4)?;
                    ensure!(values.len() == 4 && values[2] >= MIN_VIEWPORT_DIMENSION && values[3] >= MIN_VIEWPORT_DIMENSION,
                        "composition.raster.svg_viewport: viewBox requires four finite values and dimensions of at least 0.000001 user units");
                    viewport = true;
                }
                "points" => {
                    let values = number_list(value, MAX_PATH_SEGMENTS * 2)?;
                    ensure!(values.len() % 2 == 0,
                        "composition.raster.svg_points: every point requires an x and y coordinate");
                    segments = segments.saturating_add(values.len() / 2);
                    ensure!(segments <= MAX_PATH_SEGMENTS,
                        "composition.raster.svg_complexity: captured SVG exceeds 65536 path segments or coordinate pairs");
                }
                "width" | "height" | "r" | "rx" | "ry" | "stroke-width" => {
                    let number = scalar(value)?;
                    ensure!(number >= 0.0,
                        "composition.raster.svg_geometry: dimensions, radii and stroke widths cannot be negative");
                    if node == root && matches!(name, "width" | "height") {
                        ensure!(number >= MIN_VIEWPORT_DIMENSION,
                            "composition.raster.svg_viewport: an explicit root dimension must be at least 0.000001 user units");
                    }
                }
                "stroke-miterlimit" => ensure!(scalar(value)? >= 1.0,
                    "composition.raster.svg_geometry: stroke-miterlimit must be at least one"),
                _ => {}
            }
        }
    }
    ensure!(viewport || (root.attribute("width").is_some() && root.attribute("height").is_some()),
        "composition.raster.svg_viewport: supply a positive viewBox or both positive root dimensions; implicit provider viewport defaults are not accepted");
    Ok(())
}

fn scalar(value: &str) -> Result<f64> {
    let number: f64 = value.parse().context("composition.raster.svg_number: invalid numeric geometry")?;
    ensure!(number.is_finite() && number.abs() <= MAX_COORDINATE,
        "composition.raster.svg_number: coordinates must be finite and within 10000000 user units");
    Ok(number)
}

fn number_list(value: &str, maximum: usize) -> Result<Vec<f64>> {
    let mut result = Vec::new();
    for number in NumberListParser::from(value) {
        let number = number.context("composition.raster.svg_number_list: malformed numeric list")?;
        ensure!(number.is_finite() && number.abs() <= MAX_COORDINATE,
            "composition.raster.svg_number_list: coordinates must be finite and bounded");
        ensure!(result.len() < maximum,
            "composition.raster.svg_number_list: too many coordinates");
        result.push(number);
    }
    Ok(result)
}

fn inspect_segment(segment: PathSegment) -> Result<()> {
    // Keep complete command parsing in the pinned SVG parser, rather than
    // inventing a path grammar or accepting just a character whitelist.
    let coordinates = match segment {
        PathSegment::MoveTo { x, y, .. } | PathSegment::LineTo { x, y, .. }
        | PathSegment::SmoothQuadratic { x, y, .. } => vec![x, y],
        PathSegment::HorizontalLineTo { x, .. } => vec![x],
        PathSegment::VerticalLineTo { y, .. } => vec![y],
        PathSegment::CurveTo { x1, y1, x2, y2, x, y, .. } => vec![x1, y1, x2, y2, x, y],
        PathSegment::SmoothCurveTo { x2, y2, x, y, .. } => vec![x2, y2, x, y],
        PathSegment::Quadratic { x1, y1, x, y, .. } => vec![x1, y1, x, y],
        PathSegment::EllipticalArc { rx, ry, x_axis_rotation, x, y, .. } => {
            ensure!(rx >= 0.0 && ry >= 0.0,
                "composition.raster.svg_path: arc radii cannot be negative");
            vec![rx, ry, x_axis_rotation, x, y]
        }
        PathSegment::ClosePath { .. } => Vec::new(),
    };
    ensure!(coordinates.iter().all(|value| value.is_finite() && value.abs() <= MAX_COORDINATE),
        "composition.raster.svg_path: path coordinates must be finite and bounded");
    Ok(())
}

fn attribute_allowed(tag: &str, name: &str) -> bool {
    if matches!(name, "fill" | "stroke" | "fill-rule" | "stroke-linecap"
        | "stroke-linejoin" | "stroke-width" | "stroke-miterlimit"
        | "opacity" | "fill-opacity" | "stroke-opacity") { return true; }
    match tag {
        "svg" => matches!(name, "width" | "height" | "viewBox" | "preserveAspectRatio"),
        "g" => false,
        "path" => name == "d",
        "rect" => matches!(name, "x" | "y" | "width" | "height" | "rx" | "ry"),
        "circle" => matches!(name, "cx" | "cy" | "r"),
        "ellipse" => matches!(name, "cx" | "cy" | "rx" | "ry"),
        "line" => matches!(name, "x1" | "y1" | "x2" | "y2"),
        "polyline" | "polygon" => name == "points",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_geometry_is_not_silently_dropped() {
        for shape in [
            "<path d=\"MMMM\"/>",
            "<path d=\"M0 0 L1e309 2\"/>",
            "<polygon points=\"0 0 2\"/>",
            "<rect width=\"-1\" height=\"4\"/>",
            "<circle width=\"4\" r=\"2\"/>",
        ] {
            let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\">{shape}</svg>");
            assert!(inspect(svg.as_bytes()).is_err(), "{shape}");
        }
        assert!(inspect(b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1e309 20\"/>").is_err());
        assert!(inspect(b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1e-300 20\"/>").is_err());
        assert!(inspect(b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1e-300\" height=\"20\"/>").is_err());
        assert!(inspect(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").is_err());
    }

    #[test]
    fn complete_paths_and_explicit_viewports_are_admitted() {
        let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><path d=\"M1 1 L4 1 Q5 2 4 3 C3 4 2 4 1 3 Z\" fill=\"#112233\"/></svg>";
        assert!(inspect(svg).is_ok());
    }
}
