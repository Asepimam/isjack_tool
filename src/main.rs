#![allow(dead_code)]
//! ISJack-Tools — Payment Gateway Toolkit
//! Entry point: event loop, key dispatch, sample data, action processors.

mod app;
mod ui;
mod json_tool;
mod iso8583;
mod iso8583_encode;
mod tlv;
mod keymgmt;
mod simulator;
mod settlement;

use app::{ActiveTab, App, Focus, IsoMode, JsonMode, KeyOp, SimMode};
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture,
        EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use simulator::{AutoResponse, Framing};
use std::{io, time::Duration};

// ─── ISO 8583 Samples ─────────────────────────────────────────────────────────

const ISO_SAMPLES: &[(&str, &str)] = &[
    (
        "HEX | 0200 Purchase Auth (hex-encoded ASCII)",
        "30323030723800000080000031363431313131313131313131313131313130303030303030303030303030303130303030333133313735303030303030303031313735303030303331335445524D30303031",
    ),
    (
        "HEX | 0810 Network Mgmt Response (F7/F11/F12/F39)",
        "303831300230000002000000303331333137353030313030303030313137353030313030",
    ),
];

// ─── JSON Samples ─────────────────────────────────────────────────────────────

const JSON_SAMPLES: &[(&str, &str)] = &[
    (
        "E-Commerce Transaction",
        r#"{"transaction":{"id":"TXN-20260313-001","type":"purchase","amount":150000,"currency":"IDR","status":"approved","card":{"pan":"411111****1111","expiry":"12/28","scheme":"VISA"},"merchant":{"id":"MERCH001","name":"Toko Online ABC","mcc":"5411"},"timestamp":"2026-03-13T09:01:00+07:00","auth_code":"AUTH123","rrn":"RRN000001"}}"#,
    ),
    (
        "ISO 8583 Field Map",
        r#"{"fields":{"002":{"name":"PAN","type":"LLVAR","max_len":19},"003":{"name":"Processing Code","type":"FIXED","len":6},"004":{"name":"Amount Transaction","type":"FIXED","len":12},"039":{"name":"Response Code","type":"FIXED","len":2},"041":{"name":"Terminal ID","type":"FIXED","len":8},"048":{"name":"Additional Data","type":"LLLVAR","max_len":999}}}"#,
    ),
    (
        "Settlement Batch",
        r#"{"batch":{"date":"2026-03-13","cutoff":"23:59:59","totals":{"debit":{"count":245,"amount":18750000},"credit":{"count":3,"amount":450000},"net":18300000},"currency":"IDR","terminals":["TERM001","TERM002","TERM003"]}}"#,
    ),
    (
        "Empty / minimal",
        r#"{}"#,
    ),
];

// ─── TLV Samples ─────────────────────────────────────────────────────────────

const TLV_SAMPLES: &[(&str, &str)] = &[
    (
        "EMV ARQC — typical F55 content",
        "9F2608A1B2C3D4E5F6A7829F2701809F101307010103A0B800F4A50000000000000000FF9F3704AABBCCDD9F360200579A032603139C015F9F02060000001500009F03060000000000009F1A0204609F4104000001125F3401019F0607A0000000031010",
    ),
    (
        "AFL — Application File Locator (3 records, SFI 1/2/3)",
        "940C080101001001010018010100",
    ),
    (
        "FCI with PDOL",
        "6F37840E325041592E5359532E4444463031A525BF0C229F4A01829F38139F0206A0000000041010AF0706A0000000031010BF0C039F5A0140",
    ),
    (
        "Track 2 + AIP + ATC (masked PAN shown as FF)",
        "570C476173FFFFFF4761D261220182025C008407A00000000310109F360200A1",
    ),
];

// ─── Key Management Samples ───────────────────────────────────────────────────

