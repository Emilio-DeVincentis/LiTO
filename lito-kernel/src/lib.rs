
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use steel::rvals::{Result, SteelVal};
use steel::rerrs::{ErrorKind, SteelErr};
use std::collections::HashMap;
use std::io::{Write, Read};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

// --- Public Core Data Structures ---

pub struct PtyHandle {
    writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
    output_buffer: Arc<Mutex<String>>,
}

#[derive(Clone)]
pub struct KernelState {
    pub processes: Arc<Mutex<HashMap<String, PtyHandle>>>,
    pub event_sender: mpsc::Sender<String>,
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
        .map_err(|e| SteelErr::new(ErrorKind::Generic, e.to_string()))?;

    let mut cmd = CommandBuilder::new(command);
    cmd.args(&args);

    let _child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| SteelErr::new(ErrorKind::Generic, e.to_string()))?;

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| SteelErr::new(ErrorKind::Generic, e.to_string()))?;

    // Take the writer once here.
    let writer = pair.master.take_writer()
        .map_err(|e| SteelErr::new(ErrorKind::Generic, e.to_string()))?;

    let process_id = Uuid::new_v4().to_string();
    let output_buffer = Arc::new(Mutex::new(String::new()));

    let handle = PtyHandle {
        writer: Arc::new(Mutex::new(writer)),
        output_buffer: Arc::clone(&output_buffer),
    };

    state.processes.lock().unwrap().insert(process_id.clone(), handle);

    let event_sender = state.event_sender.clone();
    let process_id_clone = process_id.clone();
    tokio::spawn(async move {
        let mut reader = reader;
        loop {
            let mut buf = [0u8; 1024];
            let (res, buf, reader_res) = match tokio::task::spawn_blocking({
                move || {
                    let res = reader.read(&mut buf);
                    (res, buf, reader)
                }
            }).await {
                Ok(tuple) => tuple,
                Err(e) => {
                    tracing::error!("Blocking task panicked for {}: {}", process_id_clone, e);
                    break;
                }
            };

            reader = reader_res;
            match res {
                Ok(0) => break,
                Ok(n) => {
                    let output = String::from_utf8_lossy(&buf[..n]).to_string();
                    output_buffer.lock().unwrap().push_str(&output);

                    let event = format!("(pty-output \"{}\")", process_id_clone);
                    if event_sender.send(event).await.is_err() {
                        tracing::error!("Failed to send event: Steel-Bus channel closed.");
                        break;
                    }
                },
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
        let mut writer = handle.writer.lock().unwrap();
        writer
            .write_all(input.as_bytes())
            .map_err(|e| SteelErr::new(ErrorKind::Generic, e.to_string()))?;
        Ok(SteelVal::BoolV(true))
    } else {
        Err(SteelErr::new(
            ErrorKind::Generic,
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
            ErrorKind::Generic,
            format!("No process found with ID: {}", id),
        ))
    }
}
