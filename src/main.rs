#![allow(dead_code)]
//! Entry point — event loop, key dispatch, sample data, action processors.

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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use simulator::{AutoResponse, Framing};
use std::{io, time::Duration};

// ─── Sample data ─────────────────────────────────────────────────────────────

const ISO_SAMPLES: &[(&str, &str)] = &[
    (
        "RAW | 0200 Purchase Auth (Indonesian bank format)",
        "0200FA3A4011888101000000000012000000165029962222233333cV000000000000000000000000000003191755289934711755280319031960170000000000000MTP002988953GATEUSER000310{\"email\":\"mralexnurdin@gmail.com\",\"nik\":\"3201071106960006\",\"noTelp\":\"081585452299\",\"agreement\":{\"marketing\":true,\"product\":true,\"privacyPolicy\":true,\"apps\":true,\"marketingData\":[{\"type\":\"Email\",\"status\":true},{\"type\":\"SMS\",\"status\":false},{\"type\":\"WhatsApp\",\"status\":true},{\"type\":\"Telegram\",\"status\":false}]}}022MB.CARD.CHECK.VALIDASI0356400",
    ),
    (
        "HEX | 0200 Purchase Auth Request",
        "30323030423000010AC08010000000000000313634303131313131313131313131313130303030303035303030303030313031303230393330353030303530303531303531323437",
    ),
    (
        "HEX | 0810 Network Management Response",
        "30383130A2380000040000000000000000000100000003039393939393939393030",
    ),
];

const JSON_SAMPLES: &[(&str, &str)] = &[
    (
        "E-Commerce Transaction",
        r#"{"transaction":{"id":"TXN-20260313-001","type":"purchase","amount":150000,"currency":"IDR","status":"approved","card":{"pan":"411111****1111","expiry":"12/28","scheme":"VISA"},"merchant":{"id":"MERCH001","name":"Toko Online ABC","mcc":"5411"},"timestamp":"2026-03-13T09:01:00+07:00","auth_code":"AUTH123","rrn":"RRN000001"}}"#,
    ),
    (
        "ISO 8583 Field Map",
        r#"{"fields":{"002":{"name":"PAN","type":"LLVAR","max_len":19},"003":{"name":"Processing Code","type":"FIXED","len":6},"004":{"name":"Amount Transaction","type":"FIXED","len":12},"039":{"name":"Response Code","type":"FIXED","len":2},"041":{"name":"Terminal ID","type":"FIXED","len":8},"042":{"name":"Merchant ID","type":"FIXED","len":15},"048":{"name":"Additional Data","type":"LLLVAR","max_len":999}}}"#,
    ),
    (
        "Settlement Batch",
        r#"{"batch":{"date":"2026-03-13","cutoff":"23:59:59","totals":{"debit":{"count":245,"amount":18750000},"credit":{"count":3,"amount":450000},"net":18300000},"currency":"IDR","terminals":["TERM001","TERM002","TERM003"]}}"#,
    ),
];

const TLV_SAMPLES: &[(&str, &str)] = &[
    (
        "EMV ARQC — typical F55 content",
        "9F2608A1B2C3D4E5F6A7829F2701809F101307010103A0B800F4A50000000000000000FF9F3704AABBCCDD9F360200579A032603139C015F9F02060000001500009F03060000000000009F1A0204609F4104000001125F3401019F0607A0000000031010",
    ),
    (
        "AFL — Application File Locator",
        "940C08010100100101001801010070",
    ),
    (
        "FCI with PDOL",
        "6F37840E325041592E5359532E4444463031A525BF0C229F4A01829F38139F0206A0000000041010AF0706A0000000031010BF0C039F5A0140",
    ),
    (
        "Track 2 + AIP + ATC",
        "5719476173XXXXXX4761D261220119257019891F82025C008407A0000000031010570D476173XXXXXX4761D26122019F360200A1",
    ),
];

