
use steel_core::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use lito_kernel::{KernelState, lito_spawn, lito_write_pty, lito_read_pty};

#[tokio::test]
async fn test_spawn_write_read_event_cycle() {
    // --- Setup ---
    let (event_sender, mut event_receiver) = mpsc::channel(100);
    let kernel_state = KernelState {
        processes: Arc::new(Mutex::new(HashMap::new())),
        event_sender,
    };

    // --- Spawn `cat` ---
    let spawn_result = lito_spawn("cat".to_string(), vec![], kernel_state.clone());
    assert!(spawn_result.is_ok());
    let process_id = spawn_result.unwrap().to_string();

    // --- Wait for Initial Output Event ---
    // The `cat` process is running and its PTY setup might produce an initial, often empty, output.
    let initial_event = event_receiver.recv().await;
    assert!(initial_event.is_some(), "Did not receive the initial event after spawn.");
    let event = initial_event.unwrap();
    if let SteelVal::Vector(v) = event {
        assert_eq!(v[0], SteelVal::Symbol(Box::new("pty-output".to_string())));
        assert_eq!(v[1].to_string(), process_id);
    } else {
        panic!("Received event was not a vector: {:?}", event);
    }

    // --- Write to PTY ---
    let write_result = lito_write_pty(process_id.clone(), "Ciao Kernel!\n".to_string(), kernel_state.clone());
    assert!(write_result.is_ok());

    // --- Wait for Echo Event ---
    // We expect the `cat` process to echo our input back to us.
    let echo_event = event_receiver.recv().await;
    assert!(echo_event.is_some(), "Did not receive the echo event after write.");
    let event = echo_event.unwrap();
    if let SteelVal::Vector(v) = event {
        assert_eq!(v[0], SteelVal::Symbol(Box::new("pty-output".to_string())));
        assert_eq!(v[1].to_string(), process_id);
    } else {
        panic!("Received echo event was not a vector: {:?}", event);
    }

    // --- Read from PTY and Verify ---
    // Give a brief moment for the background task to update the shared buffer.
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let read_result = lito_read_pty(process_id.clone(), kernel_state.clone());
    assert!(read_result.is_ok());
    let output = read_result.unwrap().to_string();

    // The output buffer should contain the echoed string.
    assert!(output.contains("Ciao Kernel!"), "Output buffer did not contain the expected text. Got: '{}'", output);
}