const KEY_SAMPLES_KCV: &[(&str, &str, &str, &str)] = &[
    ("2-key 3DES ZPK",        "0123456789ABCDEFFEDCBA9876543210", "", ""),
    ("Single DES (8 bytes)",  "0133456789ABCDEF",                 "", ""),
    ("3-key 3DES (24 bytes)", "0123456789ABCDEFFEDCBA98765432100123456789ABCDEF", "", ""),
];
const KEY_SAMPLES_TDES_ENC: &[(&str, &str, &str, &str)] = &[
    ("Encrypt zeros",    "0123456789ABCDEFFEDCBA9876543210", "0000000000000000", ""),
    ("Encrypt PAN data", "0123456789ABCDEFFEDCBA9876543210", "AABBCCDDEEFF0011", ""),
];
const KEY_SAMPLES_TDES_DEC: &[(&str, &str, &str, &str)] = &[
    ("Decrypt ciphertext", "0123456789ABCDEFFEDCBA9876543210", "0E329232EA6D0D73", ""),
];
const KEY_SAMPLES_PIN_BUILD: &[(&str, &str, &str, &str)] = &[
    ("PIN 1234 / Visa test PAN", "1234", "4761739001010010", ""),
    ("PIN 9999 / MC test PAN",   "9999", "5500005555555559", ""),
    ("PIN 0000 / BCA test PAN",  "0000", "4026840505840000", ""),
];
const KEY_SAMPLES_PIN_DECRYPT: &[(&str, &str, &str, &str)] = &[
    ("Decrypt sample", "0123456789ABCDEFFEDCBA9876543210", "0412AC3A2B1FC6D8", "4761739001010010"),
];
const KEY_SAMPLES_XOR: &[(&str, &str, &str, &str)] = &[
    ("Two key components", "0123456789ABCDEF0123456789ABCDEF", "FEDCBA9876543210FEDCBA9876543210", ""),
    ("Derive session key", "A1B2C3D4E5F60718",                 "0807F6E5D4C3B2A1",                 ""),
];
const KEY_SAMPLES_LUHN: &[(&str, &str, &str, &str)] = &[
    ("Visa test",       "4111111111111111", "", ""),
    ("Mastercard test", "5500005555555559", "", ""),
    ("Amex test",       "378282246310005",  "", ""),
    ("BCA Visa sample", "4026840505840123", "", ""),
    ("Invalid Luhn",    "1234567890123456", "", ""),
];

fn key_samples_for_op(op: KeyOp) -> &'static [(&'static str, &'static str, &'static str, &'static str)] {
    match op {
        KeyOp::Kcv         => KEY_SAMPLES_KCV,
        KeyOp::TdesEncrypt => KEY_SAMPLES_TDES_ENC,
        KeyOp::TdesDecrypt => KEY_SAMPLES_TDES_DEC,
        KeyOp::PinBuild    => KEY_SAMPLES_PIN_BUILD,
        KeyOp::PinDecrypt  => KEY_SAMPLES_PIN_DECRYPT,
        KeyOp::XorHex      => KEY_SAMPLES_XOR,
        KeyOp::LuhnBin     => KEY_SAMPLES_LUHN,
    }
}

// ─── Simulator Client Samples ─────────────────────────────────────────────────

const SIM_CLIENT_SAMPLES: &[(&str, &str)] = &[
    (
        "0800 Network Management / Sign-On (RAW)",
        "0800822000000000000000000003010000099999990000",
    ),
    (
        "0200 Purchase Auth (HEX-encoded)",
        "303230304220000102C08010000000000000313634303131313131313131313131313130303030303030313530303030",
    ),
   
];

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let mut app = App::new();
    load_all_samples(&mut app);

    loop {
        if app.active_tab == ActiveTab::Simulator {
            refresh_sim_log(&mut app);
        }

        term.draw(|f| ui::render(f, &mut app))?;

        if event::poll(Duration::from_millis(150))? {
            match event::read()? {
                // ── Bracketed paste — insert as one atomic operation ──────────
                Event::Paste(text) => {
                    handle_paste(&mut app, &text);
                }
                Event::Key(key) => {
                    handle_key(&mut app, key.code, key.modifiers);
                }
                _ => {}
            }
        }

        if app.should_quit { break; }
    }

    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste,
    )?;
    term.show_cursor()?;
    Ok(())
}

fn refresh_sim_log(app: &mut App) {
    if let Ok(state) = app.sim_state.lock() {
        if !state.logs.is_empty() {
            let text = simulator::format_logs(&state.logs);
            drop(state);
            app.sim_output.content = text;
        }
    }
}

// ─── Paste handler ────────────────────────────────────────────────────────────

