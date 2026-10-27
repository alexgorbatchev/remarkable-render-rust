//! The native boundary. Every PDFium call runs under ENGINE's process-wide lock.
//! Document buffers outlive native documents, and guards close dependent handles
//! before their parent or the engine lock is released. Bindings stay loaded for
//! the process lifetime, so initialization happens exactly once.

use crate::{Error, PageLink, Rect, Result};
use image::RgbaImage;
use pdfium_render::prelude::{
    FPDF_ACTION, FPDF_BITMAP, FPDF_DEST, FPDF_DOCUMENT, FPDF_LINK, FPDF_PAGE, FPDF_TEXTPAGE,
    FS_RECTF, Pdfium, PdfiumLibraryBindings,
};
use sha2::{Digest, Sha256};
use std::{
    ptr::{self, NonNull},
    sync::{Mutex, MutexGuard, OnceLock},
};

type Bindings = Box<dyn PdfiumLibraryBindings>;

#[cfg(test)]
#[path = "../tests/native/mod.rs"]
mod tests;
static ENGINE: OnceLock<Mutex<Bindings>> = OnceLock::new();
static INITIALIZE: Mutex<()> = Mutex::new(());

fn engine() -> Result<MutexGuard<'static, Bindings>> {
    if ENGINE.get().is_none() {
        let _init = INITIALIZE.lock().unwrap_or_else(|e| e.into_inner());
        if ENGINE.get().is_none() {
            let path = pdfium_bundled::ensure_pdfium_library(None)
                .map_err(|e| Error::Initialization(e.to_string()))?;
            verify_library(
                &std::fs::read(&path)?,
                pdfium_bundled::pinned_library_sha256()
                    .map_err(|e| Error::Initialization(e.to_string()))?,
            )?;
            let bindings =
                Pdfium::bind_to_library(&path).map_err(|e| Error::Initialization(e.to_string()))?;
            // SAFETY: The initialization lock prevents concurrent initialization;
            // bindings have just loaded a verified library and remain alive forever.
            unsafe {
                bindings.FPDF_InitLibrary();
            }
            let _ = ENGINE.set(Mutex::new(bindings));
        }
    }
    Ok(ENGINE
        .get()
        .expect("initialized above")
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}

fn verify_library(bytes: &[u8], expected: &str) -> Result<()> {
    let digest: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if digest == expected {
        Ok(())
    } else {
        Err(Error::LibraryDigest)
    }
}

fn failure(bindings: &dyn PdfiumLibraryBindings, operation: &'static str) -> Error {
    // SAFETY: Caller holds the engine lock, and the library is initialized.
    let code = unsafe { bindings.FPDF_GetLastError() };
    Error::Pdfium { operation, code }
}

pub(crate) struct NativeDocument {
    handle: FPDF_DOCUMENT,
    _bytes: Vec<u8>,
}

// SAFETY: Every access to this native handle, including Drop, holds ENGINE's
// mutex. Moving the Vec leaves its allocation stable; no native pointer escapes.
unsafe impl Send for NativeDocument {}
// SAFETY: Shared access is serialized by the same process-wide mutex.
unsafe impl Sync for NativeDocument {}

impl NativeDocument {
    pub(crate) fn open(bytes: Vec<u8>) -> Result<Self> {
        let bindings = engine()?;
        // SAFETY: bytes remains owned by this object until after CloseDocument.
        let raw = unsafe { bindings.FPDF_LoadMemDocument64(&bytes, None) };
        let handle =
            NonNull::new(raw).ok_or_else(|| failure(bindings.as_ref(), "open document"))?;
        Ok(Self {
            handle: handle.as_ptr(),
            _bytes: bytes,
        })
    }

    pub(crate) fn num_pages(&self) -> Result<usize> {
        let bindings = engine()?;
        // SAFETY: self owns a live document; engine lock serializes access.
        let count = unsafe { bindings.FPDF_GetPageCount(self.handle) };
        usize::try_from(count).map_err(|_| failure(bindings.as_ref(), "count pages"))
    }

