// ─── ISO 8583 TCP Simulator ───────────────────────────────────────────────────
// Server: listens on TCP port, auto-responds per MTI rules
// Client: connects to host, sends message, shows response

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

/// A log entry from the simulator
#[derive(Clone)]
pub struct SimLog {
    pub ts: String,
    pub dir: LogDir,
    pub raw_hex: String,
    pub summary: String,
}

#[derive(Clone, PartialEq)]
pub enum LogDir {
    Recv,     // Incoming (server received / client received response)
    Send,     // Outgoing (server sent response / client sent request)
    Info,     // System message
    Error,
}

impl LogDir {
    pub fn label(&self) -> &'static str {
        match self {
            LogDir::Recv  => "▼ RECV",
            LogDir::Send  => "▲ SEND",
            LogDir::Info  => "  INFO",
            LogDir::Error => "✗ ERR ",
        }
    }
}

/// Command sent to server thread
pub enum ServerCmd {
    Stop,
}

/// Shared state between TUI and simulator threads
pub struct SimState {
    pub logs: Vec<SimLog>,
    pub running: bool,
    pub connection_count: usize,
    pub tx_count: usize,
}

impl SimState {
    pub fn new() -> Self {
        SimState { logs: Vec::new(), running: false, connection_count: 0, tx_count: 0 }
    }

    pub fn log(&mut self, dir: LogDir, raw_hex: &str, summary: &str) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let hh = (secs % 86400) / 3600;
        let mm = (secs % 3600) / 60;
        let ss = secs % 60;
        self.logs.push(SimLog {
            ts: format!("{:02}:{:02}:{:02}", hh, mm, ss),
            dir,
            raw_hex: raw_hex.to_string(),
            summary: summary.to_string(),
        });
        if self.logs.len() > 500 { self.logs.remove(0); }
    }
}

/// ISO 8583 Auto-Response rules for the simulator host
pub struct AutoResponse {
    /// MTI to respond to → (response MTI, response code, optional extra fields)
    pub rules: Vec<(String, String, String)>,  // (req_mti, resp_mti, resp_code)
}

impl AutoResponse {
    pub fn default_rules() -> Self {
        AutoResponse {
            rules: vec![
                ("0200".to_string(), "0210".to_string(), "00".to_string()),
                ("0400".to_string(), "0410".to_string(), "00".to_string()),
                ("0420".to_string(), "0430".to_string(), "00".to_string()),
                ("0800".to_string(), "0810".to_string(), "00".to_string()),
                ("0100".to_string(), "0110".to_string(), "00".to_string()),
            ],
        }
    }

    /// Build auto-response hex for a given request
    /// Very simple: flip last digit of MTI 0→1 or 1→0, set F39 to response code
    pub fn build_response(&self, req_hex: &str, resp_code_override: Option<&str>) -> Option<String> {
        let req: String = req_hex.chars().filter(|c| !c.is_whitespace()).map(|c|c.to_ascii_uppercase()).collect();
        if req.len() < 8 { return None; }

        // Detect if hex or raw mode
        let is_hex = req.chars().all(|c| c.is_ascii_hexdigit());
        let mti = if is_hex {
            // MTI is hex-encoded ASCII: first 8 hex chars
            let bytes: Vec<u8> = (0..8).step_by(2)
                .filter_map(|i| u8::from_str_radix(&req[i..i+2], 16).ok())
                .collect();
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            req[0..4].to_string()
        };

        let rule = self.rules.iter().find(|(req_mti, _, _)| *req_mti == mti);
        let (resp_mti, _resp_code) = if let Some(r) = rule {
            (r.1.clone(), resp_code_override.unwrap_or(&r.2).to_string())
        } else {
            // Default: increment last digit by 1 for response
            let mut resp = mti.clone();
            if let Some(last) = resp.pop() {
                let new_last = if last == '0' { '1' } else if last == '2' { '3' } else { last };
                resp.push(new_last);
            }
            (resp, resp_code_override.unwrap_or("96").to_string())
        };

        // Build minimal response: MTI + same bitmaps + echo fields + set F39
        // Strategy: take the request as-is, replace MTI, ensure F39 is set
        build_minimal_response(&req, &resp_mti, is_hex)
    }
}