const KEY_SAMPLES: &[(&str, &str, &str, &str, KeyOp)] = &[
    ("KCV of 2-key 3DES",   "0123456789ABCDEFFEDCBA9876543210", "",                 "",              KeyOp::Kcv),
    ("3DES Encrypt",        "0123456789ABCDEFFEDCBA9876543210", "0000000000000000", "",              KeyOp::TdesEncrypt),
    ("Build PIN Block ISO0","1234",                             "4761739001010010", "",              KeyOp::PinBuild),
    ("XOR two components",  "0123456789ABCDEF0123456789ABCDEF", "FEDCBA9876543210FEDCBA9876543210", "", KeyOp::XorHex),
    ("Luhn check — Visa",   "4111111111111111",                 "",                 "",              KeyOp::LuhnBin),
    ("Luhn check — MC",     "5500005555555559",                 "",                 "",              KeyOp::LuhnBin),
];

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let mut app = App::new();
    load_samples(&mut app); // pre-fill all tabs with sample data

    loop {
        // Pull simulator logs into the output buffer before draw
        if app.active_tab == ActiveTab::Simulator {
            refresh_sim_log(&mut app);
        }

        term.draw(|f| ui::render(f, &mut app))?;

        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut app, key.code, key.modifiers);
            }
        }

        if app.should_quit { break; }
    }

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
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

// ─── Top-level key dispatcher ────────────────────────────────────────────────

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let ctrl = mods.contains(KeyModifiers::CONTROL);

    // ── Global shortcuts ────────────────────────────────────────────────────
    if ctrl {
        match code {
            KeyCode::Char('q') => { app.should_quit = true; return; }
            KeyCode::Char('l') => {
                app.current_input().clear();
                app.current_output().set(String::new());
                app.set_status("Cleared", false);
                return;
            }
            _ => {}
        }
    }

    // ── Tab switching F1-F6 ─────────────────────────────────────────────────
    match code {
        KeyCode::F(1) => { switch_tab(app, ActiveTab::Json,       "JSON Beautify/Minify");       return; }
        KeyCode::F(2) => { switch_tab(app, ActiveTab::Iso8583,    "ISO 8583 Decoder");            return; }
        KeyCode::F(3) => { switch_tab(app, ActiveTab::Tlv,        "TLV/EMV Decoder");             return; }
        KeyCode::F(4) => { switch_tab(app, ActiveTab::KeyMgmt,    "Key Management");              return; }
        KeyCode::F(5) => { switch_tab(app, ActiveTab::Simulator,  "ISO 8583 Simulator");          return; }
        KeyCode::F(6) => { switch_tab(app, ActiveTab::Settlement, "Settlement & Reconciliation"); return; }
        _ => {}
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
    app.focus = Focus::Input;
    app.set_status(label, false);
}

// ─── JSON tab ────────────────────────────────────────────────────────────────