fn handle_paste(app: &mut App, text: &str) {
    let buf = app.current_input();
    if buf.has_selection() {
        buf.delete_selection();
    }
    // Insert all chars; convert \n to insert_newline
    for ch in text.chars() {
        match ch {
            '\n' => buf.insert_newline(),
            '\r' => {}  // skip bare CR
            c    => buf.insert_char(c),
        }
    }
    let char_count = text.chars().filter(|&c| c != '\r').count();
    app.set_status(format!("Pasted {} chars", char_count), false);
}

// ─── Top-level key dispatcher ────────────────────────────────────────────────

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let ctrl       = mods.contains(KeyModifiers::CONTROL);
    let shift      = mods.contains(KeyModifiers::SHIFT);
    let ctrl_shift = ctrl && shift;

    // ── Ctrl+Shift shortcuts (copy/cut/paste — Linux convention) ────────────
    if ctrl_shift {
        match code {
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if let Some(text) = app.current_input().copy_selection() {
                    let len = text.chars().count();
                    app.clipboard = text;
                    app.set_status(format!("Copied {} chars  (Ctrl+Shift+V to paste)", len), false);
                }
                return;
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                let deleted = app.current_input().delete_selection();
                if !deleted.is_empty() {
                    let len = deleted.chars().count();
                    app.clipboard = deleted;
                    app.set_status(format!("Cut {} chars  (Ctrl+Shift+V to paste)", len), false);
                }
                return;
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                let text = app.clipboard.clone();
                if !text.is_empty() {
                    handle_paste(app, &text);
                } else {
                    app.set_status("Clipboard empty — paste from terminal is handled automatically", false);
                }
                return;
            }
            _ => {}
        }
    }

    // ── Plain Ctrl shortcuts ─────────────────────────────────────────────────
    if ctrl && !shift {
        match code {
            KeyCode::Char('q') => { app.should_quit = true; return; }
            KeyCode::Char('l') => {
                app.current_input().clear_selection();
                app.current_input().clear();
                app.current_output().set(String::new());
                app.set_status("Cleared", false);
                return;
            }
            KeyCode::Char('a') => {
                app.current_input().select_all();
                app.set_status("All selected — Ctrl+Shift+C copy, Ctrl+Shift+X cut, Del remove", false);
                return;
            }
            _ => {}
        }
    }

    // ── Shift+Arrow — extend selection ──────────────────────────────────────
    if shift && !ctrl {
        match code {
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down |
            KeyCode::Home | KeyCode::End => {
                let buf = app.current_input();
                edit_buf_shift(buf, code);
                return;
            }
            _ => {}
        }
    }

    // ── Tab switching F1-F6 ─────────────────────────────────────────────────
    if !ctrl && !shift {
        match code {
            KeyCode::F(1) => { switch_tab(app, ActiveTab::Json,       "JSON Beautify/Minify"); return; }
            KeyCode::F(2) => { switch_tab(app, ActiveTab::Iso8583,    "ISO 8583 Decoder"); return; }
            KeyCode::F(3) => { switch_tab(app, ActiveTab::Tlv,        "TLV/EMV Decoder"); return; }
            KeyCode::F(4) => { switch_tab(app, ActiveTab::KeyMgmt,    "Key Management"); return; }
            KeyCode::F(5) => { switch_tab(app, ActiveTab::Simulator,  "ISO 8583 Simulator"); return; }
            KeyCode::F(6) => { switch_tab(app, ActiveTab::Settlement, "Settlement & Reconciliation"); return; }
            _ => {}
        }
    }

    // ── Per-tab handlers ────────────────────────────────────────────────────
    match app.active_tab {
        ActiveTab::Json       => handle_json(app, code),
        ActiveTab::Iso8583    => handle_iso(app, code),
        ActiveTab::Tlv        => handle_tlv(app, code),
        ActiveTab::KeyMgmt    => handle_key_mgmt(app, code),
        ActiveTab::Simulator  => handle_simulator(app, code),
        ActiveTab::Settlement => handle_settlement(app, code),
    }
}

fn switch_tab(app: &mut App, tab: ActiveTab, label: &str) {
    app.active_tab = tab;
    app.focus      = Focus::Input;
    app.set_status(label, false);
}

// ─── JSON tab ────────────────────────────────────────────────────────────────

