#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_pty_e2e_communication() {
    use greenup_simulator::engine::EngineConfig;
    use greenup_simulator::pty::PtyPair;
    use greenup_simulator::worker::{AtmegaWorker, UserEvent};
    use std::io::{BufRead, BufReader, Write};
    use std::path::PathBuf;
    use std::time::Duration;

    let test_symlink = PathBuf::from("/tmp/ttyUSB_sim_e2e_test");
    if test_symlink.exists() {
        let _ = std::fs::remove_file(&test_symlink);
    }

    let pty = PtyPair::new(Some(&test_symlink)).expect("Failed to create PTY pair");
    let config = EngineConfig::for_reference("058013", 1);

    let (worker, handle) = AtmegaWorker::spawn(pty, config).expect("Failed to spawn worker");

    // Open connection using standard serialport library, exactly like production greenup-driver
    let mut port = serialport::new(test_symlink.to_str().unwrap(), 115_200)
        .timeout(Duration::from_millis(1500))
        .open()
        .expect("Failed to open serial port on PTY symlink");

    let mut reader = BufReader::new(port.try_clone().expect("try_clone port failed"));
    let mut line = String::new();

    // Give the worker tasks a fraction of a second to spin up
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send RaspberryPiModeOK handshake
    port.write_all(b"RaspberryPiModeOK\r").expect("Write failed");
    port.flush().expect("Flush failed");

    // Read until we get DIP1:1 (Legrand power board response to RaspberryPiModeOK)
    let mut got_handshake = false;
    for _ in 0..5 {
        line.clear();
        if reader.read_line(&mut line).is_ok() {
            let trimmed = line.trim();
            if trimmed == "DIP1:1" {
                got_handshake = true;
                break;
            }
        }
    }
    assert!(got_handshake, "Expected DIP1:1 handshake response");

    // Query Reference?
    port.write_all(b"Reference?\r").expect("Write failed");
    port.flush().expect("Flush failed");

    let mut got_ref = false;
    for _ in 0..5 {
        line.clear();
        if reader.read_line(&mut line).is_ok() {
            let trimmed = line.trim();
            if trimmed == "Reference:058013" {
                got_ref = true;
                break;
            }
        }
    }
    assert!(got_ref, "Expected Reference:058013 response");

    // Query Side?
    port.write_all(b"Side?\r").expect("Write failed");
    port.flush().expect("Flush failed");

    let mut got_side = false;
    for _ in 0..5 {
        line.clear();
        if reader.read_line(&mut line).is_ok() {
            let trimmed = line.trim();
            if trimmed == "Side:49" {
                got_side = true;
                break;
            }
        }
    }
    assert!(got_side, "Expected Side:49 response for side 1");

    // Ping heartbeat probe test
    port.write_all(b"ping\r").expect("Write failed");
    port.flush().expect("Flush failed");

    let mut got_pong = false;
    for _ in 0..5 {
        line.clear();
        if reader.read_line(&mut line).is_ok() {
            let trimmed = line.trim();
            if trimmed == "pong" {
                got_pong = true;
                break;
            }
        }
    }
    assert!(got_pong, "Expected pong response to ping heartbeat");

    // Trigger vehicle plug-in
    worker.trigger(UserEvent::Plug).await.expect("Trigger plug failed");

    let mut got_state_b = false;
    for _ in 0..10 {
        line.clear();
        if reader.read_line(&mut line).is_ok() {
            let trimmed = line.trim();
            if trimmed == "State:B" {
                got_state_b = true;
                break;
            }
        }
    }
    assert!(got_state_b, "Expected spontaneous State:B frame on plug");

    // Trigger charge start
    worker.trigger(UserEvent::Charge).await.expect("Trigger charge failed");

    let mut got_state_c = false;
    for _ in 0..10 {
        line.clear();
        if reader.read_line(&mut line).is_ok() {
            let trimmed = line.trim();
            if trimmed == "State:C" {
                got_state_c = true;
                break;
            }
        }
    }
    assert!(got_state_c, "Expected spontaneous State:C frame on charge authorize");

    // Wait a brief moment for the 1Hz telemetry tick (CCI:xx.xx)
    tokio::time::sleep(Duration::from_millis(1100)).await;

    let mut got_cci = false;
    for _ in 0..10 {
        line.clear();
        if reader.read_line(&mut line).is_ok() {
            let trimmed = line.trim();
            if trimmed.starts_with("CCI:") {
                got_cci = true;
                break;
            }
        }
    }
    assert!(got_cci, "Expected periodic spontaneous CCI frame during charge");

    // Clean up
    handle.abort();
    drop(reader);
    drop(port);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_pty_dual_point_e2e_communication() {
    use greenup_simulator::engine::EngineConfig;
    use greenup_simulator::pty::PtyPair;
    use greenup_simulator::worker::{AtmegaWorker, UserEvent};
    use std::io::{BufRead, BufReader, Write};
    use std::path::PathBuf;
    use std::time::Duration;

    let symlink1 = PathBuf::from("/tmp/ttyUSB_sim_dual_0");
    let symlink2 = PathBuf::from("/tmp/ttyUSB_sim_dual_1");
    let _ = std::fs::remove_file(&symlink1);
    let _ = std::fs::remove_file(&symlink2);

    let pty1 = PtyPair::new(Some(&symlink1)).expect("Failed to create PTY pair 1");
    let pty2 = PtyPair::new(Some(&symlink2)).expect("Failed to create PTY pair 2");

    let cfg1 = EngineConfig::for_reference("058013", 1);
    let cfg2 = EngineConfig::for_reference("058013", 2);

    let (worker1, handle1) = AtmegaWorker::spawn(pty1, cfg1).expect("Spawn worker 1 failed");
    let (_worker2, handle2) = AtmegaWorker::spawn(pty2, cfg2).expect("Spawn worker 2 failed");

    let mut port1 = serialport::new(symlink1.to_str().unwrap(), 115_200)
        .timeout(Duration::from_millis(1500))
        .open()
        .expect("Open port 1 failed");
    let mut port2 = serialport::new(symlink2.to_str().unwrap(), 115_200)
        .timeout(Duration::from_millis(1500))
        .open()
        .expect("Open port 2 failed");

    let mut reader1 = BufReader::new(port1.try_clone().unwrap());
    let mut reader2 = BufReader::new(port2.try_clone().unwrap());

    // Query Side? on both sides simultaneously
    port1.write_all(b"Side?\r").unwrap();
    port1.flush().unwrap();
    port2.write_all(b"Side?\r").unwrap();
    port2.flush().unwrap();

    let mut line = String::new();
    reader1.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "Side:49");

    line.clear();
    reader2.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "Side:50");

    // Plug & Charge on Point 1 only
    worker1.trigger(UserEvent::Plug).await.unwrap();
    worker1.trigger(UserEvent::Charge).await.unwrap();

    // Query State? on both
    port1.write_all(b"State?\r").unwrap();
    port1.flush().unwrap();
    port2.write_all(b"State?\r").unwrap();
    port2.flush().unwrap();

    // Point 1 should be in State C (or had State:C event)
    let mut got_p1_state = false;
    for _ in 0..10 {
        line.clear();
        if reader1.read_line(&mut line).is_ok() {
            let t = line.trim();
            if t == "State:C" {
                got_p1_state = true;
                break;
            }
        }
    }
    assert!(got_p1_state, "Expected Point 1 in State C");

    // Point 2 must remain in State A!
    line.clear();
    reader2.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "State:A", "Point 2 should remain in State A");

    handle1.abort();
    handle2.abort();
}
