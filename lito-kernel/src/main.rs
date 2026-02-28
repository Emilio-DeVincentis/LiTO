
use steel_core::prelude::*;
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

    let (event_sender, mut event_receiver) = mpsc::channel(100);

    let kernel_state = KernelState {
        processes: Arc::new(Mutex::new(HashMap::new())),
        event_sender,
    };

    let mut vm = Engine::new();

    // --- Register Primitives ---
    let state_clone = kernel_state.clone();
    vm.register_fn("lito/spawn", move |args: &[SteelVal]| {
        let (command, rest) = args.split_first().ok_or_else(SteelErr::ArityMismatch)?;
        let command_str = command.string_or_else(SteelErr::TypeMismatch)?;
        let arg_vec: Vec<String> = rest
            .iter()
            .map(|v| v.string_or_else(SteelErr::TypeMismatch).map(|s| s.to_string()))
            .collect::<Result<_, _>>()?;
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

    let receiver_arc = Arc::new(Mutex::new(event_receiver));
    vm.register_fn("lito/get-event", move || {
        let mut receiver = receiver_arc.lock().unwrap();
        match tokio::runtime::Handle::current().block_on(receiver.recv()) {
            Some(event) => Ok(event),
            None => Err(SteelErr::new(ErrorKind::Generic, "Event channel closed".to_string())),
        }
    });

    // Note: The path is now relative to the `lito-kernel` directory where the binary runs.
    let spec_path = Path::new("../SPECIFICA_CORE.scm");
    if spec_path.exists() {
        info!("Loading SPECIFICA_CORE.scm...");
        if let Err(e) = vm.parse_and_eval_file(spec_path) {
            tracing::error!("Failed to load SPECIFICA_CORE.scm: {}", e);
        }
    } else {
        tracing::warn!("SPECIFICA_CORE.scm not found at {:?}. Skipping.", spec_path.canonicalize().unwrap_or_default());
    }

    info!("Kernel initialized. Welcome to the Lito REPL!");
    vm.repl().unwrap();
}