fn handle_json(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => toggle_focus(app),
        KeyCode::Char('s') => {
            app.json_mode = match app.json_mode { JsonMode::Beautify => JsonMode::Minify, JsonMode::Minify => JsonMode::Beautify };
            run_json(app);
        }
        KeyCode::Char('n') => {
            let idx = (app.sample_idx[0] + 1) % JSON_SAMPLES.len();
            app.sample_idx[0] = idx;
            load_json(app, idx);
        }
        KeyCode::Char(' ') => run_json(app),
        code => scroll_or_edit(app, code),
    }
}

fn run_json(app: &mut App) {
    let input  = app.json_input.get_text();
    let result = match app.json_mode { JsonMode::Beautify => json_tool::beautify(&input), JsonMode::Minify => json_tool::minify(&input) };
    let mode   = match app.json_mode { JsonMode::Beautify => "Beautify", JsonMode::Minify => "Minify" };
    if let Some(e) = result.error {
        app.json_output.set(format!("Error: {}", e));
        app.set_status(format!("JSON error: {}", e), true);
    } else {
        app.json_output.set(result.output);
        app.set_status(format!("JSON {} OK — {} fields, depth {}", mode, result.field_count, result.depth), false);
    }
}

// ─── ISO 8583 tab ────────────────────────────────────────────────────────────

fn handle_iso(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => toggle_focus(app),
        KeyCode::Char('d') => {
            app.iso_mode = match app.iso_mode { IsoMode::Hex => IsoMode::Raw, IsoMode::Raw => IsoMode::Hex };
            run_iso(app);
        }
        KeyCode::Char('n') => {
            let idx = (app.sample_idx[1] + 1) % ISO_SAMPLES.len();
            app.sample_idx[1] = idx;
            load_iso(app, idx);
        }
        KeyCode::Char(' ') => run_iso(app),
        code => scroll_or_edit(app, code),
    }
}

fn run_iso(app: &mut App) {
    let input  = app.iso_input.get_text();
    let result = match app.iso_mode { IsoMode::Hex => iso8583::decode(input.trim()), IsoMode::Raw => iso8583::decode_raw(input.trim()) };
    let mode   = match app.iso_mode { IsoMode::Hex => "HEX", IsoMode::Raw => "RAW/ASCII" };
    let out    = iso8583::format_result_with_mode(&result, mode);
    let ok     = result.errors.is_empty();
    app.iso_output.set(out);
    app.set_status(format!("ISO 8583 {} — {} fields, {} error(s)", mode, result.fields.len(), result.errors.len()), !ok);
}

// ─── TLV / EMV tab ───────────────────────────────────────────────────────────

fn handle_tlv(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => toggle_focus(app),
        KeyCode::Char('n') => {
            let idx = (app.sample_idx[2] + 1) % TLV_SAMPLES.len();
            app.sample_idx[2] = idx;
            load_tlv(app, idx);
        }
        KeyCode::Char(' ') | KeyCode::Enter => run_tlv(app),
        code => scroll_or_edit(app, code),
    }
}

fn run_tlv(app: &mut App) {
    let hex: String = app.tlv_input.get_text().chars().filter(|c| !c.is_whitespace()).collect();
    match tlv::decode(&hex) {
        Ok(nodes) => {
            let count = nodes.len();
            app.tlv_output.set(tlv::format_nodes(&nodes));
            app.set_status(format!("TLV decoded — {} top-level tag(s)", count), false);
        }
        Err(e) => {
            app.tlv_output.set(format!("TLV Error: {}\n\nInput ({} bytes):\n{}", e, hex.len()/2, hex));
            app.set_status(format!("TLV error: {}", e), true);
        }
    }
}

// ─── Key Management tab ──────────────────────────────────────────────────────

fn handle_key_mgmt(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => {
            if app.focus == Focus::Input {
                let max = app.key_op.active_field_count().saturating_sub(1);
                if app.key_focus_field < max { app.key_focus_field += 1; }
                else { app.focus = Focus::Output; }
            } else {
                app.focus = Focus::Input;
                app.key_focus_field = 0;
            }
        }
        KeyCode::Char('o') => {
            app.key_op = app.key_op.next();
            app.key_focus_field = 0;
            app.focus = Focus::Input;
            load_key_data(app, 0);
        }
        KeyCode::Char('n') => {
            let n = key_samples_for_op(app.key_op).len();
            let idx = (app.sample_idx[3] + 1) % n;
            load_key_data(app, idx);
        }
        KeyCode::Char(' ') => run_key_op(app),
        KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown if app.focus == Focus::Output => {
            scroll_output(app, code);
        }
        code => {
            if app.focus == Focus::Input {
                let f = app.key_focus_field.min(2) as usize;
                edit_buf(&mut app.key_field[f], code);
            }
        }
    }
}

