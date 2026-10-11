# OSC Input (Hardware Control)

## Overview

Add general-purpose OSC (Open Sound Control) support for receiving real-time control signals from hardware controllers, software instruments, and custom bridges. Signals appear in the Signal panel and can drive animation tracks.

This enables USB mixers, MIDI controllers, touchscreen apps (TouchOSC), and custom Python scripts to control fractal parameters in real-time.

## Motivation

The animation system supports `TrackSource::Signal` for mapping signals to config parameters. Currently the only signal sources are audio analysis and procedural generators. Hardware controllers (faders, knobs, buttons) are the natural next step for live performance and interactive exploration.

OSC is the standard protocol for this in creative tools (VJ software, DAWs, lighting). It's UDP-based, low-latency, and widely supported.

## Architecture

### Data Flow

```
Hardware → Software → UDP :9000 → OscListener thread → SharedState → SignalManager → Animation tracks
```

Examples:
- USB Mixer → TouchOSC → UDP → FFlame
- MIDI Controller → Python bridge script → UDP → FFlame
- Phone touchscreen → TouchOSC app → WiFi UDP → FFlame
- Ableton Live → OSC output plugin → UDP → FFlame

### Module Structure

```
src/osc/
  mod.rs              - OscListener struct, SignalProducer impl, start/stop
  listener.rs         - Background thread, UDP socket, rosc packet parsing
```

### Dependencies

Desktop only (no WASM — browsers can't receive UDP):

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
rosc = "0.10"   # OSC packet parsing (MIT, lightweight, no dependencies)
```

## Design

### OscListener

```rust
pub struct OscListener {
    /// Shared signal values: OSC address → latest f32 value
    signals: Arc<Mutex<HashMap<String, f32>>>,

    /// Background thread handle
    thread: Option<JoinHandle<()>>,

    /// Stop flag for clean shutdown
    running: Arc<AtomicBool>,

    /// UDP port (configurable, default 9000)
    port: u16,

    /// Whether user has activated this in the Signals panel
    active: bool,
}
```

### SignalProducer Implementation

`OscListener` implements the existing `SignalProducer` trait:

- **`signal_names()`** — Returns all discovered OSC addresses (keys from shared HashMap). Addresses appear automatically as messages arrive — no configuration needed.
- **`get_live_value(name)`** — Reads latest value from shared HashMap. Lock is brief (single HashMap lookup).
- **`get_signal(name)`** — Returns `Signal { name, signal_type: Continuous, .. }` with single-sample data. OSC signals are live-only (no history buffer).
- **`is_active()`** — Returns `true` only when the listener thread is running.

### Background Thread

```rust
fn listener_thread(
    port: u16,
    signals: Arc<Mutex<HashMap<String, f32>>>,
    running: Arc<AtomicBool>,
) {
    let socket = UdpSocket::bind(("0.0.0.0", port)).unwrap();
    socket.set_read_timeout(Some(Duration::from_millis(100)));

    let mut buf = [0u8; 4096];
    while running.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buf) {
            Ok((size, _addr)) => {
                if let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..size]) {
                    process_packet(&packet, &signals);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => break,
        }
    }
}
```

Key details:
- **100ms read timeout** allows clean shutdown (check `running` flag between reads)
- **Thread only runs when activated** in the Signals panel — zero overhead otherwise
- **Any number of senders** can send to the same port simultaneously
- **No handshake** — OSC is fire-and-forget UDP

### OSC Packet Processing

```rust
fn process_packet(packet: &OscPacket, signals: &Arc<Mutex<HashMap<String, f32>>>) {
    match packet {
        OscPacket::Message(msg) => {
            if let Some(value) = extract_f32(&msg.args) {
                signals.lock().unwrap().insert(msg.addr.clone(), value);
            }
        }
        OscPacket::Bundle(bundle) => {
            for p in &bundle.content {
                process_packet(p, signals);
            }
        }
    }
}

