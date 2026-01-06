use clap::Parser;
use log::{error, info};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;

mod errors;
mod instrumentors;
mod opentelemetry_controller;
mod process;

use errors::Result;
use instrumentors::Manager;
use opentelemetry_controller::Controller;
use process::{Analyzer, TargetArgs};

#[derive(Parser, Debug)]
#[command(name = "otel-rust-agent")]
#[command(author = "OpenTelemetry Authors")]
#[command(version = "0.1.0")]
#[command(about = "OpenTelemetry Auto-Instrumentation for Rust using eBPF")]
struct Args {
    #[arg(long, env = "OTEL_TARGET_EXE")]
    target_exe: Option<String>,

    #[arg(long, env = "OTEL_TARGET_PID")]
    target_pid: Option<i32>,

    #[arg(long, env = "OTEL_SERVICE_NAME")]
    service_name: String,

    #[arg(long, env = "OTEL_EXPORTER_OTLP_ENDPOINT", default_value = "http://localhost:4317")]
    otlp_endpoint: String,

    #[arg(long, env = "OTEL_STDOUT", default_value = "false")]
    stdout: bool,

    #[arg(long, short, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    env_logger::Builder::new()
        .filter_level(args.log_level.parse().unwrap_or(log::LevelFilter::Info))
        .init();

    info!("Starting Rust OpenTelemetry Agent...");

    let target = TargetArgs {
        exe_path: args.target_exe,
        pid: args.target_pid,
    };

    if let Err(e) = target.validate() {
        error!("Invalid target args: {}", e);
        return Err(e);
    }

    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let shutdown_rx = shutdown_tx.subscribe();

    let controller = if args.stdout {
        Controller::new_stdout(&args.service_name)?
    } else {
        Controller::new(&args.otlp_endpoint, &args.service_name)?
    };

    let controller = Arc::new(controller);
    let analyzer = Analyzer::new();
    let manager = Manager::new(Arc::clone(&controller));

    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Received shutdown signal, cleaning up...");
        let _ = shutdown_tx_clone.send(());
    });

    let pid = match analyzer.discover_process(&target).await {
        Ok(pid) => pid,
        Err(e) => {
            error!("Failed to discover process: {}", e);
            return Err(e);
        }
    };

    info!("Found target process with PID: {}", pid);

    let target_details = match analyzer.analyze(pid, manager.get_relevant_funcs()).await {
        Ok(details) => details,
        Err(e) => {
            error!("Failed to analyze target process: {}", e);
            return Err(e);
        }
    };

    info!(
        "Target process analysis completed - PID: {}, Functions found: {}",
        target_details.pid,
        target_details.functions.len()
    );

    manager.filter_unused_instrumentors(&target_details);

    info!("Invoking instrumentors...");
    if let Err(e) = manager.run(&target_details, shutdown_rx).await {
        if !matches!(e, errors::Error::Interrupted) {
            error!("Error running instrumentors: {}", e);
            return Err(e);
        }
    }

    info!("Agent shutdown complete");
    Ok(())
}

mod errors {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum Error {
        #[error("Invalid target arguments: {0}")]
        InvalidTarget(String),

        #[error("Process not found: {0}")]
        ProcessNotFound(String),

        #[error("Failed to analyze binary: {0}")]
        BinaryAnalysis(String),

        #[error("eBPF error: {0}")]
        Ebpf(String),

        #[error("OpenTelemetry error: {0}")]
        OpenTelemetry(String),

        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),

        #[error("Agent interrupted")]
        Interrupted,
    }

    pub type Result<T> = std::result::Result<T, Error>;
}

mod opentelemetry_controller {
    use super::errors::{Error, Result};
    use opentelemetry::trace::TracerProvider;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::Tracer;
    use std::time::Duration;

    pub struct Controller {
        tracer: Tracer,
        service_name: String,
    }

    impl Controller {
        pub fn new(endpoint: &str, service_name: &str) -> Result<Self> {
            let exporter = opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint)
                .with_timeout(Duration::from_secs(10));

            let provider = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(exporter)
                .with_trace_config(
                    opentelemetry_sdk::trace::Config::default()
                        .with_resource(opentelemetry_sdk::Resource::new(vec![
                            opentelemetry::KeyValue::new("service.name", service_name.to_string()),
                        ])),
                )
                .install_batch(opentelemetry_sdk::runtime::Tokio)
                .map_err(|e| Error::OpenTelemetry(e.to_string()))?;

            let tracer = provider.tracer("rust-auto-instrumentation");

            Ok(Self {
                tracer,
                service_name: service_name.to_string(),
            })
        }

        pub fn new_stdout(service_name: &str) -> Result<Self> {
            let provider = opentelemetry_sdk::trace::TracerProvider::builder()
                .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
                .with_resource(opentelemetry_sdk::Resource::new(vec![
                    opentelemetry::KeyValue::new("service.name", service_name.to_string()),
                ]))
                .build();

            let tracer = provider.tracer("rust-auto-instrumentation");

            Ok(Self {
                tracer,
                service_name: service_name.to_string(),
            })
        }