fn run_key_op(app: &mut App) {
    let f0 = app.key_field[0].get_text().trim().to_string();
    let f1 = app.key_field[1].get_text().trim().to_string();
    let f2 = app.key_field[2].get_text().trim().to_string();
    let result: Result<String, String> = match app.key_op {
        KeyOp::Kcv         => keymgmt::kcv(&f0).map(|k| format!("Key   : {}\nKCV   : {}\nLength: {} bytes\n", f0, k, f0.replace(' ',"").len()/2)),
        KeyOp::TdesEncrypt => keymgmt::tdes_ecb_encrypt_hex(&f1, &f0).map(|o| format!("Key        : {}\nPlaintext  : {}\nCiphertext : {}\n", f0, f1, o)),
        KeyOp::TdesDecrypt => keymgmt::tdes_ecb_decrypt_hex(&f1, &f0).map(|o| format!("Key        : {}\nCiphertext : {}\nPlaintext  : {}\n", f0, f1, o)),
        KeyOp::PinBuild    => keymgmt::build_pin_block_iso0(&f0, &f1).map(|pb| format!("PIN       : {}\nPAN       : {}\nPIN Block : {} (ISO-0 Format 0)\n\nXOR with ZPK to get encrypted block.\n", f0, f1, pb)),
        KeyOp::PinDecrypt  => keymgmt::decrypt_pin_block(&f1, &f0, &f2).map(|p| format!("ZPK       : {}\nEnc Block : {}\nPAN       : {}\nPIN       : {}\n", f0, f1, f2, p)),
        KeyOp::XorHex      => keymgmt::xor_hex(&f0, &f1).map(|o| format!("A       : {}\nB       : {}\nA XOR B : {}\n", f0, f1, o)),
        KeyOp::LuhnBin     => Ok(keymgmt::bin_info(&f0)),
    };
    match result {
        Ok(out) => { app.key_output.set(out); app.set_status(format!("{} OK", app.key_op.label()), false); }
        Err(e)  => { app.key_output.set(format!("Error: {}", e)); app.set_status(e, true); }
    }
}

// ─── Simulator tab ───────────────────────────────────────────────────────────

fn handle_simulator(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => toggle_focus(app),
        KeyCode::Char('m') => {
            app.sim_mode = match app.sim_mode { SimMode::Server => SimMode::Client, SimMode::Client => SimMode::Server };
            app.set_status(format!("Mode: {}", match app.sim_mode { SimMode::Server => "Server", _ => "Client" }), false);
            // When switching to client, load first client sample
            if app.sim_mode == SimMode::Client {
                load_sim_client_sample(app, 0);
            }
        }
        KeyCode::Char('f') => {
            app.sim_framing = match app.sim_framing.as_str() { "binary2" => "ascii4", "ascii4" => "none", _ => "binary2" }.to_string();
            app.set_status(format!("Framing: {}", app.sim_framing), false);
        }
        KeyCode::Char('n') => {
            if app.sim_mode == SimMode::Client {
                let idx = (app.sample_idx[4] + 1) % SIM_CLIENT_SAMPLES.len();
                load_sim_client_sample(app, idx);
            }
        }
        KeyCode::Char(' ') => run_simulator(app),
        code => scroll_or_edit(app, code),
    }
}

