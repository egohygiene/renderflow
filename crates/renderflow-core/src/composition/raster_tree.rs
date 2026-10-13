//! Conservative allocation preflight for the pinned Resvg tree renderer.
//!
//! Resvg can allocate isolated group layers, nested SVG canvases, raster decode
//! surfaces and clip masks in addition to the output pixmap. Its own clipping
//! bound spans five canvas widths and heights, so output dimensions alone do
//! not bound these allocations. This estimate covers active pixel surfaces; it
//! is not an allocator, total-RSS, execution-time or successful-render proof.

use anyhow::{bail, ensure, Context, Result};
use resvg::{tiny_skia, usvg};

const MAX_ACTIVE_SURFACE_BYTES: u64 = 384 * 1024 * 1024;
const MAX_TREE_DEPTH: usize = 32;
const MAX_TREE_NODES: usize = 65_536;
const MAX_PATH_SEGMENTS: usize = 524_288;
const MAX_SOURCE_PIXELS: u64 = 16_777_216;

/// Check the exact tree and uniform point-to-pixel transform used for PNG.
/// The caller has already checked physical size and fully decoded image inputs.
pub(crate) fn validate_tree(tree: &usvg::Tree, dpi: u32, width: u32, height: u32) -> Result<()> {
    ensure!((36..=600).contains(&dpi) && width > 0 && height > 0,
        "composition.raster.tree_geometry: invalid raster preflight dimensions or resolution");
    ensure!(width <= 8192 && height <= 8192 && u64::from(width) * u64::from(height) <= MAX_SOURCE_PIXELS,
        "composition.raster.tree_geometry: output exceeds the 8192-axis/16-Mpixel profile");
    ensure!(tree.filters().is_empty() && tree.masks().is_empty(),
        "composition.raster.tree_unsupported: filters and masks are unavailable in the PNG profile");
    let scale = dpi as f32 / 72.0;
    let surface = Surface { width, height };
    let mut budget = Budget { width_limit: width + 8, height_limit: height + 8, nodes: 0, segments: 0 };
    budget.children(tree.root(), tiny_skia::Transform::from_scale(scale, scale),
        surface, surface, surface.rgba_bytes(), 0)
}

#[derive(Clone, Copy)]
struct Surface { width: u32, height: u32 }

impl Surface {
    fn rgba_bytes(self) -> u64 { u64::from(self.width) * u64::from(self.height) * 4 }
}

struct Budget {
    width_limit: u32,
    height_limit: u32,
    nodes: usize,
    segments: usize,
}

impl Budget {
    fn node(&mut self) -> Result<()> {
        self.nodes = self.nodes.saturating_add(1);
        ensure!(self.nodes <= MAX_TREE_NODES,
            "composition.raster.tree_budget: rendering tree exceeds 65536 visited nodes including repeated artwork and clip paths");
        Ok(())
    }

    fn active(&self, bytes: u64) -> Result<()> {
        ensure!(bytes <= MAX_ACTIVE_SURFACE_BYTES,
            "composition.raster.tree_budget: estimated simultaneous pixel surfaces exceed 384 MiB; reduce resolution or simplify nested opacity and clipping");
        Ok(())
    }

    fn children(&mut self, group: &usvg::Group, transform: tiny_skia::Transform,
        surface: Surface, context: Surface, active: u64, depth: usize) -> Result<()> {
        ensure!(depth <= MAX_TREE_DEPTH,
            "composition.raster.tree_budget: parsed tree exceeds 32 nested rendering levels; simplify nested groups");
        self.active(active)?;
        valid_transform(transform)?;
        for node in group.children() {
            self.node()?;
            match node {
                usvg::Node::Group(group) => self.group(group, transform, surface, context, active, depth + 1)?,
                usvg::Node::Path(path) => self.path(path)?,
                usvg::Node::Image(image) => match image.kind() {
                    usvg::ImageKind::SVG(tree) => {
                        ensure!(tree.filters().is_empty() && tree.masks().is_empty(),
                            "composition.raster.tree_unsupported: embedded SVG filters and masks are unavailable");
                        // Resvg's vector-image path allocates a full copy of the
                        // current pixmap and starts a fresh render context.
                        self.children(tree.root(), transform, surface, surface,
                            active + surface.rgba_bytes(), depth + 1)?;
                    }
                    usvg::ImageKind::PNG(_) | usvg::ImageKind::JPEG(_) => {
                        let size = image.size();
                        let (width, height) = (size.width(), size.height());
                        ensure!(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0
                            && width.fract() == 0.0 && height.fract() == 0.0
                            && width <= 8192.0 && height <= 8192.0,
                            "composition.raster.tree_geometry: raster source dimensions exceed the bounded image profile");
                        let pixels = width as u64 * height as u64;
                        ensure!(pixels <= MAX_SOURCE_PIXELS,
                            "composition.raster.tree_budget: raster source exceeds 16 Mpixels");
                        // Charge two RGBA surfaces conservatively for decode
                        // and conversion even when a codec can reuse storage.
                        self.active(active + pixels * 8)?;
                    }
                    _ => bail!("composition.raster.tree_unsupported: only preflighted PNG, JPEG and bounded SVG images are available"),
                },
                usvg::Node::Text(_) => bail!("composition.raster.tree_unsupported: visual lettering must already be measured outlines"),
            }
        }
        Ok(())
    }