        pub fn tracer(&self) -> &Tracer {
            &self.tracer
        }

        pub fn service_name(&self) -> &str {
            &self.service_name
        }
    }
}

mod process {
    use super::errors::{Error, Result};
    use goblin::elf::Elf;
    use log::{debug, info};
    use memmap2::Mmap;
    use procfs::process::Process;
    use rustc_demangle::demangle;
    use std::collections::HashMap;
    use std::fs::File;
    use std::path::PathBuf;
    use std::time::Duration;

    pub struct TargetArgs {
        pub exe_path: Option<String>,
        pub pid: Option<i32>,
    }

    impl TargetArgs {
        pub fn validate(&self) -> Result<()> {
            if self.exe_path.is_none() && self.pid.is_none() {
                return Err(Error::InvalidTarget(
                    "Either OTEL_TARGET_EXE or OTEL_TARGET_PID must be set".to_string(),
                ));
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    pub struct FunctionInfo {
        pub name: String,
        pub demangled_name: String,
        pub address: u64,
        pub size: u64,
    }

    #[derive(Debug)]
    pub struct TargetDetails {
        pub pid: i32,
        pub exe_path: PathBuf,
        pub functions: Vec<FunctionInfo>,
        pub libraries: Vec<String>,
    }

    pub struct Analyzer;

    impl Analyzer {
        pub fn new() -> Self {
            Self
        }

        pub async fn discover_process(&self, target: &TargetArgs) -> Result<i32> {
            if let Some(pid) = target.pid {
                let proc = Process::new(pid)
                    .map_err(|_| Error::ProcessNotFound(format!("PID {} not found", pid)))?;
                info!("Using provided PID: {}", pid);
                return Ok(proc.pid());
            }

            if let Some(ref exe_path) = target.exe_path {
                info!("Searching for process with executable: {}", exe_path);
                loop {
                    for proc in procfs::process::all_processes()
                        .map_err(|e| Error::ProcessNotFound(e.to_string()))?
                    {
                        if let Ok(proc) = proc {
                            if let Ok(exe) = proc.exe() {
                                if exe.to_string_lossy().contains(exe_path) {
                                    info!("Found process {} at PID {}", exe_path, proc.pid());
                                    return Ok(proc.pid());
                                }
                            }
                        }
                    }
                    debug!("Process not found yet, retrying in 1 second...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }

            Err(Error::InvalidTarget("No valid target specified".to_string()))
        }

        pub async fn analyze(
            &self,
            pid: i32,
            relevant_funcs: HashMap<String, ()>,
        ) -> Result<TargetDetails> {
            let proc = Process::new(pid)
                .map_err(|e| Error::ProcessNotFound(format!("PID {} not found: {}", pid, e)))?;

            let exe_path = proc
                .exe()
                .map_err(|e| Error::BinaryAnalysis(format!("Failed to get exe path: {}", e)))?;

            info!("Analyzing binary: {:?}", exe_path);

            let file = File::open(&exe_path)
                .map_err(|e| Error::BinaryAnalysis(format!("Failed to open binary: {}", e)))?;

            let mmap = unsafe {
                Mmap::map(&file)
                    .map_err(|e| Error::BinaryAnalysis(format!("Failed to mmap binary: {}", e)))?
            };

            let elf = Elf::parse(&mmap)
                .map_err(|e| Error::BinaryAnalysis(format!("Failed to parse ELF: {}", e)))?;

            let mut functions = Vec::new();
            let mut libraries = Vec::new();

            for lib in &elf.libraries {
                libraries.push(lib.to_string());
            }

            for sym in elf.syms.iter() {
                if sym.st_type() == goblin::elf::sym::STT_FUNC && sym.st_size > 0 {
                    if let Some(name) = elf.strtab.get_at(sym.st_name) {
                        let demangled = demangle(name).to_string();

                        let matches = relevant_funcs.is_empty()
                            || relevant_funcs.contains_key(name)
                            || relevant_funcs.contains_key(&demangled);

                        if matches {
                            functions.push(FunctionInfo {
                                name: name.to_string(),
                                demangled_name: demangled,
                                address: sym.st_value,
                                size: sym.st_size,
                            });
                        }
                    }
                }
            }

            for sym in elf.dynsyms.iter() {
                if sym.st_type() == goblin::elf::sym::STT_FUNC && sym.st_size > 0 {
                    if let Some(name) = elf.dynstrtab.get_at(sym.st_name) {
                        let demangled = demangle(name).to_string();

                        let matches = relevant_funcs.is_empty()
                            || relevant_funcs.contains_key(name)
                            || relevant_funcs.contains_key(&demangled);

                        if matches {
                            functions.push(FunctionInfo {
                                name: name.to_string(),
                                demangled_name: demangled,
                                address: sym.st_value,
                                size: sym.st_size,
                            });
                        }
                    }
                }
            }

            info!("Found {} relevant functions", functions.len());

            Ok(TargetDetails {
                pid,
                exe_path,
                functions,
                libraries,
            })
        }
    }
}

mod instrumentors {
    use super::errors::{Error, Result};
    use super::opentelemetry_controller::Controller;
    use super::process::TargetDetails;
    use async_trait::async_trait;
    use log::{info, warn};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    #[derive(Debug, Clone)]
    pub struct Event {
        pub library: String,
        pub name: String,
        pub start_time: u64,
        pub end_time: u64,
        pub trace_id: [u8; 16],
        pub span_id: [u8; 8],
        pub attributes: Vec<(String, String)>,
    }

    #[async_trait]
    pub trait Instrumentor: Send + Sync {
        fn library_name(&self) -> &str;
        fn func_names(&self) -> Vec<&str>;
        async fn load(&mut self, target: &TargetDetails) -> Result<()>;
        async fn run(&self, events_tx: tokio::sync::mpsc::Sender<Event>) -> Result<()>;
        fn close(&mut self);
    }

    pub struct Manager {
        instrumentors: HashMap<String, Box<dyn Instrumentor>>,
        controller: Arc<Controller>,
    }

    impl Manager {
        pub fn new(controller: Arc<Controller>) -> Self {
            let mut instrumentors: HashMap<String, Box<dyn Instrumentor>> = HashMap::new();

            instrumentors.insert(
                "hyper".to_string(),
                Box::new(super::hyper_instrumentor::HyperInstrumentor::new()),
            );

            Self {
                instrumentors,
                controller,
            }
        }

        pub fn get_relevant_funcs(&self) -> HashMap<String, ()> {
            let mut funcs = HashMap::new();
            for inst in self.instrumentors.values() {
                for func in inst.func_names() {
                    funcs.insert(func.to_string(), ());
                }
            }
            funcs
        }

        pub fn filter_unused_instrumentors(&self, target: &TargetDetails) {
            let existing_funcs: HashMap<String, ()> = target
                .functions
                .iter()
                .map(|f| (f.demangled_name.clone(), ()))
                .collect();

            for (name, inst) in &self.instrumentors {
                let found = inst
                    .func_names()
                    .iter()
                    .filter(|f| existing_funcs.contains_key(*f))
                    .count();

                if found == 0 {
                    warn!("Instrumentor {} has no matching functions", name);
                } else {
                    info!(
                        "Instrumentor {} found {}/{} functions",
                        name,
                        found,
                        inst.func_names().len()
                    );
                }
            }
        }

        pub async fn run(
            &self,
            target: &TargetDetails,
            mut shutdown_rx: broadcast::Receiver<()>,
        ) -> Result<()> {
            let (events_tx, mut events_rx) = tokio::sync::mpsc::channel::<Event>(1024);

            info!("Starting instrumentors for {} libraries", self.instrumentors.len());

            let controller = Arc::clone(&self.controller);
            let events_handler = tokio::spawn(async move {
                use opentelemetry::trace::{Span, SpanKind, Tracer};
                let tracer = controller.tracer();

                while let Some(event) = events_rx.recv().await {
                    let mut span = tracer
                        .span_builder(event.name.clone())
                        .with_kind(SpanKind::Server)
                        .start(tracer);

                    for (key, value) in event.attributes {
                        span.set_attribute(opentelemetry::KeyValue::new(key, value));
                    }

                    span.end();
                }
            });

            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    return Err(Error::Interrupted);
                }
                _ = events_handler => {
                    info!("Events handler completed");
                }
            }

            Ok(())
        }
    }
}

mod hyper_instrumentor {
    use super::errors::Result;
    use super::instrumentors::{Event, Instrumentor};
    use super::process::TargetDetails;
    use async_trait::async_trait;

    pub struct HyperInstrumentor {
        loaded: bool,
    }

    impl HyperInstrumentor {
        pub fn new() -> Self {
            Self { loaded: false }
        }
    }

    #[async_trait]
    impl Instrumentor for HyperInstrumentor {
        fn library_name(&self) -> &str {
            "hyper"
        }

        fn func_names(&self) -> Vec<&str> {
            vec![
                "hyper::proto::h1::dispatch::Dispatcher<D,Bs,I,T>::poll_read",
                "hyper::server::conn::Http::serve_connection",
                "<hyper::server::server::Server<I,S,E>>::serve",
            ]
        }

        async fn load(&mut self, _target: &TargetDetails) -> Result<()> {
            self.loaded = true;
            Ok(())
        }

        async fn run(&self, _events_tx: tokio::sync::mpsc::Sender<Event>) -> Result<()> {
            Ok(())
        }

        fn close(&mut self) {
            self.loaded = false;
        }
    }
}

