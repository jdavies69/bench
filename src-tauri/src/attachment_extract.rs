//! Local text extraction only: never executes document code or resolves links.
use calamine::{Reader as WorkbookReader, Xlsx};
use quick_xml::{events::Event, Reader};
use std::{
    collections::HashSet,
    io::{Cursor, Read, Write},
};

const SOURCE: usize = 10 * 1024 * 1024;
const TEXT: usize = 256 * 1024;
const ENTRY: usize = 8 * 1024 * 1024;
const EXPANDED: usize = 20 * 1024 * 1024;
const INVALID: &str = "This document is damaged or its contents do not match its file type.";
const TOO_LARGE: &str = "This document expands beyond Bench's attachment limits.";
const PASSWORD: &str =
    "Password-protected or encrypted documents are not supported. Export an unlocked copy.";

pub struct ExtractedDocument {
    pub text: String,
    pub mime_type: String,
}
pub fn extract(name: &str, bytes: &[u8]) -> Result<ExtractedDocument, String> {
    if bytes.is_empty() || bytes.len() > SOURCE {
        return Err("Attach a nonempty document no larger than 10 MiB.".into());
    }
    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let (text, mime) = match extension.as_str() {
        "pdf" => (guard_pdf(|| pdf(bytes))?, "application/pdf"),
        "docx" => (
            docx(bytes)?,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        "xlsx" => (
            xlsx(bytes)?,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
        "xls" => {
            return Err(
                "Legacy XLS attachments are not supported yet. Save a copy as XLSX or CSV.".into(),
            )
        }
        _ => return Err("Choose a PDF, DOCX or XLSX document.".into()),
    };
    if text.trim().is_empty() {
        return Err("This document contains no readable text.".into());
    }
    Ok(ExtractedDocument {
        text,
        mime_type: mime.into(),
    })
}
fn append(target: &mut String, value: &str) -> Result<(), String> {
    if target.len().saturating_add(value.len()) > TEXT {
        return Err(
            "Extracted text exceeds 256 KiB. Attach a smaller document or selected pages.".into(),
        );
    }
    target.push_str(value);
    Ok(())
}
fn archive(bytes: &[u8]) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0]) {
            PASSWORD
        } else {
            INVALID
        }
        .into());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| INVALID)?;
    if zip.len() > 1000 {
        return Err(TOO_LARGE.into());
    }
    let mut total = 0usize;
    let mut names = HashSet::new();
    let mut files = std::collections::HashMap::new();
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|_| PASSWORD)?;
        if entry.encrypted() {
            return Err(PASSWORD.into());
        }
        if entry.enclosed_name().is_none()
            || entry.name().contains('\\')
            || !names.insert(entry.name().to_owned())
            || entry.is_symlink()
        {
            return Err(INVALID.into());
        }
        if entry.size() > ENTRY as u64 {
            return Err(TOO_LARGE.into());
        }
        let mut data = Vec::new();
        entry
            .by_ref()
            .take((ENTRY + 1) as u64)
            .read_to_end(&mut data)
            .map_err(|_| INVALID)?;
        total = total.checked_add(data.len()).ok_or(TOO_LARGE)?;
        if data.len() > ENTRY || total > EXPANDED {
            return Err(TOO_LARGE.into());
        }
        if !entry.is_dir() {
            files.insert(entry.name().to_owned(), data);
        }
    }
    if !files.contains_key("[Content_Types].xml") {
        return Err(INVALID.into());
    }
    Ok(files)
}
fn xml_text(xml: &[u8]) -> Result<String, String> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().check_end_names = true;
    let mut output = String::new();
    let mut inside_text = false;
    let mut depth = 0usize;
    let mut roots = 0usize;
    loop {
        match reader.read_event().map_err(|_| INVALID)? {
            Event::Start(event) => {
                if depth == 0 {
                    roots += 1;
                    if roots > 1 {
                        return Err(INVALID.into());
                    }
                }
                depth += 1;
                if depth > 128 {
                    return Err(TOO_LARGE.into());
                }
                if event.local_name().as_ref() == "t" {
                    inside_text = true;
                }
            }
            Event::End(event) => {
                depth = depth.checked_sub(1).ok_or(INVALID)?;
                if event.local_name().as_ref() == "t" {
                    inside_text = false;
                }
                if ["p", "tr"].contains(&event.local_name().as_ref()) {
                    append(&mut output, "\n")?;
                }
                if event.local_name().as_ref() == "tc" {
                    append(&mut output, "\t")?;
                }
            }
            Event::Text(event) if inside_text => {
                let decoded = event.xml10_content();
                append(
                    &mut output,
                    &quick_xml::escape::unescape(&decoded).map_err(|_| INVALID)?,
                )?;
            }
            Event::GeneralRef(event) if inside_text => {
                let name = event.xml10_content();
                let reference = format!("&{name};");
                append(
                    &mut output,
                    &quick_xml::escape::unescape(&reference).map_err(|_| INVALID)?,
                )?;
            }
            Event::Empty(_) if depth == 0 => {
                roots += 1;
                if roots > 1 {
                    return Err(INVALID.into());
                }
            }
            Event::DocType(_) => {
                return Err("Documents with XML entity declarations are not supported.".into())
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if depth != 0 || roots != 1 {
        return Err(INVALID.into());
    }
    Ok(output)
}
fn docx(bytes: &[u8]) -> Result<String, String> {
    let files = archive(bytes)?;
    let types = std::str::from_utf8(&files["[Content_Types].xml"]).map_err(|_| INVALID)?;
    if !types.contains(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
    ) || files
        .keys()
        .any(|name| name.to_ascii_lowercase().contains("vbaproject"))
    {
        return Err(INVALID.into());
    }
    for (name, xml) in &files {
        if name.ends_with(".xml") {
            xml_text(xml)?;
        }
    }
    xml_text(files.get("word/document.xml").ok_or(INVALID)?)
}
fn xlsx(bytes: &[u8]) -> Result<String, String> {
    let files = archive(bytes)?;
    let types = std::str::from_utf8(&files["[Content_Types].xml"]).map_err(|_| INVALID)?;
    if !types.contains("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml")
        || !files.contains_key("xl/workbook.xml")
        || files
            .keys()
            .any(|name| name.to_ascii_lowercase().contains("vbaproject"))
    {
        return Err(INVALID.into());
    }
    // Fully parse XML first to reject entities, excess nesting and malformed XML.
    for (name, xml) in &files {
        if name.ends_with(".xml") {
            xml_text(xml)?;
        }
    }
    let mut workbook = Xlsx::new(Cursor::new(bytes)).map_err(|_| INVALID)?;
    let names = workbook.sheet_names().to_vec();
    if names.len() > 100 {
        return Err(TOO_LARGE.into());
    }
    let mut output = String::new();
    let mut cells = 0usize;
    for name in names {
        append(&mut output, &format!("Sheet: {name}\n"))?;
        // Streaming used-cell reader avoids allocating huge sparse worksheet ranges.
        let mut reader = workbook
            .worksheet_cells_reader(&name)
            .map_err(|_| INVALID)?;
        while let Some(cell) = reader.next_cell().map_err(|_| INVALID)? {
            cells += 1;
            if cells > 100_000 {
                return Err(TOO_LARGE.into());
            }
            let (row, col) = cell.get_position();
            append(
                &mut output,
                &format!(
                    "R{}C{}\t{}\n",
                    u64::from(row) + 1,
                    u64::from(col) + 1,
                    calamine::Data::from(cell.get_value().clone())
                ),
            )?;
        }
    }
    Ok(output)
}
// Scan all name-like tokens, including binary/string false positives. Conservative
// rejection is intentional; it prevents compressed object loading before limits.
thread_local! { static PDF_PARSER_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
fn guard_pdf<T>(parse: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    static INSTALL_HOOK: std::sync::Once = std::sync::Once::new();
    INSTALL_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Some parser panics format source strings. Suppress only this
            // thread's document-parser panic; retain normal hooks everywhere else.
            if !PDF_PARSER_ACTIVE
                .try_with(|active| active.get())
                .unwrap_or(false)
            {
                previous(info);
            }
        }));
    });
    struct Active(bool);
    impl Drop for Active {
        fn drop(&mut self) {
            PDF_PARSER_ACTIVE.with(|flag| flag.set(self.0));
        }
    }
    let old = PDF_PARSER_ACTIVE.with(|flag| flag.replace(true));
    let _active = Active(old);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(parse))
        .map_err(|_| INVALID.to_string())?
}

