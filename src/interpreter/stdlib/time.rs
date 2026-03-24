use crate::error::{CapError, Span};
use crate::interpreter::value::{MapKey, Value};
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUILTINS: &[&str] = &[
    "time_now", "time_now_utc", "time_unix", "time_parse",
    "time_format", "time_add", "time_diff", "time_sleep",
    "time_year", "time_month", "time_day", "time_hour", "time_minute", "time_second",
    "time_weekday",
];

/// A time value is stored as a cap Map with keys: "unix" (i64 seconds) and "tz" ("local"|"utc")
fn make_time_map(unix: i64, tz: &str) -> Value {
    let mut map = IndexMap::new();
    map.insert(MapKey::Str("unix".into()), Value::Int(unix));
    map.insert(MapKey::Str("tz".into()), Value::Str(tz.to_string()));
    Value::Map(Rc::new(RefCell::new(map)))
}

fn get_unix(v: &Value, span: &Span) -> Result<i64, CapError> {
    match v {
        Value::Map(m) => {
            let b = m.borrow();
            match b.get(&MapKey::Str("unix".into())) {
                Some(Value::Int(n)) => Ok(*n),
                _ => Err(CapError::Runtime { message: "time map must have 'unix' key".into(), span: span.clone() }),
            }
        }
        Value::Int(n) => Ok(*n),
        _ => Err(CapError::TypeError { expected: "time map or int", got: v.type_name().to_string(), span: span.clone() }),
    }
}

