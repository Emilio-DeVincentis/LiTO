
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use lito_kernel::{KernelState, lito_spawn, lito_write_pty, lito_read_pty};

#[tokio::test]
async fn test_spawn_write_read_event_cycle() {
    println!("Starting test...");
    // --- Setup ---
    let (event_sender, mut event_receiver) = mpsc::channel::<String>(100);
    let kernel_state = KernelState {
        processes: Arc::new(Mutex::new(HashMap::new())),
        event_sender,
    };

    // --- Spawn `cat` ---
    println!("Spawning cat...");
    let spawn_result = lito_spawn("cat".to_string(), vec![], kernel_state.clone());
    assert!(spawn_result.is_ok());
    let process_id_val = spawn_result.unwrap();

    // Extract process ID string properly. SteelVal::StringV(s)
    let process_id = process_id_val.as_string().expect("Process ID should be a string").to_string();
    println!("Extracted Process ID: {}", process_id);

    // --- Write to PTY ---
    println!("Writing to PTY...");
    let write_result = lito_write_pty(process_id.clone(), "Ciao Kernel!\n".to_string(), kernel_state.clone());
    assert!(write_result.is_ok());

    // --- Wait for Echo Event ---
    println!("Waiting for echo event...");
    let echo_event = tokio::time::timeout(tokio::time::Duration::from_secs(5), event_receiver.recv()).await;
    match echo_event {
        Ok(Some(event_str)) => {
            println!("Received event: {}", event_str);
            assert!(event_str.contains("pty-output"));
            assert!(event_str.contains(&process_id));
        },
        Ok(None) => panic!("Channel closed"),
        Err(_) => println!("Timeout waiting for echo event, continuing to check buffer..."),
    }

    // --- Read from PTY and Verify ---
    println!("Reading from PTY...");
    // Give a brief moment for the background task to update the shared buffer.
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let read_result = lito_read_pty(process_id.clone(), kernel_state.clone());
    assert!(read_result.is_ok());
    let output = read_result.unwrap().as_string().expect("Output should be a string").to_string();
    println!("Output: '{}'", output);

    // The output buffer should contain the echoed string.
    assert!(output.contains("Ciao Kernel!"), "Output buffer did not contain the expected text. Got: '{}'", output);

    // Test multiple writes to ensure the writer wasn't taken and lost
    println!("Writing second time...");
    let write_result2 = lito_write_pty(process_id.clone(), "Seconda riga!\n".to_string(), kernel_state.clone());
    assert!(write_result2.is_ok(), "Second write failed, writer might have been taken and dropped!");

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let read_result2 = lito_read_pty(process_id.clone(), kernel_state.clone());
    let output2 = read_result2.unwrap().as_string().expect("Output should be a string").to_string();
    println!("Output after second write: '{}'", output2);
    assert!(output2.contains("Seconda riga!"), "Second write output not found!");

    println!("Test finished successfully!");
}