fn pdf_admission(bytes: &[u8]) -> Result<(), String> {
    let mut index = 0usize;
    let mut nesting = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'[' | b'(' | b'<' => {
                nesting += 1;
                if nesting > 128 {
                    return Err(TOO_LARGE.into());
                }
            }
            b']' | b')' | b'>' => nesting = nesting.saturating_sub(1),
            b'/' => {
                index += 1;
                let mut name = Vec::new();
                while index < bytes.len()
                    && !bytes[index].is_ascii_whitespace()
                    && !b"()<>[]{}/%".contains(&bytes[index])
                {
                    if bytes[index] == b'#' {
                        let pair = bytes.get(index + 1..index + 3).ok_or(INVALID)?;
                        let value = std::str::from_utf8(pair)
                            .ok()
                            .and_then(|s| u8::from_str_radix(s, 16).ok())
                            .ok_or(INVALID)?;
                        name.push(value);
                        index += 3;
                    } else {
                        name.push(bytes[index]);
                        index += 1;
                    }
                    if name.len() > 256 {
                        return Err(INVALID.into());
                    }
                }
                match name.as_slice() {
                    b"Encrypt" => return Err(PASSWORD.into()),
                    b"ObjStm" | b"XRef" => return Err("This PDF uses compressed object tables that Bench cannot safely read yet. Export a simpler PDF or attach text/DOCX.".into()),
                    b"F" | b"FFilter" | b"FDecodeParms" => return Err("PDFs with external streams are not supported.".into()),
                    _ => {},
                }
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    Ok(())
}
fn bounded_pdf_streams(document: &mut pdf_extract::Document) -> Result<(), String> {
    use pdf_extract::Object;
    if document.objects.len() > 10_000 {
        return Err(TOO_LARGE.into());
    }
    let mut total = 0usize;
    for object in document.objects.values_mut() {
        if let Object::Stream(stream) = object {
            if stream.dict.has(b"DecodeParms") {
                return Err("PDF stream decoding options are not supported. Export a simpler PDF or attach text/DOCX.".into());
            }
            let compressed = match stream.dict.get(b"Filter") {
                Err(_) => false,
                Ok(Object::Name(name)) if name == b"FlateDecode" => true,
                Ok(Object::Array(filters)) if filters.len() == 1 && filters[0].as_name().is_ok_and(|name| name == b"FlateDecode") => true,
                _ => return Err("This PDF uses unsupported stream compression. Export a simpler PDF or attach text/DOCX.".into()),
            };
            if compressed {
                let mut decoder = flate2::read::ZlibDecoder::new(stream.content.as_slice());
                let mut decoded = Vec::new();
                decoder
                    .by_ref()
                    .take((ENTRY + 1) as u64)
                    .read_to_end(&mut decoded)
                    .map_err(|_| INVALID)?;
                if decoded.len() > ENTRY {
                    return Err(TOO_LARGE.into());
                }
                stream.content = decoded;
                stream.dict.remove(b"Filter");
                stream.dict.remove(b"DecodeParms");
                stream.dict.set("Length", stream.content.len() as i64);
            }
            if stream.content.len() > ENTRY {
                return Err(TOO_LARGE.into());
            }
            total = total.checked_add(stream.content.len()).ok_or(TOO_LARGE)?;
            if total > EXPANDED {
                return Err(TOO_LARGE.into());
            }
        }
    }
    Ok(())
}

struct BoundedOutput {
    bytes: Vec<u8>,
    overflow: bool,
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > TEXT {
            self.overflow = true;
            return Err(std::io::Error::other("attachment text limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn pdf(bytes: &[u8]) -> Result<String, String> {
    if !bytes.starts_with(b"%PDF-") {
        return Err(INVALID.into());
    }
    pdf_admission(bytes)?;
    let mut document = pdf_extract::Document::load_mem_with_options(
        bytes,
        pdf_extract::LoadOptions {
            strict: true,
            ..Default::default()
        },
    )
    .map_err(|_| INVALID)?;
    bounded_pdf_streams(&mut document)?;
    if document.is_encrypted() {
        return Err(PASSWORD.into());
    }
    if document.get_pages().len() > 500 {
        return Err(TOO_LARGE.into());
    }
    let mut writer = BoundedOutput {
        bytes: Vec::new(),
        overflow: false,
    };
    let result = {
        let writer_ref: &mut dyn Write = &mut writer;
        let mut output = pdf_extract::PlainTextOutput::new(writer_ref);
        pdf_extract::output_doc(&document, &mut output)
    };
    if writer.overflow {
        return Err(
            "Extracted text exceeds 256 KiB. Attach a smaller PDF or selected pages.".into(),
        );
    }
    result.map_err(|_| INVALID)?;
    let text = String::from_utf8(writer.bytes).map_err(|_| INVALID)?;
    if text.trim().is_empty() {
        return Err("This PDF has no readable text. Scanned or image-only PDFs require OCR, which Bench does not provide.".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            writer
                .start_file(
                    *name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    fn word(text: &str) -> Vec<u8> {
        let doc = format!("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:body></w:document>");
        package(&[("[Content_Types].xml", b"<Types><Override ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>"), ("word/document.xml", doc.as_bytes())])
    }
    fn sheet() -> Vec<u8> {
        package(&[
            ("[Content_Types].xml", b"<Types><Override ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/></Types>"),
            ("_rels/.rels", b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"),
            ("xl/workbook.xml", b"<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Budget\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"),
            ("xl/_rels/workbook.xml.rels", b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>"),
            ("xl/worksheets/sheet1.xml", b"<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>Rent</t></is></c><c r=\"B1\"><v>42</v></c></row></sheetData></worksheet>"),
        ])
    }
    fn pdf_fixture(text: bool) -> Vec<u8> {
        use pdf_extract::{dictionary, Document, Object, Stream};
        let mut doc = Document::with_version("1.5");
        doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        let pages = doc.new_object_id();
        let font = doc
            .add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
        let resources = doc.add_object(dictionary! {"Font"=>dictionary! {"F1"=>font}});
        let content = if text {
            b"BT /F1 12 Tf 10 10 Td (Hello PDF) Tj ET".to_vec()
        } else {
            Vec::new()
        };
        let stream = doc.add_object(Stream::new(dictionary! {}, content));
        let page = doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages,"Contents"=>stream,"Resources"=>resources,"MediaBox"=>vec![0.into(),0.into(),300.into(),300.into()]});
        doc.objects.insert(
            pages,
            Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
        );
        let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        doc.trailer.set("Root", root);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }
    #[test]
    fn extracts_real_minimal_word_workbook_and_pdf() {
        let word = extract("notes.DOCX", &word("Hello &amp; world")).unwrap();
        assert_eq!(word.text.trim(), "Hello & world");
        assert!(word.mime_type.contains("wordprocessingml"));
        let sheet = extract("budget.xlsx", &sheet()).unwrap();
        assert!(sheet.text.contains("Sheet: Budget"));
        assert!(sheet.text.contains("Rent"));
        assert!(sheet.text.contains("42"));
        let pdf = extract("paper.pdf", &pdf_fixture(true)).unwrap();
        assert!(pdf.text.contains("Hello PDF"));
    }
    #[test]
    fn rejects_malformed_wrong_type_encrypted_container_and_no_ocr() {
        for (name, data) in [
            ("bad.pdf", b"not a PDF".as_slice()),
            ("bad.docx", b"not a zip"),
            ("bad.xlsx", b"PK\x03\x04truncated"),
        ] {
            assert!(extract(name, data).is_err());
        }
        assert!(extract("wrong.xlsx", &word("hello")).is_err());
        assert!(extract("encrypted.docx", &[0xd0, 0xcf, 0x11, 0xe0, 0xa1])
            .err()
            .unwrap()
            .contains("Password"));
        assert!(extract("scan.pdf", &pdf_fixture(false))
            .err()
            .unwrap()
            .contains("OCR"));
        assert!(extract("old.xls", b"legacy")
            .err()
            .unwrap()
            .contains("XLSX or CSV"));
    }

    #[test]
    fn pdf_stream_expansion_object_tables_and_parser_panics_are_contained() {
        assert!(pdf_admission(b"%PDF-1.5 /Type /Obj#53tm").is_err());
        assert!(pdf_admission(b"%PDF-1.5 /Encr#79pt 1 0 R")
            .err()
            .unwrap()
            .contains("Password"));
        let mut document = pdf_extract::Document::load_mem(&pdf_fixture(true)).unwrap();
        let stream = document
            .objects
            .values_mut()
            .find_map(|object| match object {
                pdf_extract::Object::Stream(stream) => Some(stream),
                _ => None,
            })
            .unwrap();
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&stream.content).unwrap();
        stream.content = encoder.finish().unwrap();
        stream.dict.set("Length", stream.content.len() as i64);
        stream.dict.set("Filter", "FlateDecode");
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        assert!(extract("compressed.pdf", &bytes)
            .unwrap()
            .text
            .contains("Hello PDF"));
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&vec![b'a'; ENTRY + 1]).unwrap();
        let stream = document
            .objects
            .values_mut()
            .find_map(|object| match object {
                pdf_extract::Object::Stream(stream) => Some(stream),
                _ => None,
            })
            .unwrap();
        stream.content = encoder.finish().unwrap();
        assert!(bounded_pdf_streams(&mut document).is_err());
        let error: Result<(), String> =
            guard_pdf(|| panic!("private document content must never be logged"));
        assert_eq!(error.unwrap_err(), INVALID);
        assert!(!PDF_PARSER_ACTIVE.with(|active| active.get()));
        // Unsupported font encoding reaches a real pdf-extract panic path.
        let mut document = pdf_extract::Document::load_mem(&pdf_fixture(true)).unwrap();
        for object in document.objects.values_mut() {
            if let pdf_extract::Object::Dictionary(dict) = object {
                if dict.get(b"BaseFont").is_ok() {
                    dict.set(
                        "Encoding",
                        pdf_extract::Object::Name(b"UnsupportedTestEncoding".to_vec()),
                    );
                }
            }
        }
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        assert!(extract("malformed-font.pdf", &bytes).is_err());
    }
    #[test]
    fn zip_bombs_paths_macros_and_xml_entities_fail_before_activation() {
        let bomb = package(&[("huge", &vec![b'a'; ENTRY + 1])]);
        assert!(archive(&bomb).is_err());
        let seven_mb = vec![b'a'; 7 * 1024 * 1024];
        assert!(archive(&package(&[
            ("one", &seven_mb),
            ("two", &seven_mb),
            ("three", &seven_mb)
        ]))
        .is_err());
        assert!(xml_text(b"<root/><another/>").is_err());
        let nested = format!("{}{}", "<r>".repeat(129), "</r>".repeat(129));
        assert!(xml_text(nested.as_bytes()).is_err());
        assert!(archive(&package(&[("../escape", b"x")])).is_err());
        assert!(xml_text(b"<!DOCTYPE t [<!ENTITY bomb 'boom'>]><t>&bomb;</t>").is_err());
        assert!(xml_text(b"<t>unclosed").is_err());
        assert!(extract("huge.docx", &word(&"a".repeat(TEXT + 1))).is_err());
        assert!(extract("large.pdf", &vec![b'a'; SOURCE + 1]).is_err());
        let repeated = (0..1001)
            .map(|i| (format!("{i}"), vec![]))
            .collect::<Vec<_>>();
        let entries = repeated
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice()))
            .collect::<Vec<_>>();
        assert!(archive(&package(&entries)).is_err());
    }
}