    pub(crate) fn text(&self, index: i32) -> Result<String> {
        let bindings = engine()?;
        let page = Page::load(bindings.as_ref(), self.handle, index)?;
        let text = Text::load(&page)?;
        // SAFETY: Both text and page handles remain live under engine lock.
        let count = unsafe { bindings.FPDFText_CountChars(text.handle) };
        if count < 0 {
            return Err(failure(bindings.as_ref(), "count characters"));
        }
        if count == 0 {
            return Ok(String::new());
        }
        // PDFium can return surrogate pairs: reserve two UTF-16 units per glyph.
        let mut buffer = vec![0u16; count as usize * 2 + 1];
        // SAFETY: Buffer exceeds the documented count+1 capacity requirement.
        let written =
            unsafe { bindings.FPDFText_GetText(text.handle, 0, count, buffer.as_mut_ptr()) };
        decode_text(&buffer, written)
    }

    pub(crate) fn links(&self, index: i32) -> Result<Vec<PageLink>> {
        let bindings = engine()?;
        let page = Page::load(bindings.as_ref(), self.handle, index)?;
        let text = Text::load(&page)?;
        let mut position = 0;
        let mut links = Vec::new();
        loop {
            let mut link: FPDF_LINK = ptr::null_mut();
            // SAFETY: Live page; position and link are valid output pointers.
            let found =
                unsafe { bindings.FPDFLink_Enumerate(page.handle, &mut position, &mut link) };
            if found == 0 {
                break;
            }
            let (target_page, uri) = resolve_link(bindings.as_ref(), self.handle, link)?;
            let mut bounds = FS_RECTF {
                left: 0.,
                top: 0.,
                right: 0.,
                bottom: 0.,
            };
            // SAFETY: The link belongs to the live page and bounds is writable.
            unsafe {
                bindings.FPDFLink_GetAnnotRect(link, &mut bounds);
            }
            let rect = Rect {
                left: bounds.left.min(bounds.right) as f64,
                right: bounds.left.max(bounds.right) as f64,
                bottom: bounds.bottom.min(bounds.top) as f64,
                top: bounds.bottom.max(bounds.top) as f64,
            };
            let label = text.bounded(rect)?;
            links.push(PageLink {
                index: links.len(),
                target_page,
                uri,
                rect,
                text: label,
            });
        }
        Ok(links)
    }

    pub(crate) fn render(&self, index: i32, dpi: i32) -> Result<(RgbaImage, f64, f64)> {
        let bindings = engine()?;
        let page = Page::load(bindings.as_ref(), self.handle, index)?;
        // SAFETY: The loaded page is live for the duration of rendering.
        let (width_pt, height_pt) = unsafe {
            (
                bindings.FPDF_GetPageWidthF(page.handle) as f64,
                bindings.FPDF_GetPageHeightF(page.handle) as f64,
            )
        };
        let (width, height) = pixel_size(width_pt, height_pt, dpi)?;
        // SAFETY: Valid positive dimensions with checked byte size. PDFium owns
        // the buffer and Bitmap's destructor frees it before releasing the lock.
        let handle = unsafe { bindings.FPDFBitmap_Create(width, height, 1) };
        if handle.is_null() {
            return Err(failure(bindings.as_ref(), "create bitmap"));
        }
        let bitmap = Bitmap {
            bindings: bindings.as_ref(),
            handle,
        };
        // SAFETY: All handles are live and locked; output fits the allocated bitmap.
        unsafe {
            let fill = if bindings.FPDFPage_HasTransparency(page.handle) != 0 {
                0
            } else {
                0xffffffff
            };
            bindings.FPDFBitmap_FillRect(bitmap.handle, 0, 0, width, height, fill);
            // FPDF_REVERSE_BYTE_ORDER: native BGRA becomes RGBA, no annotations.
            bindings.FPDF_RenderPageBitmap(
                bitmap.handle,
                page.handle,
                0,
                0,
                width,
                height,
                0,
                0x10,
            );
        }
        // SAFETY: PDFium created a tightly packed four-byte bitmap. Copy while
        // bitmap and engine lock remain live; no foreign buffer escapes.
        let pixels = unsafe { bindings.FPDFBitmap_GetBuffer_as_vec(bitmap.handle) };
        let image =
            RgbaImage::from_raw(width as u32, height as u32, pixels).ok_or(Error::Dimensions)?;
        Ok((image, width_pt, height_pt))
    }
}

