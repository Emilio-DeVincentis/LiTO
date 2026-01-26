
use futures::StreamExt;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use steel_core::steel_vm::engine::Engine;
use steel_core::rvals::Result;
use steel_core::rvals::{SteelVal, SteelErr};
use steel_core::rerrs::ErrorKind;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

// --- Public Core Data Structures ---

pub struct PtyHandle {
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    output_buffer: Arc<Mutex<String>>,
}

#[derive(Clone)]
pub struct KernelState {
    pub processes: Arc<Mutex<HashMap<String, PtyHandle>>>,
    pub event_sender: mpsc::Sender<SteelVal>,
}

// --- Public Steel Primitives ---

pub fn lito_spawn(
    command: String,
    args: Vec<String>,
    state: KernelState,
) -> Result<SteelVal> {
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| SteelErr::new(ErrorKind::Contract, e.to_string()))?;

    let mut cmd = CommandBuilder::new(command);
    cmd.args(&args);

    let _child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| SteelErr::new(ErrorKind::Contract, e.to_string()))?;

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| SteelErr::new(ErrorKind::Contract, e.to_string()))?;
    let writer = pair.master;

    let process_id = Uuid::new_v4().to_string();
    let output_buffer = Arc::new(Mutex::new(String::new()));

    let handle = PtyHandle {
        master: Arc::new(Mutex::new(writer)),
        output_buffer: Arc::clone(&output_buffer),
    };

    state.processes.lock().unwrap().insert(process_id.clone(), handle);

    let event_sender = state.event_sender.clone();
    let process_id_clone = process_id.clone();
    tokio::spawn(async move {
        let mut reader = tokio_util::io::ReaderStream::new(reader);
        while let Some(result) = reader.next().await {
            match result {
                Ok(bytes) => {
                    let output = String::from_utf8_lossy(&bytes).to_string();
                    output_buffer.lock().unwrap().push_str(&output);
                    let event = SteelVal::Vector(
                        vec![
                            SteelVal::Symbol(Box::new("pty-output".to_string())),
                            SteelVal::StringV(process_id_clone.clone().into()),
                        ]
                        .into(),
                    );
                    if event_sender.send(event).await.is_err() {
                        tracing::error!("Failed to send event: Steel-Bus channel closed.");
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("Error reading from PTY for {}: {}", process_id_clone, e);
                    break;
                }
            }
        }
        info!("PTY reader task finished for process {}", process_id_clone);
    });

    Ok(SteelVal::StringV(process_id.into()))
}

pub fn lito_write_pty(id: String, input: String, state: KernelState) -> Result<SteelVal> {
    let processes = state.processes.lock().unwrap();
    if let Some(handle) = processes.get(&id) {
        let mut master = handle.master.lock().unwrap();
        master
            .write_all(input.as_bytes())
            .map_err(|e| SteelErr::new(ErrorKind::Contract, e.to_string()))?;
        Ok(SteelVal::BoolV(true))
    } else {
        Err(SteelErr::new(
            ErrorKind::Contract,
            format!("No process found with ID: {}", id),
        ))
    }
}

pub fn lito_read_pty(id: String, state: KernelState) -> Result<SteelVal> {
    let processes = state.processes.lock().unwrap();
    if let Some(handle) = processes.get(&id) {
        let buffer = handle.output_buffer.lock().unwrap();
        Ok(SteelVal::StringV(buffer.clone().into()))
    } else {
        Err(SteelErr::new(
            ErrorKind::Contract,
            format!("No process found with ID: {}", id),
        ))
    }
}