fn handle_json(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => toggle_focus(app),
        KeyCode::Char('s') => {
            app.json_mode = match app.json_mode {
                JsonMode::Beautify => JsonMode::Minify,
                JsonMode::Minify   => JsonMode::Beautify,
            };
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
    let result = match app.json_mode {
        JsonMode::Beautify => json_tool::beautify(&input),
        JsonMode::Minify   => json_tool::minify(&input),
    };
    let mode = match app.json_mode { JsonMode::Beautify => "Beautify", JsonMode::Minify => "Minify" };
    if let Some(e) = result.error {
        app.json_output.set(format!("Error: {}", e));
        app.set_status(format!("JSON error: {}", e), true);
    } else {
        app.json_output.set(result.output);
        app.set_status(format!("JSON {} — {} fields, depth {}", mode, result.field_count, result.depth), false);
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
    let result = match app.iso_mode {
        IsoMode::Hex => iso8583::decode(input.trim()),
        IsoMode::Raw => iso8583::decode_raw(input.trim()),
    };
    let mode = match app.iso_mode { IsoMode::Hex => "HEX", IsoMode::Raw => "RAW/ASCII" };
    let out  = iso8583::format_result_with_mode(&result, mode);
    let ok   = result.errors.is_empty();
    app.iso_output.set(out);
    app.set_status(
        format!("ISO 8583 {} — {} fields, {} error(s)", mode, result.fields.len(), result.errors.len()),
        !ok,
    );
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
    let hex: String = app.tlv_input.get_text()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    match tlv::decode(&hex) {
        Ok(nodes) => {
            let count = nodes.len();
            app.tlv_output.set(tlv::format_nodes(&nodes));
            app.set_status(format!("TLV decoded — {} top-level tag(s)", count), false);
        }
        Err(e) => {
            app.tlv_output.set(format!("TLV Error: {}\n\nInput ({}): {}", e, hex.len() / 2, hex));
            app.set_status(format!("TLV error: {}", e), true);
        }
    }
}

// ─── Key Management tab ──────────────────────────────────────────────────────

fn handle_key_mgmt(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => {
            // Cycle: field 0 → 1 → … → last active field → output → field 0
            if app.focus == Focus::Input {
                let max = app.key_op.active_field_count().saturating_sub(1);
                if app.key_focus_field < max {
                    app.key_focus_field += 1;
                } else {
                    app.focus = Focus::Output;
                }
            } else {
                app.focus = Focus::Input;
                app.key_focus_field = 0;
            }
        }
        KeyCode::Char('o') => {
            app.key_op = app.key_op.next();
            app.key_focus_field = 0;
            app.focus = Focus::Input;
            app.set_status(format!("Operation: {}", app.key_op.label()), false);
        }
        KeyCode::Char('n') => {
            let idx = (app.sample_idx[3] + 1) % KEY_SAMPLES.len();
            app.sample_idx[3] = idx;
            load_key(app, idx);
        }
        KeyCode::Char(' ') => run_key_op(app),
        KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
            if app.focus == Focus::Output =>
        {
            scroll_output(app, code);
        }
        code => {
            // Route keystrokes to the focused input field
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
        KeyOp::Kcv => keymgmt::kcv(&f0).map(|k| {
            let kb = f0.replace(' ', "").len() / 2;
            format!("Key   : {}\nKCV   : {}\nLength: {} bytes\n", f0, k, kb)
        }),
        KeyOp::TdesEncrypt => keymgmt::tdes_ecb_encrypt_hex(&f1, &f0).map(|out| {
            format!("Key        : {}\nPlaintext  : {}\nCiphertext : {}\n", f0, f1, out)
        }),
        KeyOp::TdesDecrypt => keymgmt::tdes_ecb_decrypt_hex(&f1, &f0).map(|out| {
            format!("Key        : {}\nCiphertext : {}\nPlaintext  : {}\n", f0, f1, out)
        }),
        KeyOp::PinBuild => keymgmt::build_pin_block_iso0(&f0, &f1).map(|pb| {
            format!("PIN       : {}\nPAN       : {}\nPIN Block : {} (ISO-0)\n\nXOR with ZPK to encrypt.\n", f0, f1, pb)
        }),
        KeyOp::PinDecrypt => keymgmt::decrypt_pin_block(&f1, &f0, &f2).map(|pin| {
            format!("ZPK       : {}\nEnc Block : {}\nPAN       : {}\nPIN       : {}\n", f0, f1, f2, pin)
        }),
        KeyOp::XorHex => keymgmt::xor_hex(&f0, &f1).map(|out| {
            format!("A       : {}\nB       : {}\nA XOR B : {}\n", f0, f1, out)
        }),
        KeyOp::LuhnBin => Ok(keymgmt::bin_info(&f0)),
    };

    match result {
        Ok(out) => {
            app.key_output.set(out);
            app.set_status(format!("{} OK", app.key_op.label()), false);
        }
        Err(e) => {
            app.key_output.set(format!("Error: {}", e));
            app.set_status(e, true);
        }
    }
}

// ─── Simulator tab ───────────────────────────────────────────────────────────

fn handle_simulator(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab => toggle_focus(app),
        KeyCode::Char('m') => {
            app.sim_mode = match app.sim_mode { SimMode::Server => SimMode::Client, SimMode::Client => SimMode::Server };
            app.set_status(format!("Mode: {}", match app.sim_mode { SimMode::Server => "Server", _ => "Client" }), false);
        }
        KeyCode::Char('f') => {
            app.sim_framing = match app.sim_framing.as_str() {
                "binary2" => "ascii4", "ascii4" => "none", _ => "binary2",
            }.to_string();
            app.set_status(format!("Framing: {}", app.sim_framing), false);
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
                let framing = match app.sim_framing.as_str() {
                    "binary2" => Framing::Binary2,
                    "ascii4"  => Framing::Ascii4,
                    _         => Framing::None,
                };
                let state = std::sync::Arc::clone(&app.sim_state);
                app.sim_server.start_with_state(port, framing, AutoResponse::default_rules(), state);
                app.set_status(format!("Server started on :{}", port), false);
            }
        }
        SimMode::Client => {
            let host    = app.sim_host.clone();
            let port: u16 = app.sim_port.parse().unwrap_or(8583);
            let msg     = app.sim_message.get_text().trim().to_string();
            let framing = app.sim_framing.clone();

            if msg.is_empty() {
                app.set_status("Enter a hex message first", true);
                return;
            }
            match simulator::send_message(&host, port, &msg, &framing) {
                Ok((resp_hex, summary)) => {
                    let out = format!("▲ SENT:\n  {}\n\n▼ RESPONSE ({}):\n  {}\n", msg, summary, resp_hex);
                    app.sim_output.set(out);
                    app.set_status(format!("Response: {}", summary), false);
                }
                Err(e) => {
                    app.sim_output.set(format!("Error: {}", e));
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
            app.set_status("Sample data loaded — Space to parse", false);
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
    app.set_status(
        format!("{} txns │ Net {} │ {} warning(s)",
            txns.len(),
            settlement::format_amount(report.net_total, &report.currency),
            errs.len()),
        !errs.is_empty(),
    );
    app.settle_output.set(out);
}

// ─── Shared input / scroll helpers ───────────────────────────────────────────

fn toggle_focus(app: &mut App) {
    app.focus = if app.focus == Focus::Input { Focus::Output } else { Focus::Input };
}

/// For panes with a single input + single output: route arrow/pgup/pgdn to
/// output scroll when focus is Output, otherwise send to the active input buffer.
fn scroll_or_edit(app: &mut App, code: KeyCode) {
    if app.focus == Focus::Output {
        scroll_output(app, code);
    } else {
        let buf = app.current_input();
        edit_buf(buf, code);
    }
}

fn scroll_output(app: &mut App, code: KeyCode) {
    let vis = 40u16; // approximate; actual height not critical for scrolling
    let out = app.current_output();
    match code {
        KeyCode::Up       => out.scroll_up(),
        KeyCode::Down     => out.scroll_down(vis),
        KeyCode::PageUp   => out.page_up(vis / 2),
        KeyCode::PageDown => out.page_down(vis / 2, vis),
        KeyCode::Char('g') => out.scroll_to_top(),
        KeyCode::Char('G') => out.scroll_to_bottom(vis),
        _ => {}
    }
}

/// Apply an editing keystroke to any `InputBuffer`.
pub fn edit_buf(buf: &mut app::InputBuffer, code: KeyCode) {
    match code {
        KeyCode::Char(c)   => buf.insert_char(c),
        KeyCode::Enter      => buf.insert_newline(),
        KeyCode::Backspace  => buf.backspace(),
        KeyCode::Delete     => buf.delete_char(),
        KeyCode::Left       => buf.move_left(),
        KeyCode::Right      => buf.move_right(),
        KeyCode::Up         => buf.move_up(),
        KeyCode::Down       => buf.move_down(),
        KeyCode::Home       => buf.move_home(),
        KeyCode::End        => buf.move_end(),
        KeyCode::PageUp     => buf.page_up(10),
        KeyCode::PageDown   => buf.page_down(10),
        _                   => {}
    }
}

// ─── Sample loaders ───────────────────────────────────────────────────────────

fn load_samples(app: &mut App) {
    load_json(app, 0);
    load_iso(app, 0);
    load_tlv(app, 0);
    load_key(app, 0);
    app.settle_input.set_text(settlement::SAMPLE_CSV);
}

fn load_json(app: &mut App, idx: usize) {
    let (label, data) = JSON_SAMPLES[idx];
    app.json_input.set_text(data);
    app.set_status(format!("JSON sample: {}", label), false);
}

fn load_iso(app: &mut App, idx: usize) {
    let (label, data) = ISO_SAMPLES[idx];
    app.iso_input.set_text(data);
    // Auto-detect mode: if first 4 chars are ASCII digits it's RAW, otherwise HEX
    let clean: String = data.chars().filter(|c| !c.is_whitespace()).collect();
    app.iso_mode = if clean.len() >= 4 && clean[..4].chars().all(|c| c.is_ascii_digit()) {
        IsoMode::Raw
    } else {
        IsoMode::Hex
    };
    app.set_status(format!("ISO sample: {} ({})", label,
        match app.iso_mode { IsoMode::Raw => "RAW/ASCII", IsoMode::Hex => "HEX" }), false);
}

fn load_tlv(app: &mut App, idx: usize) {
    let (label, data) = TLV_SAMPLES[idx];
    app.tlv_input.set_text(data);
    app.set_status(format!("TLV sample: {}", label), false);
}

fn load_key(app: &mut App, idx: usize) {
    let (label, f0, f1, f2, op) = KEY_SAMPLES[idx];
    app.key_op = op;
    app.key_field[0].set_text(f0);
    app.key_field[1].set_text(f1);
    app.key_field[2].set_text(f2);
    app.key_focus_field = 0;
    app.set_status(format!("Key sample: {}", label), false);
}
