
use steel::steel_vm::engine::Engine;
use steel::rvals::{SteelVal, Result};
use steel::rerrs::{SteelErr, ErrorKind};
use steel::steel_vm::register_fn::RegisterFn;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::info;
use lito_kernel::{KernelState, lito_spawn, lito_write_pty, lito_read_pty};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let (event_sender, event_receiver) = mpsc::channel::<String>(100);
    let event_receiver = Arc::new(Mutex::new(event_receiver));

    let kernel_state = KernelState {
        processes: Arc::new(Mutex::new(HashMap::new())),
        event_sender,
    };

    let mut vm = Engine::new();

    // --- Register Primitives ---
    let state_clone = kernel_state.clone();
    vm.register_fn("lito/spawn", move |arg1: SteelVal, arg2: SteelVal| -> Result<SteelVal> {
        let command_str = arg1.as_string().ok_or_else(|| SteelErr::new(ErrorKind::TypeMismatch, "lito/spawn expected a string command".to_string()))?;
        let arg_vec = match &arg2 {
            SteelVal::VectorV(v) => {
                v.iter()
                 .map(|v| v.as_string().map(|s| s.to_string()).ok_or_else(|| SteelErr::new(ErrorKind::TypeMismatch, "lito/spawn expected string arguments".to_string())))
                 .collect::<Result<Vec<String>>>()?
            },
            _ => {
                if let Some(s) = arg2.as_string() {
                    vec![s.to_string()]
                } else {
                    vec![]
                }
            }
        };
        lito_spawn(command_str.to_string(), arg_vec, state_clone.clone())
    });

    let state_clone = kernel_state.clone();
    vm.register_fn("lito/write-pty", move |id: String, input: String| {
        lito_write_pty(id, input, state_clone.clone())
    });

    let state_clone = kernel_state.clone();
    vm.register_fn("lito/read-pty", move |id: String| {
        lito_read_pty(id, state_clone.clone())
    });

    let receiver_clone = Arc::clone(&event_receiver);
    vm.register_fn("lito/get-event-blocking", move || {
        let mut receiver = receiver_clone.lock().unwrap();
        match receiver.try_recv() {
            Ok(event) => Ok(SteelVal::StringV(event.into())),
            Err(mpsc::error::TryRecvError::Empty) => Ok(SteelVal::BoolV(false)),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(SteelErr::new(ErrorKind::Generic, "Event channel closed".to_string())),
        }
    });

    // Note: The path is now relative to the `lito-kernel` directory where the binary runs.
    let spec_path = Path::new("../SPECIFICA_CORE.scm");
    if spec_path.exists() {
        info!("Loading SPECIFICA_CORE.scm...");
        let content = std::fs::read_to_string(spec_path).expect("Failed to read SPECIFICA_CORE.scm");
        if let Err(e) = vm.compile_and_run_raw_program(content) {
            tracing::error!("Failed to load SPECIFICA_CORE.scm: {}", e);
        }
    } else {
        tracing::warn!("SPECIFICA_CORE.scm not found at {:?}. Skipping.", spec_path.canonicalize().unwrap_or_default());
    }

    info!("Kernel initialized. Welcome to the Lito REPL!");
    use std::io::{self, Write};
    let mut input_buf = String::new();
    loop {
        print!("lito> ");
        io::stdout().flush().unwrap();
        input_buf.clear();
        if io::stdin().read_line(&mut input_buf).unwrap() == 0 {
            break;
        }
        let input = input_buf.trim().to_string();
        if input.is_empty() {
            continue;
        }
        if let Err(e) = vm.compile_and_run_raw_program(input) {
            println!("Error: {}", e);
        }
    }
}