fn run_simulator(app: &mut App) {
    match app.sim_mode {
        SimMode::Server => {
            if app.sim_server.is_running() {
                app.sim_server.stop();
                app.set_status("Server stopped", false);
            } else {
                let port: u16 = app.sim_port.parse().unwrap_or(8583);
                let framing = match app.sim_framing.as_str() { "binary2" => Framing::Binary2, "ascii4" => Framing::Ascii4, _ => Framing::None };
                let state = std::sync::Arc::clone(&app.sim_state);
                app.sim_server.start_with_state(port, framing, AutoResponse::default_rules(), state);
                app.set_status(format!("Server listening on :{} — waiting for connections", port), false);
            }
        }
        SimMode::Client => {
            let host    = app.sim_host.clone();
            let port: u16 = app.sim_port.parse().unwrap_or(8583);
            let msg     = app.sim_message.get_text().trim().to_string();
            let framing = app.sim_framing.clone();
            if msg.is_empty() { app.set_status("Enter a hex message first ('n' for samples)", true); return; }
            match simulator::send_message(&host, port, &msg, &framing) {
                Ok((resp_hex, summary)) => {
                    // Also try to decode the response
                    let resp_clean: String = resp_hex.chars().filter(|c| !c.is_whitespace()).collect();
                    let decode_hint = if resp_clean.len() >= 8 {
                        let first4: String = resp_clean.chars().take(4).collect();
                        if first4.chars().all(|c| c.is_ascii_digit()) {
                            let r = iso8583::decode_raw(&resp_clean);
                            format!("\n\n── Auto-decode (RAW) ──\n{}", iso8583::format_result_with_mode(&r, "RAW"))
                        } else {
                            let r = iso8583::decode(&resp_clean);
                            format!("\n\n── Auto-decode (HEX) ──\n{}", iso8583::format_result_with_mode(&r, "HEX"))
                        }
                    } else { String::new() };
                    let out = format!("▲ SENT:\n  {}\n\n▼ RESPONSE ({}):\n  {}{}", msg, summary, resp_hex, decode_hint);
                    app.sim_output.set(out);
                    app.set_status(format!("Response: {}", summary), false);
                }
                Err(e) => {
                    app.sim_output.set(format!("Connection error: {}\n\nMake sure the server is running:\n  Mode: Server | Space to start", e));
                    app.set_status(e, true);
                }
            }
        }
    }
}

// ─── Settlement tab ───────────────────────────────────────────────────────────

fn handle_settlement(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => toggle_focus(app),
        KeyCode::Char('r') => {
            app.settle_input.set_text(settlement::SAMPLE_CSV);
            app.set_status("Sample CSV loaded — Space to parse", false);
        }
        KeyCode::Char(' ') => run_settlement(app),
        code => scroll_or_edit(app, code),
    }
}