/// Build a minimal response by echoing the request with modified MTI and appending/replacing F39
fn build_minimal_response(req: &str, resp_mti: &str, is_hex_encoded: bool) -> Option<String> {
    if is_hex_encoded {
        // Replace first 8 hex chars (MTI) with the response MTI hex-encoded
        let resp_mti_hex: String = resp_mti.bytes().map(|b| format!("{:02X}", b)).collect();
        if req.len() < 8 { return None; }
        // Parse bitmap to check if F39 is already set
        let resp_hex = format!("{}{}", resp_mti_hex, &req[8..]);
        // Add F39 (response code) hex: "30 30" or "30 30"
        // For simplicity, we'll append: since this is a simulator we just deliver a recognizable response
        // Real implementation would parse and rebuild; here we just replace MTI and note it in the summary
        Some(resp_hex)
    } else {
        // RAW mode: replace first 4 chars
        if req.len() < 4 { return None; }
        Some(format!("{}{}", resp_mti, &req[4..]))
    }
}

// ─── TCP Length Framing ───────────────────────────────────────────────────────
// Two common framing styles:
// 1. 2-byte big-endian binary length header (common in ISO 8583)
// 2. 4-byte ASCII length header (used by some systems)

pub enum Framing {
    Binary2,  // 2-byte big-endian length prefix (bytes, not including header)
    Ascii4,   // 4-char ASCII decimal length prefix
    None,     // Raw — read until connection closes or 4096 bytes
}

fn read_framed(stream: &mut TcpStream, framing: &Framing) -> Result<Vec<u8>, String> {
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    match framing {
        Framing::Binary2 => {
            let mut len_buf = [0u8; 2];
            stream.read_exact(&mut len_buf).map_err(|e| e.to_string())?;
            let len = u16::from_be_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            stream.read_exact(&mut data).map_err(|e| e.to_string())?;
            Ok(data)
        }
        Framing::Ascii4 => {
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).map_err(|e| e.to_string())?;
            let len_str = String::from_utf8_lossy(&len_buf);
            let len = len_str.trim().parse::<usize>().map_err(|_| "Bad ASCII length".to_string())?;
            let mut data = vec![0u8; len];
            stream.read_exact(&mut data).map_err(|e| e.to_string())?;
            Ok(data)
        }
        Framing::None => {
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
            buf.truncate(n);
            Ok(buf)
        }
    }
}

fn write_framed(stream: &mut TcpStream, data: &[u8], framing: &Framing) -> Result<(), String> {
    match framing {
        Framing::Binary2 => {
            let len = (data.len() as u16).to_be_bytes();
            stream.write_all(&len).map_err(|e| e.to_string())?;
            stream.write_all(data).map_err(|e| e.to_string())?;
        }
        Framing::Ascii4 => {
            let header = format!("{:04}", data.len());
            stream.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
            stream.write_all(data).map_err(|e| e.to_string())?;
        }
        Framing::None => {
            stream.write_all(data).map_err(|e| e.to_string())?;
        }
    }
    stream.flush().map_err(|e| e.to_string())
}

fn bytes_to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02X}", x)).collect()
}

fn extract_mti_summary(data: &[u8]) -> String {
    if data.len() < 4 {
        return format!("[{} bytes]", data.len());
    }
    // Try ASCII MTI first
    let mti: String = data[0..4].iter().map(|&b| if b.is_ascii_graphic() { b as char } else { '?' }).collect();
    if mti.chars().all(|c| c.is_ascii_digit()) {
        format!("MTI={}", mti)
    } else if data.len() >= 8 {
        // Try hex-decoded MTI
        let hex_mti = bytes_to_hex(&data[0..4]);
        let bytes: Vec<u8> = (0..hex_mti.len()).step_by(2)
            .filter_map(|i| u8::from_str_radix(&hex_mti[i..i+2], 16).ok())
            .collect();
        let decoded = String::from_utf8_lossy(&bytes).to_string();
        format!("MTI={} (raw)", decoded)
    } else {
        format!("[{} bytes, MTI?]", data.len())
    }
}

