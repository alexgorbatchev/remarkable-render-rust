use crate::{Error, Result, native};
use std::{io::Read, ops::Deref, path::Path};

/// Link bounds in default PDF user space (points, with y increasing upwards).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
    pub top: f64,
}

/// One link annotation, in PDFium enumeration order.
#[derive(Debug, Clone, PartialEq)]
pub struct PageLink {
    pub index: usize,
    /// Zero-based internal destination; URI, remote, and unresolved links have none.
    pub target_page: Option<usize>,
    /// PDFium's raw URI bytes. Internal destinations use `#page=N` (one-based).
    /// Catalog URI bases are concatenated by PDFium, without RFC URL resolution.
    pub uri: Vec<u8>,
    pub rect: Rect,
    /// Bounded PDF text, with Unicode whitespace collapsed and trimmed.
    pub text: String,
}

/// An owned PDF document. Native handles and their backing bytes close together.
/// Calls across all documents are serialized because PDFium is not thread safe.
pub struct Document {
    pub(crate) native: native::NativeDocument,
    pages: usize,
}

impl Document {
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Result<Self> {
        let native = native::NativeDocument::open(bytes.into())?;
        let pages = native.num_pages()?;
        Ok(Self { native, pages })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_bytes(std::fs::read(path)?)
    }

    pub fn from_reader(mut reader: impl Read) -> Result<Self> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Self::from_bytes(bytes)
    }

    pub fn num_pages(&self) -> usize {
        self.pages
    }

    pub fn text(&self, page: isize) -> Result<String> {
        self.native.text(self.page_index(page)?)
    }

    pub fn links(&self, page: isize) -> Result<Vec<PageLink>> {
        self.native.links(self.page_index(page)?)
    }

    pub(crate) fn page_index(&self, page: isize) -> Result<i32> {
        if page < 0 || page as usize >= self.pages {
            Err(Error::PageOutOfBounds {
                page,
                pages: self.pages,
            })
        } else {
            Ok(page as i32)
        }
    }
}

/// Typed alternatives to a borrowed open document, PDF bytes, a path, or a reader.
pub enum DocumentSource<'a> {
    Document(&'a Document),
    Bytes(&'a [u8]),
    Path(&'a Path),
    Reader(&'a mut dyn Read),
}

impl<'a> From<&'a Document> for DocumentSource<'a> {
    fn from(value: &'a Document) -> Self {
        Self::Document(value)
    }
}
impl<'a> From<&'a [u8]> for DocumentSource<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::Bytes(value)
    }
}
impl<'a> From<&'a Vec<u8>> for DocumentSource<'a> {
    fn from(value: &'a Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}
impl<'a> From<&'a Path> for DocumentSource<'a> {
    fn from(value: &'a Path) -> Self {
        Self::Path(value)
    }
}
impl<'a> From<&'a str> for DocumentSource<'a> {
    fn from(value: &'a str) -> Self {
        Self::Path(Path::new(value))
    }
}

pub(crate) enum ResolvedDocument<'a> {
    Borrowed(&'a Document),
    Owned(Document),
}
impl Deref for ResolvedDocument<'_> {
    type Target = Document;
    fn deref(&self) -> &Document {
        match self {
            Self::Borrowed(doc) => doc,
            Self::Owned(doc) => doc,
        }
    }
}
impl<'a> DocumentSource<'a> {
    pub(crate) fn resolve(self) -> Result<ResolvedDocument<'a>> {
        Ok(match self {
            Self::Document(doc) => ResolvedDocument::Borrowed(doc),
            Self::Bytes(bytes) => ResolvedDocument::Owned(Document::from_bytes(bytes)?),
            Self::Path(path) => ResolvedDocument::Owned(Document::from_path(path)?),
            Self::Reader(reader) => ResolvedDocument::Owned(Document::from_reader(reader)?),
        })
    }
}
