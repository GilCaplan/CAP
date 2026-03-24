/// Image processing via Python PIL/Pillow
use crate::error::{CapError, Span};
use crate::interpreter::value::Value;
use crate::interpreter::stdlib::sys::run_python;
use crate::interpreter::stdlib::json::{json_to_value, value_to_json};

pub const BUILTINS: &[&str] = &[
    "img_open", "img_save", "img_info",
    "img_resize", "img_crop", "img_rotate", "img_flip",
    "img_grayscale", "img_blur", "img_sharpen", "img_brightness", "img_contrast",
    "img_thumbnail", "img_convert",
    "img_pixels", "img_size", "img_mode",
    "img_draw_text", "img_paste",
    "img_from_array", "img_to_array",
    "img_show",
];

fn run_img(code: &str, span: &Span) -> Result<Value, CapError> {
    let wrapped = format!(
        "import json as _json, sys as _sys\n\
         def cap_return(__v): print(_json.dumps(__v)); _sys.exit(0)\n\
         {code}"
    );
    let out = run_python(&wrapped, None, span)?;
    if let Value::Str(s) = &out {
        if s.is_empty() { return Ok(Value::Null); }
        let j: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| CapError::Runtime { message: format!("img: invalid JSON: {e}"), span: span.clone() })?;
        json_to_value(j, span)
    } else { Ok(Value::Null) }
}

fn v2j(v: &Value, span: &Span) -> Result<String, CapError> {
    Ok(value_to_json(v, span)?.to_string())
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "img_open" => {
            // img_open(path) → {path, width, height, mode, format}
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image
img = Image.open("{path}")
cap_return({{"path": "{path}", "width": img.width, "height": img.height,
             "mode": img.mode, "format": img.format or ""}})
"#);
            run_img(&code, span)
        }
        "img_save" => {
            // img_save(src_path, dst_path, opts?) → dst_path
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let src = args.remove(0).as_str(span)?.to_string();
            let dst = args.remove(0).as_str(span)?.to_string();
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
from PIL import Image
import json
opts = json.loads('''{opts_json}''')
img = Image.open("{src}")
quality = opts.get("quality", 95)
img.save("{dst}", quality=quality)
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_info" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image
import os
img = Image.open("{path}")
size = os.path.getsize("{path}")
cap_return({{"path": "{path}", "width": img.width, "height": img.height,
             "mode": img.mode, "format": img.format or "",
             "size_bytes": size}})
"#);
            run_img(&code, span)
        }
        "img_size" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image
img = Image.open("{path}")
cap_return({{"width": img.width, "height": img.height}})
"#);
            run_img(&code, span)
        }
        "img_mode" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image