impl Drop for NativeDocument {
    fn drop(&mut self) {
        // A live document implies a successfully initialized engine; Drop never
        // reinitializes or downloads. Recover poisoning to release the resource.
        let bindings = ENGINE
            .get()
            .expect("live document has an engine")
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // SAFETY: This is the sole owner and its backing Vec is still alive.
        unsafe {
            bindings.FPDF_CloseDocument(self.handle);
        }
    }
}

struct Page<'a> {
    bindings: &'a dyn PdfiumLibraryBindings,
    handle: FPDF_PAGE,
}
impl<'a> Page<'a> {
    fn load(
        bindings: &'a dyn PdfiumLibraryBindings,
        doc: FPDF_DOCUMENT,
        index: i32,
    ) -> Result<Self> {
        // SAFETY: Caller holds lock and owns doc, index was checked by Document.
        let handle = unsafe { bindings.FPDF_LoadPage(doc, index) };
        if handle.is_null() {
            Err(failure(bindings, "load page"))
        } else {
            Ok(Self { bindings, handle })
        }
    }
}
impl Drop for Page<'_> {
    fn drop(&mut self) {
        // SAFETY: Dependent text handles have dropped; lock and parent remain live.
        unsafe {
            self.bindings.FPDF_ClosePage(self.handle);
        }
    }
}
struct Text<'a> {
    bindings: &'a dyn PdfiumLibraryBindings,
    handle: FPDF_TEXTPAGE,
}
impl<'a> Text<'a> {
    fn load(page: &'a Page<'a>) -> Result<Self> {
        // SAFETY: Caller retains the page guard and lock until this guard drops.
        let handle = unsafe { page.bindings.FPDFText_LoadPage(page.handle) };
        if handle.is_null() {
            Err(failure(page.bindings, "load text"))
        } else {
            Ok(Self {
                bindings: page.bindings,
                handle,
            })
        }
    }
    fn bounded(&self, rect: Rect) -> Result<String> {
        // SAFETY: Live text page and lock; null output requests UTF-16 length.
        let count = unsafe {
            self.bindings.FPDFText_GetBoundedText(
                self.handle,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                ptr::null_mut(),
                0,
            )
        };
        if count <= 0 {
            return Ok(String::new());
        }
        let capacity = count.checked_add(1).ok_or(Error::Dimensions)?;
        let mut buffer = vec![0u16; capacity as usize];
        // SAFETY: Buffer holds every UTF-16 unit plus the terminating NUL.
        let written = unsafe {
            self.bindings.FPDFText_GetBoundedText(
                self.handle,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                buffer.as_mut_ptr(),
                capacity,
            )
        };
        Ok(decode_text(&buffer, written)?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "))
    }
}
impl Drop for Text<'_> {
    fn drop(&mut self) {
        // SAFETY: This guard owns the text handle; its page and lock remain live.
        unsafe {
            self.bindings.FPDFText_ClosePage(self.handle);
        }
    }
}
struct Bitmap<'a> {
    bindings: &'a dyn PdfiumLibraryBindings,
    handle: FPDF_BITMAP,
}
impl Drop for Bitmap<'_> {
    fn drop(&mut self) {
        // SAFETY: This guard exclusively owns the bitmap under the engine lock.
        unsafe {
            self.bindings.FPDFBitmap_Destroy(self.handle);
        }
    }
}

