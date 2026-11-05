//! Low-level driver state container (`DriverHandle`) managing a single USB serial port
//! and broadcasting [`DriverSnapshot`] updates over a `tokio::sync::watch` channel.

use std::sync::{
    atomic::{AtomicU64, AtomicU8, Ordering},
    Arc, Mutex,
};
use tokio::sync::{mpsc, watch};

pub use crate::raw_state::{
    AtmegaRawState, DriverSnapshot, SerialStatus, SupervisionMode, UsbLinkState,
};

#[derive(Default, Debug, Clone)]
pub struct TicDetectionState {
    pub is_active: bool,
    pub zero_count: u32,
}

/// Low-level handle to a single ATmega2560 USB serial connection.
///
/// Holds only raw ATmega registers ([`AtmegaRawState`]) and USB link status ([`UsbLinkState`]),
/// and notifies upper layers on every modification via a `tokio::sync::watch` channel.
pub struct DriverHandle {
    /// Physical charge point side (`1` or `2`).
    pub side: AtomicU8,
    /// UART supervision mode (`0` = Remote active, `1` = Local passive).
    pub mode: AtomicU8,
    /// Monotonic sequence counter incremented on every state change.
    pub seq: AtomicU64,
    /// Raw ATmega2560 registers reported over serial.
    pub raw: Mutex<AtmegaRawState>,
    /// Physical USB serial link status and counters.
    pub usb: Mutex<UsbLinkState>,
    /// Internal state for the `TICTM:1` / `TICTM:0` baudrate detection sequence.
    pub tic_detection: Mutex<TicDetectionState>,
    /// Channel used to send TX commands to the serial worker thread.
    pub serial_tx: mpsc::Sender<greenup_protocol::Command>,
    /// Watch sender broadcasting a [`DriverSnapshot`] on every state update.
    watch_tx: watch::Sender<DriverSnapshot>,
    /// Watch receiver template cloned by subscribers.
    watch_rx: watch::Receiver<DriverSnapshot>,
}

impl DriverHandle {
    pub fn new(serial_tx: mpsc::Sender<greenup_protocol::Command>) -> Self {
        Self::new_with_side(1, serial_tx)
    }

    pub fn new_with_side(side: u8, serial_tx: mpsc::Sender<greenup_protocol::Command>) -> Self {
        Self::new_with_side_and_mode(side, SupervisionMode::Remote, serial_tx)
    }

    pub fn new_with_side_and_mode(
        side: u8,
        mode: SupervisionMode,
        serial_tx: mpsc::Sender<greenup_protocol::Command>,
    ) -> Self {
        let mut raw = AtmegaRawState::default();
        raw.side = Some(side);

        let mut usb = UsbLinkState::default();
        usb.mode = mode;

        let initial_snap = DriverSnapshot {
            seq: 0,
            atmega: raw.clone(),
            usb: usb.clone(),
        };
        let (watch_tx, watch_rx) = watch::channel(initial_snap);

        let mode_val = match mode {
            SupervisionMode::Remote => 0,
            SupervisionMode::Local => 1,
        };

        Self {
            side: AtomicU8::new(side),
            mode: AtomicU8::new(mode_val),
            seq: AtomicU64::new(0),
            raw: Mutex::new(raw),
            usb: Mutex::new(usb),
            tic_detection: Mutex::new(TicDetectionState::default()),
            serial_tx,
            watch_tx,
            watch_rx,
        }
    }

    /// Returns the active side number (`1` or `2`).
    pub fn side(&self) -> u8 {
        self.side.load(Ordering::Relaxed)
    }

    /// Updates the side number once discovered from `Side:`.
    pub fn set_side(&self, s: u8) {
        self.side.store(s, Ordering::Relaxed);
        self.update_raw(|raw| {
            raw.side = Some(s);
        });
    }

    /// Returns the current UART supervision mode (`Remote` or `Local`).
    pub fn mode(&self) -> SupervisionMode {
        if self.mode.load(Ordering::Relaxed) == 1 {
            SupervisionMode::Local
        } else {
            SupervisionMode::Remote
        }
    }

    /// Updates the UART supervision mode and broadcasts the updated snapshot.
    pub fn set_mode(&self, mode: SupervisionMode) {
        let mode_val = match mode {
            SupervisionMode::Remote => 0,
            SupervisionMode::Local => 1,
        };
        self.mode.store(mode_val, Ordering::Relaxed);
        self.update_usb(|usb| {
            usb.mode = mode;
        });
    }

