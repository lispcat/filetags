use std::sync::{Arc, Mutex};

use tracing::{debug, Subscriber};
use tracing_subscriber::{
    layer::{Context, Layered},
    prelude::*,
    registry::LookupSpan,
    Layer,
};

///////////////////////////////////////////////////////////////////////////////
//                                  Logging                                  //
///////////////////////////////////////////////////////////////////////////////

/// Holds a single log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: tracing::Level,
    pub target: String,
    pub message: String,
}

/// Type-alias for storage for logs
pub type LogStorage = Arc<Mutex<Vec<LogEntry>>>;

// Memory Layer ///////////////////////////////////////////////////////////////

/// Custom tracing layer that captures logs to memory
pub struct MemoryLayer {
    storage: LogStorage,
}

impl MemoryLayer {
    pub fn new(storage: LogStorage) -> Self {
        Self { storage }
    }
}

/// impl adding new entry to MemoryLayer.LogStorage upon receiving a tracing event
impl<S> Layer<S> for MemoryLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);

        let entry = LogEntry {
            level: *event.metadata().level(),
            target: event.metadata().target().to_string(),
            message: visitor.message,
        };

        // lock mutex and push new entry to LogStorage
        if let Ok(mut logs) = self.storage.lock() {
            logs.push(entry);
        }
    }
}

// Field Visitor //////////////////////////////////////////////////////////////

/// Visitor to extract fields from tracing event
struct FieldVisitor {
    message: String,
}

impl FieldVisitor {
    pub fn new() -> Self {
        Self {
            message: String::new(),
        }
    }
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        debug!("TODO: Not yet implemented!");
    }
}

// Logger /////////////////////////////////////////////////////////////////////

pub struct Logger {
    _guard: tracing::subscriber::DefaultGuard,
    pub storage: LogStorage,
}

impl Logger {
    pub fn new() -> Self {
        let storage = Arc::new(Mutex::new(Vec::new()));
        let memory_layer = MemoryLayer::new(storage.clone());
        let subscriber = Logger::create_subscriber(memory_layer);
        let guard = tracing::subscriber::set_default(subscriber);

        Self {
            _guard: guard,
            storage,
        }
    }

    fn create_subscriber(memory_layer: MemoryLayer) -> impl Subscriber + Send + Sync {
        tracing_subscriber::registry()
            .with(memory_layer)
            .with(tracing_subscriber::fmt::layer())
    }
}