fn decode_text(buffer: &[u16], written: i32) -> Result<String> {
    let count = usize::try_from(written)
        .ok()
        .filter(|&n| n > 0 && n <= buffer.len())
        .ok_or(Error::Pdfium {
            operation: "read text",
            code: 0,
        })?;
    let text = &buffer[..count];
    let text = text.strip_suffix(&[0]).unwrap_or(text);
    Ok(String::from_utf16_lossy(text))
}

fn resolve_link(
    bindings: &dyn PdfiumLibraryBindings,
    doc: FPDF_DOCUMENT,
    link: FPDF_LINK,
) -> Result<(Option<usize>, Vec<u8>)> {
    // SAFETY: Caller holds lock, doc/page/link are live for this operation.
    let action = unsafe { bindings.FPDFLink_GetAction(link) };
    if !action.is_null() {
        // SAFETY: Non-null action belongs to the live link.
        match unsafe { bindings.FPDFAction_GetType(action) } {
            1 => {
                // SAFETY: Action type is GoTo and document remains live.
                let dest = unsafe { bindings.FPDFAction_GetDest(doc, action) };
                Ok(destination(bindings, doc, dest))
            }
            3 => Ok((None, uri_bytes(bindings, doc, action)?)),
            _ => Ok((None, Vec::new())),
        }
    } else {
        // SAFETY: Only links without an action use their direct destination.
        let dest = unsafe { bindings.FPDFLink_GetDest(doc, link) };
        Ok(destination(bindings, doc, dest))
    }
}

fn destination(
    bindings: &dyn PdfiumLibraryBindings,
    doc: FPDF_DOCUMENT,
    dest: FPDF_DEST,
) -> (Option<usize>, Vec<u8>) {
    if dest.is_null() {
        return (None, Vec::new());
    }
    // SAFETY: Non-null destination and its document are live under engine lock.
    let page = unsafe { bindings.FPDFDest_GetDestPageIndex(doc, dest) };
    match usize::try_from(page) {
        Ok(page) => (Some(page), format!("#page={}", page + 1).into_bytes()),
        Err(_) => (None, Vec::new()),
    }
}

fn uri_bytes(
    bindings: &dyn PdfiumLibraryBindings,
    doc: FPDF_DOCUMENT,
    action: FPDF_ACTION,
) -> Result<Vec<u8>> {
    // SAFETY: A URI action in a live document; null output queries length.
    let count = unsafe { bindings.FPDFAction_GetURIPath(doc, action, ptr::null_mut(), 0) };
    if count == 0 {
        return Err(failure(bindings, "read URI length"));
    }
    let mut buffer = vec![0u8; count as usize];
    // SAFETY: Buffer matches PDFium's requested length, including terminator.
    let written =
        unsafe { bindings.FPDFAction_GetURIPath(doc, action, buffer.as_mut_ptr().cast(), count) };
    if written != count {
        return Err(failure(bindings, "read URI"));
    }
    // Keep raw bytes, including invalid UTF-8 and embedded NULs; only remove
    // PDFium's final terminator, never decode through a UTF-8/C-string helper.
    if buffer.last() == Some(&0) {
        buffer.pop();
    }
    Ok(buffer)
}

fn pixel_size(width: f64, height: f64, dpi: i32) -> Result<(i32, i32)> {
    let width = (width * f64::from(dpi) / 72.).ceil();
    let height = (height * f64::from(dpi) / 72.).ceil();
    if !width.is_finite()
        || !height.is_finite()
        || width < 1.
        || height < 1.
        || width > f64::from(i32::MAX / 4)
        || height > f64::from(i32::MAX)
        || width * height * 4. > f64::from(u32::MAX)
    {
        return Err(Error::Dimensions);
    }
    Ok((width as i32, height as i32))
}