    /// Returns `true` if the USB serial link is actively communicating with the ATmega.
    pub fn is_serial_connected(&self) -> bool {
        self.usb
            .lock()
            .map(|u| u.status == SerialStatus::UsbConnected)
            .unwrap_or(false)
    }

    /// Returns a clone of the current [`AtmegaRawState`].
    pub fn get_raw_state(&self) -> AtmegaRawState {
        self.raw.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Returns a clone of the current [`UsbLinkState`].
    pub fn get_usb_state(&self) -> UsbLinkState {
        self.usb.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Returns a clone of the latest combined [`DriverSnapshot`].
    pub fn get_snapshot(&self) -> DriverSnapshot {
        self.watch_rx.borrow().clone()
    }

    /// Returns the current [`DriverSnapshot`] serialized as a `serde_json::Value`.
    pub fn get_snapshot_json(&self) -> serde_json::Value {
        self.get_snapshot().to_json()
    }

    /// Subscribes to real-time [`DriverSnapshot`] updates via a `tokio::sync::watch::Receiver`.
    pub fn subscribe(&self) -> watch::Receiver<DriverSnapshot> {
        self.watch_rx.clone()
    }

    /// Mutates [`AtmegaRawState`] and immediately broadcasts the new [`DriverSnapshot`].
    /// Returns the value returned by closure `f`.
    pub fn update_raw<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut AtmegaRawState) -> R,
    {
        let res = {
            let mut raw_guard = self.raw.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut raw_guard)
        };
        self.publish_snapshot();
        res
    }

    /// Mutates [`UsbLinkState`] and immediately broadcasts the new [`DriverSnapshot`].
    pub fn update_usb<F>(&self, f: F)
    where
        F: FnOnce(&mut UsbLinkState),
    {
        {
            let mut usb_guard = self.usb.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut usb_guard);
        }
        self.publish_snapshot();
    }

    /// Mutates both [`AtmegaRawState`] and [`UsbLinkState`] atomically and broadcasts a single [`DriverSnapshot`].
    pub fn update_both<F>(&self, f: F)
    where
        F: FnOnce(&mut AtmegaRawState, &mut UsbLinkState),
    {
        {
            let mut raw_guard = self.raw.lock().unwrap_or_else(|e| e.into_inner());
            let mut usb_guard = self.usb.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut raw_guard, &mut usb_guard);
        }
        self.publish_snapshot();
    }

    /// Publishes the current `raw` and `usb` states to all `watch` subscribers.
    pub fn publish_snapshot(&self) {
        let raw_clone = self.raw.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let usb_clone = self.usb.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let next_seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let snap = DriverSnapshot {
            seq: next_seq,
            atmega: raw_clone,
            usb: usb_clone,
        };
        let _ = self.watch_tx.send_replace(snap);
    }

    /// Queues a command to be transmitted over the serial link.
    pub async fn send_command(
        &self,
        cmd: greenup_protocol::Command,
    ) -> Result<(), mpsc::error::SendError<greenup_protocol::Command>> {
        self.serial_tx.send(cmd).await
    }

    /// Attempts to queue a command without awaiting.
    pub fn try_send_command(
        &self,
        cmd: greenup_protocol::Command,
    ) -> Result<(), mpsc::error::TrySendError<greenup_protocol::Command>> {
        self.serial_tx.try_send(cmd)
    }
}

pub type SharedDriver = Arc<DriverHandle>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_driver_handle_watch_notifications() {
        let (tx, _rx) = mpsc::channel(8);
        let driver = Arc::new(DriverHandle::new_with_side_and_mode(
            1,
            SupervisionMode::Remote,
            tx,
        ));
        let mut sub = driver.subscribe();

        assert_eq!(sub.borrow().seq, 0);
        assert_eq!(driver.mode(), SupervisionMode::Remote);

        driver.update_raw(|raw| {
            raw.reference = Some("058001".to_string());
            raw.state = Some("A".to_string());
        });

        sub.changed().await.unwrap();
        let snap = sub.borrow().clone();
        assert_eq!(snap.seq, 1);
        assert_eq!(snap.atmega.reference.as_deref(), Some("058001"));
        assert_eq!(snap.atmega.state.as_deref(), Some("A"));

        driver.set_mode(SupervisionMode::Local);
        sub.changed().await.unwrap();
        assert_eq!(sub.borrow().usb.mode, SupervisionMode::Local);
    }
}
