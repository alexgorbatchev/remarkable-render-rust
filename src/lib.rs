#![doc = include_str!("../README.md")]

mod document;
mod native;
mod planner;
mod render;

pub use document::{Document, DocumentSource, PageLink, Rect};
pub use planner::{PlannerIndex, PlannerKind, index_planner_dates};
pub use render::{DEFAULT_DPI, render_planner_page};

/// Errors retain their category and their underlying I/O, scene, or image error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("PDFium initialization failed: {0}")]
    Initialization(String),
    #[error("PDFium library digest does not match the pinned release")]
    LibraryDigest,
    #[error("PDFium {operation} failed (code {code})")]
    Pdfium {
        operation: &'static str,
        code: std::ffi::c_ulong,
    },
    #[error("page index {page} out of bounds (document has {pages} pages)")]
    PageOutOfBounds { page: isize, pages: usize },
    #[error("year must be positive")]
    InvalidYear,
    #[error("page pixel dimensions are invalid or exceed PDFium bitmap limits")]
    Dimensions,
    #[error("failed to render rm strokes: {0}")]
    Strokes(#[from] rmscene::Error),
    #[error("failed to parse stroke SVG: {0}")]
    Svg(#[from] resvg::usvg::Error),
    #[error("failed to encode PNG: {0}")]
    Image(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, Error>;