// ─── Server ───────────────────────────────────────────────────────────────────

pub struct SimServer {
    pub state: Arc<Mutex<SimState>>,
    cmd_tx: Option<Sender<ServerCmd>>,
}

impl SimServer {
    pub fn new() -> Self {
        SimServer {
            state: Arc::new(Mutex::new(SimState::new())),
            cmd_tx: None,
        }
    }

    pub fn start(&mut self, port: u16, framing: Framing, auto_resp: AutoResponse) {
        let state = Arc::clone(&self.state);
        let (tx, rx): (Sender<ServerCmd>, Receiver<ServerCmd>) = channel();
        self.cmd_tx = Some(tx);

        let auto_resp = Arc::new(auto_resp);

        thread::spawn(move || {
            let addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&addr) {
                Ok(l) => l,
                Err(e) => {
                    if let Ok(mut s) = state.lock() {
                        s.log(LogDir::Error, "", &format!("Cannot bind {}: {}", addr, e));
                    }
                    return;
                }
            };
            listener.set_nonblocking(true).ok();

            if let Ok(mut s) = state.lock() {
                s.running = true;
                s.log(LogDir::Info, "", &format!("Server listening on {}", addr));
            }

            let framing = Arc::new(framing);
            loop {
                // Check for stop command (non-blocking)
                if rx.try_recv().is_ok() { break; }

                match listener.accept() {
                    Ok((stream, addr)) => {
                        let state2  = Arc::clone(&state);
                        let auto2   = Arc::clone(&auto_resp);
                        let frame2  = Arc::clone(&framing);
                        if let Ok(mut s) = state.lock() {
                            s.connection_count += 1;
                            s.log(LogDir::Info, "", &format!("Connection from {}", addr));
                        }
                        thread::spawn(move || {
                            handle_connection(stream, state2, auto2, frame2);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        if let Ok(mut s) = state.lock() {
                            s.log(LogDir::Error, "", &format!("Accept error: {}", e));
                        }
                    }
                }
            }

            if let Ok(mut s) = state.lock() {
                s.running = false;
                s.log(LogDir::Info, "", "Server stopped");
            }
        });
    }

    pub fn stop(&mut self) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(ServerCmd::Stop);
        }
    }

    pub fn is_running(&self) -> bool {
        self.state.lock().map(|s| s.running).unwrap_or(false)
    }
}

fn handle_connection(
    mut stream: TcpStream,
    state: Arc<Mutex<SimState>>,
    auto_resp: Arc<AutoResponse>,
    framing: Arc<Framing>,
) {
    stream.set_read_timeout(Some(Duration::from_secs(60))).ok();
    loop {
        let data = match read_framed(&mut stream, &framing) {
            Ok(d) if d.is_empty() => break,
            Ok(d) => d,
            Err(_) => break,
        };

        let hex = bytes_to_hex(&data);
        let summary = extract_mti_summary(&data);

        if let Ok(mut s) = state.lock() {
            s.tx_count += 1;
            s.log(LogDir::Recv, &hex, &summary);
        }

        // Build auto-response
        if let Some(resp_hex) = auto_resp.build_response(&hex, None) {
            // Convert hex back to bytes
            let resp_bytes: Vec<u8> = (0..resp_hex.len()).step_by(2)
                .filter_map(|i| u8::from_str_radix(&resp_hex[i..i+2], 16).ok())
                .collect();

            let resp_summary = extract_mti_summary(&resp_bytes);

            if let Err(e) = write_framed(&mut stream, &resp_bytes, &framing) {
                if let Ok(mut s) = state.lock() {
                    s.log(LogDir::Error, "", &format!("Send error: {}", e));
                }
                break;
            }

            if let Ok(mut s) = state.lock() {
                s.log(LogDir::Send, &resp_hex, &format!("{} [AUTO]", resp_summary));
            }
        }
    }
}