fn extract_f32(args: &[OscType]) -> Option<f32> {
    args.first().and_then(|arg| match arg {
        OscType::Float(f) => Some(*f),
        OscType::Int(i) => Some(*i as f32),
        OscType::Double(d) => Some(*d as f32),
        _ => None,
    })
}
```

- Handles both flat messages and bundles (bundles are common from DAWs)
- Takes only the first numeric argument per message (standard for faders/knobs)
- OSC addresses become signal names directly (e.g., `/1/fader1` → signal name `/1/fader1`)
- Auto-discovery: no predefined address list needed

### SignalManager Integration

When the user activates OSC in the Signals panel:

```rust
let osc_listener = OscListener::new(port);
osc_listener.start(); // spawns background thread
signal_manager.add_producer(Box::new(osc_listener));
```

When deactivated:
```rust
osc_listener.stop(); // sets running=false, joins thread
// Producer stays registered but is_active() returns false
```

SignalManager already checks `is_active()` before polling producers, so an inactive OscListener has zero cost.

## UI (Signals Panel)

Add an "OSC Input" collapsible section to the Signals panel:

```
Signal Panel
├─ Audio (existing)
├─ Signal Generators (existing)
├─ OSC Input (new)
│  ├─ [Enable] toggle + port field (default 9000)
│  ├─ Status: "Listening on :9000" / "Stopped"
│  ├─ Discovered signals:
│  │  ├─ /1/fader1: 0.75  ████████░░
│  │  ├─ /1/fader2: 0.30  ███░░░░░░░
│  │  └─ /1/rotary1: 0.50 █████░░░░░
│  └─ [Clear] button (removes discovered addresses)
├─ Signal Files (existing)
└─ Signal Monitor (existing)
```

- Port is only editable when stopped
- Discovered signals show live value bars (same style as signal monitor)
- Signals are available for mapping in animation tracks as soon as they appear

## Latency

| Stage | Latency |
|-------|---------|
| UDP receive | <1ms |
| Mutex lock + HashMap write | <0.01ms |
| SignalManager poll (per frame) | ~16ms at 60fps |
| **Total worst case** | ~17ms |

For tighter latency, increasing the app's target FPS reduces the poll interval. At 120fps the worst case drops to ~9ms.

## Python Bridge

Ship `scripts/osc_bridge.py` as an example for MIDI→OSC conversion:

```python
"""Forward MIDI CC messages to OSC for FFlame.

Requirements: pip install python-osc mido python-rtmidi
"""
from pythonosc import udp_client
import mido

client = udp_client.SimpleUDPClient("127.0.0.1", 9000)

print("Available MIDI inputs:", mido.get_input_names())
with mido.open_input() as port:
    print(f"Listening on: {port.name}")
    for msg in port:
        if msg.type == "control_change":
            addr = f"/midi/cc/{msg.control}"
            value = msg.value / 127.0
            client.send_message(addr, value)
            print(f"{addr} = {value:.3f}")
```

Additional example scripts could cover:
- `osc_bridge_gamepad.py` — Gamepad axes/buttons via `inputs` library
- `osc_bridge_sensors.py` — Arduino/sensor data via serial

## Files Modified

| File | Change |
|------|--------|
| `Cargo.toml` | Add `rosc = "0.10"` (desktop only) |
| `src/osc/mod.rs` | New — `OscListener`, `SignalProducer` impl, start/stop |
| `src/osc/listener.rs` | New — Background thread, UDP socket, packet parsing |
| `src/lib.rs` | Add `pub mod osc;` (behind `#[cfg(not(target_arch = "wasm32"))]`) |
| `src/signal/mod.rs` | Register `OscListener` as a producer |
| `src/ui/signal_panel.rs` | Add OSC Input section |
| `scripts/osc_bridge.py` | New — Example MIDI→OSC bridge |

## Not In Scope

- **Direct MIDI parsing in Rust** — Use Python bridge instead (keeps binary small, MIDI libraries are platform-specific)
- **WASM support** — Browsers can't receive UDP. Could add WebSocket bridge later if needed.
- **Persistent OSC settings** — Can add to SystemSettings later (port, auto-start)
- **OSC output/send** — Only receiving for now
- **OSC query protocol** — Auto-discovery of remote device capabilities (overkill for v1)

## Future Extensions

- **Auto-start option** in SystemSettings (start listening on app launch)
- **Address filtering** — Include/exclude patterns for noisy senders
- **Value history** — Ring buffer per signal for sparkline display
- **WebSocket bridge** — For WASM builds, a small local server could relay OSC→WebSocket
- **Direct MIDI** — If demand justifies it, add `midir` crate for zero-config MIDI input