fn run_settlement(app: &mut App) {
    let input = app.settle_input.get_text();
    let (txns, errs) = settlement::parse_csv(&input);
    let report = settlement::generate_report(&txns);
    let mut out = settlement::format_report(&report, &txns);
    if !errs.is_empty() {
        out.push_str("\n── PARSE WARNINGS ──────────────────────────\n");
        for e in &errs { out.push_str(&format!("  {}\n", e)); }
    }
    app.set_status(format!("{} txns │ Net {} │ {} warning(s)", txns.len(), settlement::format_amount(report.net_total, &report.currency), errs.len()), !errs.is_empty());
    app.settle_output.set(out);
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

fn toggle_focus(app: &mut App) {
    app.focus = if app.focus == Focus::Input { Focus::Output } else { Focus::Input };
}

fn scroll_or_edit(app: &mut App, code: KeyCode) {
    if app.focus == Focus::Output { scroll_output(app, code); }
    else { let buf = app.current_input(); edit_buf(buf, code); }
}

fn scroll_output(app: &mut App, code: KeyCode) {
    let vis = 40u16;
    let out = app.current_output();
    match code {
        KeyCode::Up       => out.scroll_up(),
        KeyCode::Down     => out.scroll_down(vis),
        KeyCode::PageUp   => out.page_up(vis / 2),
        KeyCode::PageDown => out.page_down(vis / 2, vis),
        KeyCode::Char('g')=> out.scroll_to_top(),
        KeyCode::Char('G')=> out.scroll_to_bottom(vis),
        _ => {}
    }
}

pub fn edit_buf(buf: &mut app::InputBuffer, code: KeyCode) {
    match code {
        KeyCode::Backspace  => { if buf.has_selection() { buf.delete_selection(); } else { buf.backspace(); } }
        KeyCode::Delete     => { if buf.has_selection() { buf.delete_selection(); } else { buf.delete_char(); } }
        KeyCode::Char(c)    => { if buf.has_selection() { buf.delete_selection(); } buf.insert_char(c); }
        KeyCode::Enter      => { if buf.has_selection() { buf.delete_selection(); } buf.insert_newline(); }
        KeyCode::Left       => { buf.clear_selection(); buf.move_left();  }
        KeyCode::Right      => { buf.clear_selection(); buf.move_right(); }
        KeyCode::Up         => { buf.clear_selection(); buf.move_up();    }
        KeyCode::Down       => { buf.clear_selection(); buf.move_down();  }
        KeyCode::Home       => { buf.clear_selection(); buf.move_home();  }
        KeyCode::End        => { buf.clear_selection(); buf.move_end();   }
        KeyCode::PageUp     => { buf.clear_selection(); buf.page_up(10);  }
        KeyCode::PageDown   => { buf.clear_selection(); buf.page_down(10);}
        _ => {}
    }
}

pub fn edit_buf_shift(buf: &mut app::InputBuffer, code: KeyCode) {
    match code {
        KeyCode::Left  => { buf.start_selection(); buf.move_left();  }
        KeyCode::Right => { buf.start_selection(); buf.move_right(); }
        KeyCode::Up    => { buf.start_selection(); buf.move_up();    }
        KeyCode::Down  => { buf.start_selection(); buf.move_down();  }
        KeyCode::Home  => { buf.start_selection(); buf.move_home();  }
        KeyCode::End   => { buf.start_selection(); buf.move_end();   }
        _              => {}
    }
}

// ─── Sample loaders ───────────────────────────────────────────────────────────

fn load_all_samples(app: &mut App) {
    load_json(app, 0);
    load_iso(app, 0);
    load_tlv(app, 0);
    load_key_data(app, 0);
    load_sim_client_sample(app, 0);
    app.settle_input.set_text(settlement::SAMPLE_CSV);
}

fn load_json(app: &mut App, idx: usize) {
    let (label, data) = JSON_SAMPLES[idx];
    app.json_input.set_text(data);
    app.set_status(format!("JSON sample {}/{}: {}", idx+1, JSON_SAMPLES.len(), label), false);
}

fn load_iso(app: &mut App, idx: usize) {
    let (label, data) = ISO_SAMPLES[idx];
    app.iso_input.set_text(data);
    let clean: String = data.chars().filter(|c| !c.is_whitespace()).collect();
    // Auto-detect: if ALL chars are hex AND even length → HEX-encoded.
    // RAW/ASCII messages always contain non-hex chars (JSON, spaces, BCD bytes like cV).
    // A string like "0323..." that is all-hex is HEX-encoded, not RAW.
    let all_hex = clean.chars().all(|c| c.is_ascii_hexdigit());
    let even_len = clean.len() % 2 == 0;
    app.iso_mode = if all_hex && even_len { IsoMode::Hex } else { IsoMode::Raw };
    app.set_status(format!("ISO sample {}/{}: {} ({})", idx+1, ISO_SAMPLES.len(), label, match app.iso_mode { IsoMode::Raw=>"RAW", IsoMode::Hex=>"HEX" }), false);
}

fn load_tlv(app: &mut App, idx: usize) {
    let (label, data) = TLV_SAMPLES[idx];
    app.tlv_input.set_text(data);
    app.set_status(format!("TLV sample {}/{}: {}", idx+1, TLV_SAMPLES.len(), label), false);
}

fn load_key_data(app: &mut App, idx: usize) {
    let samples = key_samples_for_op(app.key_op);
    let idx = idx % samples.len().max(1);
    let (label, f0, f1, f2) = samples[idx];
    app.key_field[0].set_text(f0);
    app.key_field[1].set_text(f1);
    app.key_field[2].set_text(f2);
    app.key_focus_field = 0;
    app.sample_idx[3] = idx;
    app.set_status(format!("{} — sample {}/{}: {}", app.key_op.label(), idx+1, samples.len(), label), false);
}

fn load_sim_client_sample(app: &mut App, idx: usize) {
    let idx = idx % SIM_CLIENT_SAMPLES.len();
    let (label, data) = SIM_CLIENT_SAMPLES[idx];
    app.sim_message.set_text(data);
    app.sample_idx[4] = idx;
    app.set_status(format!("Client sample {}/{}: {}", idx+1, SIM_CLIENT_SAMPLES.len(), label), false);
}