pub fn call(name: &str, args: Vec<Value>, span: &Span) -> Result<Value, CapError> {
    let mut args = args;
    match name {
        "time_now" => {
            let unix = Local::now().timestamp();
            Ok(make_time_map(unix, "local"))
        }
        "time_now_utc" => {
            let unix = Utc::now().timestamp();
            Ok(make_time_map(unix, "utc"))
        }
        "time_unix" => {
            // time_unix(seconds) → time map
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let unix = match args.remove(0) {
                Value::Int(n) => n,
                Value::Float(f) => f as i64,
                other => return Err(CapError::TypeError { expected: "int", got: other.type_name().to_string(), span: span.clone() }),
            };
            Ok(make_time_map(unix, "utc"))
        }
        "time_parse" => {
            // time_parse(str, format) → time map
            // format uses strftime codes, e.g. "%Y-%m-%d" or "%Y-%m-%d %H:%M:%S"
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let s = args.remove(0).as_str(span)?.to_string();
            let fmt = args.remove(0).as_str(span)?.to_string();
            // Try full datetime-with-tz first, then naive datetime, then date-only
            let unix = if let Ok(dt) = DateTime::parse_from_str(&s, &fmt) {
                dt.timestamp()
            } else if let Ok(ndt) = NaiveDateTime::parse_from_str(&s, &fmt) {
                Utc.from_utc_datetime(&ndt).timestamp()
            } else if let Ok(nd) = NaiveDate::parse_from_str(&s, &fmt) {
                Utc.from_utc_datetime(&nd.and_hms_opt(0, 0, 0).unwrap()).timestamp()
            } else {
                return Err(CapError::Runtime {
                    message: format!("time_parse: cannot parse {s:?} with format {fmt:?}"),
                    span: span.clone(),
                });
            };
            Ok(make_time_map(unix, "utc"))
        }
        "time_format" => {
            // time_format(time_map, format_str) → str
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let unix = get_unix(&args[0], span)?;
            let fmt = args[1].as_str(span)?.to_string();
            let dt = Utc.timestamp_opt(unix, 0).single()
                .ok_or_else(|| CapError::Runtime { message: "invalid unix timestamp".into(), span: span.clone() })?;
            Ok(Value::Str(dt.format(&fmt).to_string()))
        }
        "time_add" => {
            // time_add(time_map, seconds) → time map
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let unix = get_unix(&args[0], span)?;
            let delta = match &args[1] {
                Value::Int(n) => *n,
                Value::Float(f) => *f as i64,
                other => return Err(CapError::TypeError { expected: "int", got: other.type_name().to_string(), span: span.clone() }),
            };
            Ok(make_time_map(unix + delta, "utc"))
        }
        "time_diff" => {
            // time_diff(t1, t2) → seconds (int)  [t1 - t2]
            if args.len() < 2 {
                return Err(CapError::TooFewArgs { expected: 2, got: args.len(), span: span.clone() });
            }
            let t1 = get_unix(&args[0], span)?;
            let t2 = get_unix(&args[1], span)?;
            Ok(Value::Int(t1 - t2))
        }
        "time_sleep" => {
            // time_sleep(seconds)
            if args.is_empty() {
                return Err(CapError::TooFewArgs { expected: 1, got: 0, span: span.clone() });
            }
            let secs = match args.remove(0) {
                Value::Int(n) => n as f64,
                Value::Float(f) => f,
                other => return Err(CapError::TypeError { expected: "number", got: other.type_name().to_string(), span: span.clone() }),
            };
            std::thread::sleep(std::time::Duration::from_secs_f64(secs));
            Ok(Value::Null)
        }
        "time_year" => {
            let unix = get_unix(&args.into_iter().next().unwrap_or(Value::Null), span)?;
            let dt = Utc.timestamp_opt(unix, 0).single()
                .ok_or_else(|| CapError::Runtime { message: "invalid timestamp".into(), span: span.clone() })?;
            Ok(Value::Int(dt.year() as i64))
        }
        "time_month" => {
            let unix = get_unix(&args.into_iter().next().unwrap_or(Value::Null), span)?;
            let dt = Utc.timestamp_opt(unix, 0).single()
                .ok_or_else(|| CapError::Runtime { message: "invalid timestamp".into(), span: span.clone() })?;
            Ok(Value::Int(dt.month() as i64))
        }
        "time_day" => {
            let unix = get_unix(&args.into_iter().next().unwrap_or(Value::Null), span)?;
            let dt = Utc.timestamp_opt(unix, 0).single()
                .ok_or_else(|| CapError::Runtime { message: "invalid timestamp".into(), span: span.clone() })?;
            Ok(Value::Int(dt.day() as i64))
        }
        "time_hour" => {
            let unix = get_unix(&args.into_iter().next().unwrap_or(Value::Null), span)?;
            let dt = Utc.timestamp_opt(unix, 0).single()
                .ok_or_else(|| CapError::Runtime { message: "invalid timestamp".into(), span: span.clone() })?;
            Ok(Value::Int(dt.hour() as i64))
        }
        "time_minute" => {
            let unix = get_unix(&args.into_iter().next().unwrap_or(Value::Null), span)?;
            let dt = Utc.timestamp_opt(unix, 0).single()
                .ok_or_else(|| CapError::Runtime { message: "invalid timestamp".into(), span: span.clone() })?;
            Ok(Value::Int(dt.minute() as i64))
        }
        "time_second" => {
            let unix = get_unix(&args.into_iter().next().unwrap_or(Value::Null), span)?;
            let dt = Utc.timestamp_opt(unix, 0).single()
                .ok_or_else(|| CapError::Runtime { message: "invalid timestamp".into(), span: span.clone() })?;
            Ok(Value::Int(dt.second() as i64))
        }
        "time_weekday" => {
            let unix = get_unix(&args.into_iter().next().unwrap_or(Value::Null), span)?;
            let dt = Utc.timestamp_opt(unix, 0).single()
                .ok_or_else(|| CapError::Runtime { message: "invalid timestamp".into(), span: span.clone() })?;
            use chrono::Weekday::*;
            let name = match dt.weekday() {
                Mon => "Monday", Tue => "Tuesday", Wed => "Wednesday",
                Thu => "Thursday", Fri => "Friday", Sat => "Saturday", Sun => "Sunday",
            };
            Ok(Value::Str(name.to_string()))
        }
        _ => Err(CapError::Runtime { message: format!("unknown time builtin: {name}"), span: span.clone() }),
    }
}
