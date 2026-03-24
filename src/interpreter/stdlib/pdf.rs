/// PDF reading/writing via Python (pdfplumber, pypdf, reportlab)
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    "pdf_pages", "pdf_text", "pdf_page_text",
    "pdf_tables", "pdf_metadata",
    "pdf_images", "pdf_create", "pdf_merge",
];

fn run_pdf(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("pdf: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "pdf_pages" => {
            // pdf_pages(path) → number of pages
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
try:
    import pdfplumber
    with pdfplumber.open("{path}") as pdf:
        cap_return(len(pdf.pages))
except ImportError:
    from pypdf import PdfReader
    cap_return(len(PdfReader("{path}").pages))
"#);
            run_pdf(&code, span)
        }
        "pdf_text" => {
            // pdf_text(path) → full text string (all pages)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
try:
    import pdfplumber
    with pdfplumber.open("{path}") as pdf:
        text = "\n".join(p.extract_text() or "" for p in pdf.pages)
    cap_return(text)
except ImportError:
    from pypdf import PdfReader
    reader = PdfReader("{path}")
    text = "\n".join(p.extract_text() or "" for p in reader.pages)
    cap_return(text)
"#);
            run_pdf(&code, span)
        }
        "pdf_page_text" => {
            // pdf_page_text(path, page_num) → text of specific page (0-indexed)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let page = match args.remove(0) { Value::Int(n) => n, _ => 0 };
            let code = format!(r#"
try:
    import pdfplumber
    with pdfplumber.open("{path}") as pdf:
        cap_return(pdf.pages[{page}].extract_text() or "")
except ImportError:
    from pypdf import PdfReader
    cap_return(PdfReader("{path}").pages[{page}].extract_text() or "")
"#);
            run_pdf(&code, span)
        }
        "pdf_tables" => {
            // pdf_tables(path, page?) → list of tables (each table is list of rows)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let page_opt = if !args.is_empty() {
                match args.remove(0) { Value::Int(n) => format!("pages=[{}]", n), _ => String::new() }
            } else { String::new() };
            let code = format!(r#"
import pdfplumber
all_tables = []
with pdfplumber.open("{path}") as pdf:
    pages = pdf.pages
    for pg in pages:
        for tbl in (pg.extract_tables() or []):
            all_tables.append(tbl)
cap_return(all_tables)
"#);
            run_pdf(&code, span)
        }
        "pdf_metadata" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
try:
    import pdfplumber
    with pdfplumber.open("{path}") as pdf:
        meta = dict(pdf.metadata or {{}})
        meta["pages"] = len(pdf.pages)
        cap_return({{k: str(v) for k, v in meta.items()}})
except ImportError:
    from pypdf import PdfReader
    r = PdfReader("{path}")
    meta = dict(r.metadata or {{}})
    meta["pages"] = len(r.pages)
    cap_return({{k: str(v) for k, v in meta.items()}})
"#);
            run_pdf(&code, span)
        }
        "pdf_images" => {
            // pdf_images(path, output_dir?) → list of saved image paths
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path    = args.remove(0).as_str(span)?.to_string();
            let out_dir = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { "/tmp".into() };
            let code = format!(r#"
import pdfplumber, os
saved = []
with pdfplumber.open("{path}") as pdf:
    for i, page in enumerate(pdf.pages):
        for j, img in enumerate(page.images):
            # Extract image bytes if available
            try:
                from PIL import Image
                import io
                data = img.get("stream", b"")
                if data:
                    img_path = "{out_dir}/pdf_img_p{{i}}_{{j}}.png"
                    Image.open(io.BytesIO(data)).save(img_path)
                    saved.append(img_path)
            except Exception:
                pass
cap_return(saved)
"#);
            run_pdf(&code, span)
        }
        "pdf_create" => {
            // pdf_create(output_path, pages_list)
            // pages_list: list of strings (text content per page)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let output = args.remove(0).as_str(span)?.to_string();
            let pages_json = value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import json
from reportlab.pdfgen import canvas
from reportlab.lib.pagesizes import A4
pages = json.loads('''{pages_json}''')
c = canvas.Canvas("{output}", pagesize=A4)
w, h = A4
for text in pages:
    y = h - 50
    for line in str(text).split("\n"):
        c.drawString(50, y, line[:100])
        y -= 15
        if y < 50: break
    c.showPage()
c.save()
cap_return("{output}")
"#);
            run_pdf(&code, span)
        }
        "pdf_merge" => {
            // pdf_merge(output_path, [input_paths...])
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let output     = args.remove(0).as_str(span)?.to_string();
            let paths_json = value_to_json(&args[0], span)?.to_string();
            let code = format!(r#"
import json
from pypdf import PdfMerger
paths = json.loads('''{paths_json}''')
merger = PdfMerger()
for p in paths:
    merger.append(p)
merger.write("{output}")
merger.close()
cap_return("{output}")
"#);
            run_pdf(&code, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown pdf builtin: {name}"), span: span.clone() }),
    }
}
