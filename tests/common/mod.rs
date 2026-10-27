use std::fmt::Write;

pub fn pdf(objects: &[String]) -> Vec<u8> {
    let mut document = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(document.len());
        writeln!(document, "{} 0 obj\n{object}\nendobj", i + 1).unwrap();
    }
    let xref = document.len();
    writeln!(
        document,
        "xref\n0 {}\n0000000000 65535 f ",
        objects.len() + 1
    )
    .unwrap();
    for offset in offsets {
        writeln!(document, "{offset:010} 00000 n ").unwrap();
    }
    writeln!(
        document,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
        objects.len() + 1
    )
    .unwrap();
    document.into_bytes()
}

pub fn transparency_pdf() -> Vec<u8> {
    let content = "q /GS1 gs 1 0 0 rg 0 0 100 100 re f Q\n";
    pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Group << /S /Transparency /CS /DeviceRGB >> /Resources << /ExtGState << /GS1 5 0 R >> >> /Contents 4 0 R >>".into(),
        format!("<< /Length {} >>\nstream\n{content}endstream",content.len()),
        "<< /Type /ExtGState /BM /Screen /ca 0.5 /CA 0.5 >>".into(),
    ])
}
