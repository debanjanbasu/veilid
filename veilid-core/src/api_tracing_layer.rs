use crate::core_context::*;
use crate::veilid_api::*;
use crate::xx::*;
use core::fmt::Write;
use once_cell::sync::OnceCell;
use tracing_subscriber::*;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use send_wrapper::*;

        struct ApiLoggerInner {
            max_level: Option<VeilidLogLevel>,
            filter_ignore: Cow<'static, [Cow<'static, str>]>,
            update_callback: SendWrapper<UpdateCallback>,
        }
    } else {
        struct ApiLoggerInner {
            max_level: Option<VeilidLogLevel>,
            filter_ignore: Cow<'static, [Cow<'static, str>]>,
            update_callback: UpdateCallback,
        }
    }
}

#[derive(Clone)]
pub struct ApiTracingLayer {
    inner: Arc<Mutex<Option<ApiLoggerInner>>>,
}

static API_LOGGER: OnceCell<ApiTracingLayer> = OnceCell::new();

impl ApiTracingLayer {
    fn new_inner(
        max_level: Option<VeilidLogLevel>,
        update_callback: UpdateCallback,
    ) -> ApiLoggerInner {
        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                ApiLoggerInner {
                    max_level,
                    filter_ignore: Default::default(),
                    update_callback: SendWrapper::new(update_callback),
                }
            } else {
                ApiLoggerInner {
                    max_level,
                    filter_ignore: Default::default(),
                    update_callback,
                }
            }
        }
    }

    #[instrument(level = "debug", skip(update_callback))]
    pub async fn init(max_level: Option<VeilidLogLevel>, update_callback: UpdateCallback) {
        let api_logger = API_LOGGER.get_or_init(|| ApiTracingLayer {
            inner: Arc::new(Mutex::new(None)),
        });
        let apilogger_inner = Some(Self::new_inner(max_level, update_callback));
        *api_logger.inner.lock() = apilogger_inner;
    }

    #[instrument(level = "debug")]
    pub async fn terminate() {
        if let Some(api_logger) = API_LOGGER.get() {
            let mut inner = api_logger.inner.lock();
            *inner = None;
        }
    }

    pub fn get() -> ApiTracingLayer {
        API_LOGGER
            .get_or_init(|| ApiTracingLayer {
                inner: Arc::new(Mutex::new(None)),
            })
            .clone()
    }

    #[instrument(level = "trace")]
    pub fn change_api_log_level(max_level: Option<VeilidLogLevel>) {
        if let Some(api_logger) = API_LOGGER.get() {
            if let Some(inner) = &mut *api_logger.inner.lock() {
                inner.max_level = max_level;
            }
        }
    }

    pub fn add_filter_ignore_str(filter_ignore: &'static str) {
        if let Some(api_logger) = API_LOGGER.get() {
            if let Some(inner) = &mut *api_logger.inner.lock() {
                let mut list = Vec::from(&*inner.filter_ignore);
                list.push(Cow::Borrowed(filter_ignore));
                inner.filter_ignore = Cow::Owned(list);
            }
        }
    }
}

fn display_current_thread_id() -> String {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            "".to_owned()
        } else {
            format!("({}:{:?})",
                if let Some(n) = async_std::task::current().name() {
                    n.to_string()
                }
                else {
                    async_std::task::current().id().to_string()
                },
                std::thread::current().id()
            )
        }
    }
}

impl<S: Subscriber + for<'a> registry::LookupSpan<'a>> Layer<S> for ApiTracingLayer {
    fn enabled(&self, metadata: &tracing::Metadata<'_>, _: layer::Context<'_, S>) -> bool {
        if let Some(inner) = &mut *self.inner.lock() {
            // Skip things that are out of our level
            if let Some(max_level) = inner.max_level {
                if VeilidLogLevel::from_tracing_level(*metadata.level()) > max_level {
                    return false;
                }
            } else {
                return false;
            }
            // Skip filtered targets
            let skip = match (metadata.target(), &*inner.filter_ignore) {
                (path, ignore) if !ignore.is_empty() => {
                    // Check that the module path does not match any ignore filters
                    ignore.iter().any(|v| path.starts_with(&**v))
                }
                _ => false,
            };
            !skip
        } else {
            false
        }
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: layer::Context<'_, S>,
    ) {
        if let Some(_inner) = &mut *self.inner.lock() {
            let mut new_debug_record = StringRecorder::new();
            attrs.record(&mut new_debug_record);

            if let Some(span_ref) = ctx.span(id) {
                span_ref
                    .extensions_mut()
                    .insert::<StringRecorder>(new_debug_record);
            }
        }
    }

