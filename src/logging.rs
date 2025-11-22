use std::env;
#[cfg(not(debug_assertions))]
use std::sync::atomic::AtomicUsize;

use backtrace::Backtrace;
use lazy_static::lazy_static;

use env_logger::fmt::{Color, Style, StyledValue};
use log::{Level, LevelFilter};

#[cfg(debug_assertions)]
lazy_static! {
    static ref BT: Backtrace = Backtrace::new();
}

#[cfg(debug_assertions)]
fn get_backtrace_functions() -> Vec<String> {
    let bt = Backtrace::new();
    bt.frames()
        .iter()
        .flat_map(|frame| frame.symbols())
        .filter_map(|symbol| {
            let name = symbol.name()?.to_string();
            if name.contains("gristmill") {
                let filename = symbol
                    .filename()
                    .map(|p| {
                        let s = p.display().to_string();
                        if let Some(idx) = s.rfind("src/") {
                            s.clone()[idx..].to_string()
                        } else {
                            s.clone()
                        }
                    })
                    .unwrap_or("<unknown>".to_string());

                Some(format!(
                    "{name} ({}:{})",
                    filename,
                    symbol.lineno().unwrap_or(0)
                ))
            } else {
                None
            }
        })
        .collect()
}

pub fn setup_logging() {
    #[cfg(debug_assertions)]
    let _ = BT;

    pretty_env_logger::formatted_builder()
        .filter_level({
            // There is multiple logging levels in highest priority to lowest
            // error
            // warn
            // info
            // debug
            // trace
            // off (no logs)
            match env::var("RUST_LOG").unwrap_or("info".to_string()).as_str() {
                "error" => LevelFilter::Error,
                "warn" => LevelFilter::Warn,
                "info" => LevelFilter::Info,
                "debug" => LevelFilter::Debug,
                "trace" => LevelFilter::Trace,
                "off" => LevelFilter::Off,
                _ => LevelFilter::Info,
            }
        })
        .format(|f, record| {
            use std::io::Write;

            let mut style = f.style();
            let level = colored_level(&mut style, record.level());

            #[cfg(not(debug_assertions))]
            {
                let target = record.target();
                let max_width = max_target_width(target);

                let mut style = f.style();
                let target = style.set_bold(true).value(Padded {
                    value: target,
                    width: max_width,
                });

                let mut style = f.style();
                let file_line = style.set_bold(true).value(format!(
                    "{}:{}",
                    record.file().unwrap(),
                    record.line().unwrap()
                ));

                return writeln!(
                    f,
                    "[{}] {} ({}) ~ {}",
                    level,
                    target,
                    file_line,
                    record.args(),
                );
            }

            #[cfg(debug_assertions)]
            {
                let function_names = get_backtrace_functions();
                let mut style = f.style();
                let function_name =
                    style
                        .set_bold(true)
                        .value(if let Some(v) = function_names.get(2) {
                            if v.contains("debug_callback") {
                                function_names
                                    .get(3)
                                    .unwrap_or(
                                        &function_names
                                            .last()
                                            .unwrap_or(&"<unknown>".to_string())
                                            .to_string(),
                                    )
                                    .clone()
                            } else {
                                v.clone()
                            }
                        } else {
                            function_names
                                .last()
                                .unwrap_or(&"<unknown>".to_string())
                                .to_string()
                        });

                writeln!(f, "[{}] {} ~ {}", level, function_name, record.args())
            }
        })
        .init();
}

// https://github.com/seanmonstar/pretty-env-logger
// This is all yoinked from https://docs.rs/pretty_env_logger/0.5.0/src/pretty_env_logger/lib.rs.html

#[cfg(not(debug_assertions))]
struct Padded<T> {
    value: T,
    width: usize,
}

#[cfg(not(debug_assertions))]
impl<T: fmt::Display> fmt::Display for Padded<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{: <width$}", self.value, width = self.width)
    }
}

#[cfg(not(debug_assertions))]
static MAX_MODULE_WIDTH: AtomicUsize = AtomicUsize::new(0);

#[cfg(not(debug_assertions))]
fn max_target_width(target: &str) -> usize {
    use std::sync::atomic::Ordering;

    let max_width = MAX_MODULE_WIDTH.load(Ordering::Relaxed);
    if max_width < target.len() {
        MAX_MODULE_WIDTH.store(target.len(), Ordering::Relaxed);
        target.len()
    } else {
        max_width
    }
}

fn colored_level<'a>(style: &'a mut Style, level: Level) -> StyledValue<'a, &'static str> {
    match level {
        Level::Trace => style.set_color(Color::Magenta).value("TRACE"),
        Level::Debug => style.set_color(Color::Blue).value("DEBUG"),
        Level::Info => style.set_color(Color::Green).value("INFO"),
        Level::Warn => style.set_color(Color::Yellow).value("WARN"),
        Level::Error => style.set_color(Color::Red).value("ERROR"),
    }
}