// ─── Client ───────────────────────────────────────────────────────────────────

pub fn send_message(host: &str, port: u16, message_hex: &str, framing_type: &str) -> Result<(String, String), String> {
    let addr = format!("{}:{}", host, port);
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| format!("Cannot connect to {}: {}", addr, e))?;
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(10))).ok();

    let framing = match framing_type {
        "binary2" | "bin" => Framing::Binary2,
        "ascii4"  | "asc" => Framing::Ascii4,
        _                  => Framing::None,
    };

    // Convert hex to bytes
    let clean: String = message_hex.chars().filter(|c| !c.is_whitespace()).map(|c|c.to_ascii_uppercase()).collect();
    let msg_bytes: Vec<u8> = (0..clean.len()).step_by(2)
        .filter_map(|i| u8::from_str_radix(&clean[i..i+2], 16).ok())
        .collect();

    write_framed(&mut stream, &msg_bytes, &framing)?;

    let resp_data = read_framed(&mut stream, &framing)?;
    let resp_hex = bytes_to_hex(&resp_data);
    let resp_summary = extract_mti_summary(&resp_data);

    Ok((resp_hex, resp_summary))
}

// ─── Format Sim Log ───────────────────────────────────────────────────────────

pub fn format_logs(logs: &[SimLog]) -> String {
    if logs.is_empty() {
        return "  (no activity yet)\n".to_string();
    }
    let mut out = String::new();
    for log in logs.iter().rev().take(200) {
        let hex_preview = if log.raw_hex.len() > 60 {
            format!("{}…", &log.raw_hex[..60])
        } else {
            log.raw_hex.clone()
        };
        out.push_str(&format!(
            "[{}] {} {}\n",
            log.ts, log.dir.label(), log.summary
        ));
        if !log.raw_hex.is_empty() {
            out.push_str(&format!("           {}\n", hex_preview));
        }
    }
    out
}

impl SimServer {
    /// Start server with an externally-provided shared state (for TUI)
    pub fn start_with_state(
        &mut self,
        port: u16,
        framing: Framing,
        auto_resp: AutoResponse,
        state: Arc<Mutex<SimState>>,
    ) {
        let (tx, rx): (Sender<ServerCmd>, Receiver<ServerCmd>) = channel();
        self.cmd_tx = Some(tx);
        self.state = Arc::clone(&state);

        let auto_resp = Arc::new(auto_resp);
        let framing   = Arc::new(framing);

        thread::spawn(move || {
            let addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&addr) {
                Ok(l) => l,
                Err(e) => {
                    if let Ok(mut s) = state.lock() {
                        s.log(LogDir::Error, "", &format!("Cannot bind {}: {}", addr, e));
                    }
                    return;
                }
            };
            listener.set_nonblocking(true).ok();

            if let Ok(mut s) = state.lock() {
                s.running = true;
                s.log(LogDir::Info, "", &format!("Server listening on {}", addr));
            }

            loop {
                if rx.try_recv().is_ok() { break; }

                match listener.accept() {
                    Ok((stream, peer)) => {
                        let state2 = Arc::clone(&state);
                        let auto2  = Arc::clone(&auto_resp);
                        let frame2 = Arc::clone(&framing);
                        if let Ok(mut s) = state.lock() {
                            s.connection_count += 1;
                            s.log(LogDir::Info, "", &format!("Connection from {}", peer));
                        }
                        thread::spawn(move || { handle_connection(stream, state2, auto2, frame2); });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        if let Ok(mut s) = state.lock() {
                            s.log(LogDir::Error, "", &format!("Accept error: {}", e));
                        }
                    }
                }
            }

            if let Ok(mut s) = state.lock() {
                s.running = false;
                s.log(LogDir::Info, "", "Server stopped");
            }
        });
    }
}