    fn on_record(
        &self,
        id: &tracing::Id,
        values: &tracing::span::Record<'_>,
        ctx: layer::Context<'_, S>,
    ) {
        if let Some(_inner) = &mut *self.inner.lock() {
            if let Some(span_ref) = ctx.span(id) {
                if let Some(debug_record) = span_ref.extensions_mut().get_mut::<StringRecorder>() {
                    values.record(debug_record);
                }
            }
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, _ctx: layer::Context<'_, S>) {
        if let Some(inner) = &mut *self.inner.lock() {
            let mut recorder = StringRecorder::new();
            event.record(&mut recorder);
            let meta = event.metadata();
            let level = meta.level();
            if let Some(max_level) = inner.max_level {
                if VeilidLogLevel::from_tracing_level(*level) <= max_level {
                    let log_level = VeilidLogLevel::from_tracing_level(*level);

                    let origin = meta
                        .file()
                        .and_then(|file| meta.line().map(|ln| format!("{}:{}", file, ln)))
                        .unwrap_or_default();

                    let message = format!("{}{} {}", origin, display_current_thread_id(), recorder);

                    (inner.update_callback)(VeilidUpdate::Log(VeilidStateLog {
                        log_level,
                        message,
                    }))
                }
            }
        }
    }
}

struct StringRecorder {
    display: String,
    is_following_args: bool,
}
impl StringRecorder {
    fn new() -> Self {
        StringRecorder {
            display: String::new(),
            is_following_args: false,
        }
    }
}

impl tracing::field::Visit for StringRecorder {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn core::fmt::Debug) {
        if field.name() == "message" {
            if !self.display.is_empty() {
                self.display = format!("{:?}\n{}", value, self.display)
            } else {
                self.display = format!("{:?}", value)
            }
        } else {
            if self.is_following_args {
                // following args
                writeln!(self.display).unwrap();
            } else {
                // first arg
                write!(self.display, " ").unwrap();
                self.is_following_args = true;
            }
            write!(self.display, "{} = {:?};", field.name(), value).unwrap();
        }
    }
}

impl core::fmt::Display for StringRecorder {
    fn fmt(&self, mut f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.display.is_empty() {
            write!(&mut f, " {}", self.display)
        } else {
            Ok(())
        }
    }
}

impl core::default::Default for StringRecorder {
    fn default() -> Self {
        StringRecorder::new()
    }
}

impl log::Log for ApiTracingLayer {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        if let Some(inner) = &mut *self.inner.lock() {
            if let Some(max_level) = inner.max_level {
                return VeilidLogLevel::from_log_level(metadata.level()) <= max_level;
            }
        }
        false
    }

    fn log(&self, record: &log::Record<'_>) {
        if let Some(inner) = &mut *self.inner.lock() {
            // Skip filtered targets
            let skip = match (record.target(), &*inner.filter_ignore) {
                (path, ignore) if !ignore.is_empty() => {
                    // Check that the module path does not match any ignore filters
                    ignore.iter().any(|v| path.starts_with(&**v))
                }
                _ => false,
            };
            if skip {
                return;
            }

            let metadata = record.metadata();
            let level = metadata.level();
            let log_level = VeilidLogLevel::from_log_level(level);
            if let Some(max_level) = inner.max_level {
                if log_level <= max_level {
                    let file = record.file().unwrap_or("<unknown>");
                    let loc = if level >= log::Level::Debug {
                        if let Some(line) = record.line() {
                            format!("[{}:{}] ", file, line)
                        } else {
                            format!("[{}:<unknown>] ", file)
                        }
                    } else {
                        "".to_owned()
                    };
                    let tgt = if record.target().is_empty() {
                        "".to_owned()
                    } else {
                        format!("{}: ", record.target())
                    };

                    let message = format!("{}{}{}", tgt, loc, record.args());

                    (inner.update_callback)(VeilidUpdate::Log(VeilidStateLog {
                        log_level,
                        message,
                    }))
                }
            }
        }
    }

    fn flush(&self) {
        // always flushes
    }
}