img = Image.open("{path}")
cap_return(img.mode)
"#);
            run_img(&code, span)
        }
        "img_resize" => {
            // img_resize(src, dst, width, height, resample?)
            if args.len() < 4 {
                return Err(CapError::TooFewArgs { expected: 4, got: args.len(), span: span.clone() });
            }
            let src = args.remove(0).as_str(span)?.to_string();
            let dst = args.remove(0).as_str(span)?.to_string();
            let w = match args.remove(0) { Value::Int(n) => n, _ => 256 };
            let h = match args.remove(0) { Value::Int(n) => n, _ => 256 };
            let resample = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { "LANCZOS".into() };
            let code = format!(r#"
from PIL import Image
img = Image.open("{src}")
img = img.resize(({w}, {h}), Image.Resampling.{resample})
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_crop" => {
            // img_crop(src, dst, left, top, right, bottom)
            if args.len() < 6 {
                return Err(CapError::TooFewArgs { expected: 6, got: args.len(), span: span.clone() });
            }
            let src  = args.remove(0).as_str(span)?.to_string();
            let dst  = args.remove(0).as_str(span)?.to_string();
            let left = match args.remove(0) { Value::Int(n) => n, _ => 0 };
            let top  = match args.remove(0) { Value::Int(n) => n, _ => 0 };
            let right  = match args.remove(0) { Value::Int(n) => n, _ => 100 };
            let bottom = match args.remove(0) { Value::Int(n) => n, _ => 100 };
            let code = format!(r#"
from PIL import Image
img = Image.open("{src}")
img = img.crop(({left}, {top}, {right}, {bottom}))
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_rotate" => {
            // img_rotate(src, dst, degrees, expand?)
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let src  = args.remove(0).as_str(span)?.to_string();
            let dst  = args.remove(0).as_str(span)?.to_string();
            let deg  = match args.remove(0) { Value::Int(n) => n as f64, Value::Float(f) => f, _ => 90.0 };
            let expand = matches!(args.first(), Some(Value::Bool(true)));
            let code = format!(r#"
from PIL import Image
img = Image.open("{src}")
img = img.rotate({deg}, expand={expand})
img.save("{dst}")
cap_return("{dst}")
"#, expand = if expand { "True" } else { "False" });
            run_img(&code, span)
        }
        "img_flip" => {
            // img_flip(src, dst, direction) — direction: "horizontal" or "vertical"
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let src = args.remove(0).as_str(span)?.to_string();
            let dst = args.remove(0).as_str(span)?.to_string();
            let dir = args.remove(0).as_str(span)?.to_string();
            let method = if dir.contains("vert") { "FLIP_TOP_BOTTOM" } else { "FLIP_LEFT_RIGHT" };
            let code = format!(r#"
from PIL import Image
img = Image.open("{src}")
img = img.transpose(Image.Transpose.{method})
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_grayscale" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let src = args.remove(0).as_str(span)?.to_string();
            let dst = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image
img = Image.open("{src}").convert("L")
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_blur" => {
            // img_blur(src, dst, radius?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let src = args.remove(0).as_str(span)?.to_string();
            let dst = args.remove(0).as_str(span)?.to_string();
            let radius = if !args.is_empty() {
                match args.remove(0) { Value::Int(n) => n as f64, Value::Float(f) => f, _ => 2.0 }
            } else { 2.0 };
            let code = format!(r#"
from PIL import Image, ImageFilter
img = Image.open("{src}")
img = img.filter(ImageFilter.GaussianBlur(radius={radius}))
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_sharpen" => {
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let src = args.remove(0).as_str(span)?.to_string();
            let dst = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image, ImageFilter
img = Image.open("{src}")
img = img.filter(ImageFilter.SHARPEN)
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_brightness" => {
            // img_brightness(src, dst, factor)  — 1.0 = original, 2.0 = double
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let src    = args.remove(0).as_str(span)?.to_string();
            let dst    = args.remove(0).as_str(span)?.to_string();
            let factor = match args.remove(0) { Value::Float(f) => f, Value::Int(n) => n as f64, _ => 1.0 };
            let code = format!(r#"
from PIL import Image, ImageEnhance
img = Image.open("{src}")
img = ImageEnhance.Brightness(img).enhance({factor})
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_contrast" => {
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let src    = args.remove(0).as_str(span)?.to_string();
            let dst    = args.remove(0).as_str(span)?.to_string();
            let factor = match args.remove(0) { Value::Float(f) => f, Value::Int(n) => n as f64, _ => 1.0 };
            let code = format!(r#"
from PIL import Image, ImageEnhance
img = Image.open("{src}")
img = ImageEnhance.Contrast(img).enhance({factor})
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_thumbnail" => {
            // img_thumbnail(src, dst, max_size)
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let src  = args.remove(0).as_str(span)?.to_string();
            let dst  = args.remove(0).as_str(span)?.to_string();
            let size = match args.remove(0) { Value::Int(n) => n, _ => 128 };
            let code = format!(r#"
from PIL import Image
img = Image.open("{src}")
img.thumbnail(({size}, {size}), Image.Resampling.LANCZOS)
img.save("{dst}")
cap_return({{"path": "{dst}", "width": img.width, "height": img.height}})
"#);
            run_img(&code, span)
        }
        "img_convert" => {
            // img_convert(src, dst, mode)  — mode: "RGB", "RGBA", "L", "P"
            if args.len() < 3 {
                return Err(CapError::TooFewArgs { expected: 3, got: args.len(), span: span.clone() });
            }
            let src  = args.remove(0).as_str(span)?.to_string();
            let dst  = args.remove(0).as_str(span)?.to_string();
            let mode = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image
img = Image.open("{src}").convert("{mode}")
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_pixels" => {
            // img_pixels(path) → list of [r, g, b] or [gray] values (flattened rows)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image
img = Image.open("{path}").convert("RGB")
pixels = list(img.getdata())
cap_return([list(p) for p in pixels])
"#);
            run_img(&code, span)
        }
        "img_from_array" => {
            // img_from_array(list_of_lists, dst_path, mode?)
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let arr_json = v2j(&args.remove(0), span)?;
            let dst  = args.remove(0).as_str(span)?.to_string();
            let mode = if !args.is_empty() { args.remove(0).as_str(span)?.to_string() } else { "RGB".into() };
            let code = format!(r#"
import json
import numpy as np
from PIL import Image
arr = json.loads('''{arr_json}''')
img = Image.fromarray(np.array(arr, dtype=np.uint8), mode="{mode}")
img.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_to_array" => {
            // img_to_array(path) → nested list (H x W x C)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
import numpy as np
from PIL import Image
arr = np.array(Image.open("{path}"))
cap_return(arr.tolist())
"#);
            run_img(&code, span)
        }
        "img_draw_text" => {
            // img_draw_text(src, dst, text, x, y, opts?)
            if args.len() < 5 {
                return Err(CapError::TooFewArgs { expected: 5, got: args.len(), span: span.clone() });
            }
            let src  = args.remove(0).as_str(span)?.to_string();
            let dst  = args.remove(0).as_str(span)?.to_string();
            let text = args.remove(0).as_str(span)?.to_string();
            let x    = match args.remove(0) { Value::Int(n) => n, _ => 10 };
            let y    = match args.remove(0) { Value::Int(n) => n, _ => 10 };
            let opts_json = if !args.is_empty() { v2j(&args[0], span)? } else { "{}".into() };
            let code = format!(r#"
import json
from PIL import Image, ImageDraw, ImageFont
opts = json.loads('''{opts_json}''')
img  = Image.open("{src}")
draw = ImageDraw.Draw(img)
color = opts.get("color", "white")
size  = opts.get("font_size", 20)
try:
    font = ImageFont.truetype(opts.get("font", ""), size)
except:
    font = ImageFont.load_default()
draw.text(({x}, {y}), {text_json}, fill=color, font=font)
img.save("{dst}")
cap_return("{dst}")
"#, text_json = serde_json::json!(text).to_string());
            run_img(&code, span)
        }
        "img_paste" => {
            // img_paste(base_path, overlay_path, dst, x, y)
            if args.len() < 5 {
                return Err(CapError::TooFewArgs { expected: 5, got: args.len(), span: span.clone() });
            }
            let base    = args.remove(0).as_str(span)?.to_string();
            let overlay = args.remove(0).as_str(span)?.to_string();
            let dst     = args.remove(0).as_str(span)?.to_string();
            let x = match args.remove(0) { Value::Int(n) => n, _ => 0 };
            let y = match args.remove(0) { Value::Int(n) => n, _ => 0 };
            let code = format!(r#"
from PIL import Image
base    = Image.open("{base}")
overlay = Image.open("{overlay}")
base.paste(overlay, ({x}, {y}), overlay if overlay.mode == "RGBA" else None)
base.save("{dst}")
cap_return("{dst}")
"#);
            run_img(&code, span)
        }
        "img_show" => {
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let path = args.remove(0).as_str(span)?.to_string();
            let code = format!(r#"
from PIL import Image
Image.open("{path}").show()
cap_return(True)
"#);
            run_img(&code, span)
        }
        _ => Err(CapError::Runtime { message: format!("unknown img builtin: {name}"), span: span.clone() }),
    }
}