    fn group(&mut self, group: &usvg::Group, transform: tiny_skia::Transform,
        surface: Surface, context: Surface, active: u64, depth: usize) -> Result<()> {
        ensure!(group.filters().is_empty() && group.mask().is_none()
            && group.blend_mode() == usvg::BlendMode::Normal,
            "composition.raster.tree_unsupported: filters, masks and non-normal blend modes are unavailable");
        let transform = transform.pre_concat(group.transform());
        valid_transform(transform)?;
        if !group.should_isolate() {
            return self.children(group, transform, surface, context, active, depth);
        }
        let bbox = group.layer_bounding_box().transform(transform)
            .context("composition.raster.tree_geometry: isolated group has an invalid transformed bound")?;
        let values = [bbox.x(), bbox.y(), bbox.width(), bbox.height()];
        ensure!(values.into_iter().all(f32::is_finite),
            "composition.raster.tree_geometry: isolated group has nonfinite bounds");
        let padded_width = f64::from(bbox.width()).ceil() + 4.0;
        let padded_height = f64::from(bbox.height()).ceil() + 4.0;
        // Use the unclamped bound. This deliberately refuses giant off-canvas
        // layers instead of relying on Resvg's much larger 25x-area clamp.
        ensure!(padded_width <= f64::from(self.width_limit)
            && padded_height <= f64::from(self.height_limit),
            "composition.raster.tree_budget: an isolated layer extends beyond the bounded output-sized surface; reduce off-canvas geometry or simplify opacity and clipping");
        let left = f64::from(bbox.x()).floor() - 2.0;
        let top = f64::from(bbox.y()).floor() - 2.0;
        ensure!(left >= f64::from(i32::MIN) && top >= f64::from(i32::MIN)
            && left + padded_width <= f64::from(i32::MAX)
            && top + padded_height <= f64::from(i32::MAX),
            "composition.raster.tree_geometry: isolated layer cannot be represented by bounded integer pixel coordinates");
        // Match Resvg's clip rectangle and origin shift. The context stays the
        // original render context inside groups, but resets for SVG images.
        let right = (left + padded_width).min(f64::from(context.width) * 3.0);
        let bottom = (top + padded_height).min(f64::from(context.height) * 3.0);
        let left = left.max(-f64::from(context.width) * 2.0);
        let top = top.max(-f64::from(context.height) * 2.0);
        if right <= left || bottom <= top { return Ok(()); }
        let isolated = Surface { width: (right - left) as u32, height: (bottom - top) as u32 };
        let active = active + isolated.rgba_bytes();
        self.active(active)?;
        let shifted = tiny_skia::Transform::from_translate(-(left as f32), -(top as f32))
            .pre_concat(transform);
        self.children(group, shifted, isolated, context, active, depth)?;
        if let Some(clip) = group.clip_path() {
            self.clip(clip)?;
            // Clip application occurs after child rendering: one RGBA clip
            // pixmap and one alpha mask coexist with this isolated surface.
            self.active(active + isolated.rgba_bytes() + isolated.rgba_bytes() / 4)?;
        }
        Ok(())
    }

    fn clip(&mut self, clip: &usvg::ClipPath) -> Result<()> {
        ensure!(clip.clip_path().is_none(),
            "composition.raster.tree_unsupported: recursive clipping is unavailable in the PNG profile");
        valid_transform(clip.transform())?;
        ensure!(clip.root().children().len() <= 4,
            "composition.raster.tree_unsupported: only simple generated clipping paths are available");
        for node in clip.root().children() {
            self.node()?;
            match node {
                usvg::Node::Path(path) => self.path(path)?,
                _ => bail!("composition.raster.tree_unsupported: nested groups, images and text in clip paths are unavailable"),
            }
        }
        Ok(())
    }

    fn path(&mut self, path: &usvg::Path) -> Result<()> {
        ensure!(path.fill().is_none_or(|fill| matches!(fill.paint(), usvg::Paint::Color(_)))
            && path.stroke().is_none_or(|stroke| matches!(stroke.paint(), usvg::Paint::Color(_))),
            "composition.raster.tree_unsupported: gradients and pattern paints are outside the bounded composition profile");
        self.segments = self.segments.saturating_add(path.data().segments().count());
        ensure!(self.segments <= MAX_PATH_SEGMENTS,
            "composition.raster.tree_budget: parsed artwork and outlines exceed 524288 path segments including repetitions");
        Ok(())
    }
}

fn valid_transform(transform: tiny_skia::Transform) -> Result<()> {
    let values = [transform.sx, transform.sy, transform.kx, transform.ky, transform.tx, transform.ty];
    let determinant = f64::from(transform.sx) * f64::from(transform.sy)
        - f64::from(transform.kx) * f64::from(transform.ky);
    ensure!(values.into_iter().all(f32::is_finite) && determinant.is_finite() && determinant != 0.0,
        "composition.raster.tree_geometry: parsed geometry has a nonfinite or singular transform");
    // Match the actual f32 rendering provider's invertibility requirement.
    // A nonzero f64 determinant alone can admit an overflowing f32 inverse.
    let inverse = transform.invert()
        .context("composition.raster.tree_geometry: parsed transform is not invertible by the pinned raster provider")?;
    ensure!([inverse.sx, inverse.sy, inverse.kx, inverse.ky, inverse.tx, inverse.ty]
        .into_iter().all(f32::is_finite),
        "composition.raster.tree_geometry: parsed transform has a nonfinite raster-provider inverse");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_double_determinant_does_not_admit_an_overflowing_float_inverse() {
        let tiny = tiny_skia::Transform::from_scale(1e-40, 1e-40);
        assert!(f64::from(tiny.sx) * f64::from(tiny.sy) > 0.0);
        assert!(valid_transform(tiny).is_err());
        assert!(valid_transform(tiny_skia::Transform::from_scale(2.0, 2.0)).is_ok());
    }
}
