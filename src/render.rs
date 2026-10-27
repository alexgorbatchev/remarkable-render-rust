use crate::{DocumentSource, Error, Result};
use image::{ImageEncoder, Rgba, RgbaImage, codecs::png::PngEncoder};
use resvg::{tiny_skia, usvg};

pub const DEFAULT_DPI: i32 = 200;

/// Rasterize a PDF page and optionally composite native v6 strokes over it.
/// Non-positive DPI uses 200; the returned bytes are an RGBA PNG.
pub fn render_planner_page<'a>(
    source: impl Into<DocumentSource<'a>>,
    page: isize,
    strokes: &[u8],
    dpi: i32,
) -> Result<Vec<u8>> {
    let dpi = if dpi <= 0 { DEFAULT_DPI } else { dpi };
    let doc = source.into().resolve()?;
    let (mut background, width_pt, height_pt) = doc.native.render(doc.page_index(page)?, dpi)?;
    if !strokes.is_empty() {
        let svg = rmscene::render_rm_to_svg(
            strokes,
            &rmscene::SvgOptions {
                width_pt,
                height_pt,
                ..Default::default()
            },
        )?;
        composite_svg(&mut background, &svg)?;
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(
        background.as_raw(),
        background.width(),
        background.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(png)
}

fn composite_svg(background: &mut RgbaImage, svg: &str) -> Result<()> {
    let tree = usvg::Tree::from_data(svg.as_bytes(), &usvg::Options::default())?;
    let mut layer =
        tiny_skia::Pixmap::new(background.width(), background.height()).ok_or(Error::Dimensions)?;
    let transform = tiny_skia::Transform::from_scale(
        background.width() as f32 / tree.size().width(),
        background.height() as f32 / tree.size().height(),
    );
    resvg::render(&tree, transform, &mut layer.as_mut());
    let top = RgbaImage::from_fn(background.width(), background.height(), |x, y| {
        let color = layer.pixels()[(y * background.width() + x) as usize].demultiply();
        Rgba([color.red(), color.green(), color.blue(), color.alpha()])
    });
    image::imageops::overlay(background, &top, 0, 0);
    Ok(())
}
