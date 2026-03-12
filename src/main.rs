mod app;
mod ui;
mod json_tool;
mod iso8583;

use std::io;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture,
        Event, KeyCode, KeyModifiers,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::backend::CrosstermBackend;

use app::{ActiveTab, App, Focus, IsoMode, JsonMode};

const ISO_SAMPLES: &[(&str, &str)] = &[
    (
        "30323030723C448008E080003136343131313131313131313131313131313030303030303030303030303135303030303033313231343330323530303030303131343330323530333132323531323534313130353130303030303030303030303030315445524D303030314D45524348414E5430303030303120574152554E47204D414B414E20534544455248414E4120204A414B41525441202020202049442020333630",
        "0200 — Purchase Auth Request (IDR 1500.00, Chip)"
    ),
    (
        "30323130723C04800EC08000313634313131313131313131313131313131303030303030303030303030313530303030303331323134333032363030303030313134333032363033313232353132303531303030303030303030303030303141555448303130305445524D303030314D45524348414E5430303030303120333630",
        "0210 — Auth Response Approved (RC=00, Auth=AUTH01)"
    ),
    (
        "30323130723C04800EC08000313635353030303035353535353535353539303030303030303030303032353030303030303331323135303030303030303030323135303030303033313232333031303231303030303030303030303030303220202020202035345445524D303030324F4E4C494E452D53484F502D303031333630",
        "0210 — Auth Response Declined (RC=54: Expired Card)"
    ),
    (
        "30343030F23C04800EC080000000004000000000313634313131313131313131313131313131303030303030303030303030313530303030303331323134333130303030303030333134333130303033313232353132303531303030303030303030303030303341555448303130305445524D303030314D45524348414E5430303030303120333630303230303030303030303030303130333132313433303235303030303030303030303030303030303030",
        "0400 — Reversal Request (original STAN 000001)"
    ),
    (
        "303830308220000000000000040000000000000030333132303030303030393939393939333031",
        "0800 — Network Management Request (Sign-On, F70=301)"
    ),
];

const JSON_SAMPLES: &[(&str, &str)] = &[
    (
        r#"{"transaction":{"id":"TXN-20260312-001","type":"purchase","status":"approved","amount":{"value":150000,"currency":"IDR","formatted":"Rp 1.500,00"},"timestamp":"2026-03-12T14:30:25+07:00","merchant":{"id":"MERCHANT000001","name":"Warung Makan Sederhana","category":{"code":"5411","description":"Grocery Stores & Supermarkets"},"location":{"address":"Jl. Sudirman No. 1","city":"Jakarta","country":"ID","postal_code":"10220"}},"card":{"pan_masked":"411111******1111","expiry":"12/25","entry_mode":"chip","scheme":"VISA"},"terminal":{"id":"TERM0001","type":"EDC","acquirer_id":"BNI001"},"auth":{"stan":"000001","rrn":"000000000001","approval_code":"AUTH01","response_code":"00","response_desc":"Approved"},"fees":{"mdr":0.007,"mdr_amount":1050,"net_amount":148950}}}"#,
        "E-Commerce Transaction Object"
    ),
    (
        r#"{"iso8583":{"version":"1987","encoding":"ASCII","fields":{"2":{"name":"PAN","type":"LLVAR","data_type":"N","max_length":19,"sensitive":true},"3":{"name":"Processing Code","type":"FIXED","data_type":"N","length":6},"4":{"name":"Amount Transaction","type":"FIXED","data_type":"N","length":12},"7":{"name":"Transmission Date & Time","type":"FIXED","data_type":"N","length":10},"11":{"name":"STAN","type":"FIXED","data_type":"N","length":6},"12":{"name":"Time Local Transaction","type":"FIXED","data_type":"N","length":6},"13":{"name":"Date Local Transaction","type":"FIXED","data_type":"N","length":4},"22":{"name":"POS Entry Mode","type":"FIXED","data_type":"N","length":3},"37":{"name":"RRN","type":"FIXED","data_type":"AN","length":12},"38":{"name":"Auth ID Response","type":"FIXED","data_type":"AN","length":6},"39":{"name":"Response Code","type":"FIXED","data_type":"AN","length":2},"41":{"name":"Terminal ID","type":"FIXED","data_type":"ANS","length":8},"42":{"name":"Merchant ID","type":"FIXED","data_type":"ANS","length":15},"49":{"name":"Currency Code","type":"FIXED","data_type":"AN","length":3}},"response_codes":{"00":"Approved","01":"Refer to Card Issuer","05":"Do Not Honour","12":"Invalid Transaction","14":"Invalid Card Number","51":"Insufficient Funds","54":"Expired Card","55":"Incorrect PIN","91":"Issuer Inoperative","96":"System Malfunction"},"mti":{"0200":"Financial Transaction Request","0210":"Financial Transaction Response","0400":"Reversal Request","0410":"Reversal Response","0800":"Network Management Request","0810":"Network Management Response"}}}"#,
        "ISO 8583 Field Config Map"
    ),
    (
        r#"{"api":{"version":"v2","base_url":"https://api.bank.co.id/payment","auth":{"type":"OAuth2","token_endpoint":"/oauth/token","scopes":["payment:read","payment:write","settlement:read"]},"endpoints":[{"method":"POST","path":"/transactions/authorize","description":"Authorize a card transaction","request":{"content_type":"application/json","body":{"required":["amount","currency","card","merchant"],"amount":{"type":"number","min":100,"max":100000000},"currency":{"type":"string","enum":["IDR","USD","SGD"]}}},"response":{"200":{"status":"approved","transaction_id":"string","approval_code":"string"},"402":{"status":"declined","reason_code":"string","reason_desc":"string"}}},{"method":"POST","path":"/transactions/void","description":"Void/Reverse a transaction","request":{"body":{"required":["original_transaction_id","reason"]}},"response":{"200":{"status":"voided"},"404":{"error":"Transaction not found"}}}],"rate_limits":{"requests_per_minute":600,"requests_per_day":100000},"timeout_ms":30000,"retry":{"max_attempts":3,"backoff_ms":500}}}"#,
        "Bank Payment API Spec"
    ),
    (
        r#"{"settlement":{"batch_id":"BATCH-20260312","date":"2026-03-12","acquirer":{"institution_code":"014","name":"BCA","bin_range":["400000-499999","510000-559999"]},"summary":{"total_transactions":1547,"approved":1489,"declined":58,"reversed":12,"total_amount":{"debit":{"count":1245,"amount":187650000},"credit":{"count":244,"amount":32100000},"net":{"amount":155550000,"currency":"IDR"}},"by_card_scheme":{"VISA":{"count":892,"amount":98750000},"MASTERCARD":{"count":597,"amount":88900000}},"by_mcc":{"5411":{"description":"Grocery","count":456,"amount":34200000},"5812":{"description":"Restaurant","count":312,"amount":18900000},"4816":{"description":"Digital Goods","count":231,"amount":45600000}}},"status":"pending_upload","generated_at":"2026-03-12T23:59:59+07:00"}}"#,
        "Daily Settlement Batch"
    ),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    let mut app = App::new();

    app.json_input.set_text(JSON_SAMPLES[0].0);
    app.iso_input.set_text(ISO_SAMPLES[0].0);

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    if let Err(e) = res { eprintln!("Error: {}", e); }
    Ok(())
}

fn run_app(
    terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_idx: usize = 0;
    let mut iso_idx:  usize = 0;

    app.set_status("F1:JSON  F2:ISO8583  F3◀ F4▶:Sample  F5:Process  F6:Mode  Tab:Pane  Ctrl+Q:Quit", false);

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            use KeyCode::*;
            use KeyModifiers as KM;
            let ctrl  = key.modifiers.contains(KM::CONTROL);
            let shift = key.modifiers.contains(KM::SHIFT);

            if ctrl && key.code == Char('q') { break; }

            match key.code {
                F(1) => {
                    app.active_tab = ActiveTab::Json;
                    app.focus = Focus::Input;
                    app.set_status(format!("JSON tab | Sample {}/{}: {}", json_idx+1, JSON_SAMPLES.len(), JSON_SAMPLES[json_idx].1), false);
                    continue;
                }
                F(2) => {
                    app.active_tab = ActiveTab::Iso8583;
                    app.focus = Focus::Input;
                    app.set_status(format!("ISO 8583 tab | Sample {}/{}: {}", iso_idx+1, ISO_SAMPLES.len(), ISO_SAMPLES[iso_idx].1), false);
                    continue;
                }
                F(3) => {
                    match app.active_tab {
                        ActiveTab::Json => {
                            json_idx = (json_idx + JSON_SAMPLES.len() - 1) % JSON_SAMPLES.len();
                            app.json_input.set_text(JSON_SAMPLES[json_idx].0);
                            app.json_output.set(String::new());
                            app.set_status(format!("◀ JSON Sample {}/{}: {} | F5 to process", json_idx+1, JSON_SAMPLES.len(), JSON_SAMPLES[json_idx].1), false);
                        }
                        ActiveTab::Iso8583 => {
                            iso_idx = (iso_idx + ISO_SAMPLES.len() - 1) % ISO_SAMPLES.len();
                            app.iso_input.set_text(ISO_SAMPLES[iso_idx].0);
                            app.iso_output.set(String::new());
                            app.set_status(format!("◀ ISO Sample {}/{}: {} | F5 to decode", iso_idx+1, ISO_SAMPLES.len(), ISO_SAMPLES[iso_idx].1), false);
                        }
                    }
                    continue;
                }
                F(4) => {
                    match app.active_tab {
                        ActiveTab::Json => {
                            json_idx = (json_idx + 1) % JSON_SAMPLES.len();
                            app.json_input.set_text(JSON_SAMPLES[json_idx].0);
                            app.json_output.set(String::new());
                            app.set_status(format!("▶ JSON Sample {}/{}: {} | F5 to process", json_idx+1, JSON_SAMPLES.len(), JSON_SAMPLES[json_idx].1), false);
                        }
                        ActiveTab::Iso8583 => {
                            iso_idx = (iso_idx + 1) % ISO_SAMPLES.len();
                            app.iso_input.set_text(ISO_SAMPLES[iso_idx].0);
                            app.iso_output.set(String::new());
                            app.set_status(format!("▶ ISO Sample {}/{}: {} | F5 to decode", iso_idx+1, ISO_SAMPLES.len(), ISO_SAMPLES[iso_idx].1), false);
                        }
                    }
                    continue;
                }
                _ => {}
            }

            if key.code == Tab && !ctrl && !shift {
                app.focus = match app.focus { Focus::Input => Focus::Output, Focus::Output => Focus::Input };
                continue;
            }

            if app.focus == Focus::Input {
                match key.code {
                    F(5) => { process_action(app); }
                    F(6) if app.active_tab == ActiveTab::Json => {
                        app.json_mode = match app.json_mode { JsonMode::Beautify => JsonMode::Minify, JsonMode::Minify => JsonMode::Beautify };
                        app.set_status(format!("Mode: {} | F5 to process", match app.json_mode { JsonMode::Beautify => "Beautify", JsonMode::Minify => "Minify" }), false);
                    }
                    F(6) if app.active_tab == ActiveTab::Iso8583 => {
                        app.iso_mode = match app.iso_mode { IsoMode::Hex => IsoMode::Raw, IsoMode::Raw => IsoMode::Hex };
                        let mode_name = match app.iso_mode {
                            IsoMode::Hex => "HEX (fully encoded, e.g. MTI='30323030')",
                            IsoMode::Raw => "RAW/ASCII (MTI='0200', bitmap=hex, data=ASCII)",
                        };
                        app.set_status(format!("ISO Mode: {} | F5 to decode", mode_name), false);
                    }
                    Char('l') if ctrl => {
                        match app.active_tab {
                            ActiveTab::Json    => { app.json_input.clear(); app.json_output.set(String::new()); }
                            ActiveTab::Iso8583 => { app.iso_input.clear(); app.iso_output.set(String::new()); }
                        }
                        app.set_status("Cleared | F3/F4 untuk load sample", false);
                    }
                    Up       => { app.current_input().move_up(); }
                    Down     => { app.current_input().move_down(); }
                    Left     => { app.current_input().move_left(); }
                    Right    => { app.current_input().move_right(); }
                    Home     => { app.current_input().move_home(); }
                    End      => { app.current_input().move_end(); }
                    PageUp   => { app.current_input().page_up(10); }
                    PageDown => { app.current_input().page_down(10); }
                    Enter    => { app.current_input().insert_newline(); }
                    Backspace=> { app.current_input().backspace(); }
                    Delete   => { app.current_input().delete_char(); }
                    Char(c) if !ctrl => { app.current_input().insert_char(c); }
                    _ => {}
                }
            }

            if app.focus == Focus::Output {
                let out_lines = match app.active_tab {
                    ActiveTab::Json    => app.json_output.content.lines().count(),
                    ActiveTab::Iso8583 => app.iso_output.content.lines().count(),
                };
                match key.code {
                    Up       => { app.current_output().scroll_up(); }
                    Down     => { app.current_output().scroll_down(out_lines, 40); }
                    PageUp   => { app.current_output().page_up(10); }
                    PageDown => { app.current_output().page_down(10, out_lines, 40); }
                    Char('g')=> { app.current_output().scroll_to_top(); }
                    Char('G')=> { app.current_output().scroll_to_bottom(out_lines, 40); }
                    Char('k')=> { app.current_output().scroll_up(); }
                    Char('j')=> { app.current_output().scroll_down(out_lines, 40); }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn process_action(app: &mut App) {
    match app.active_tab {
        ActiveTab::Json => {
            let input = app.json_input.get_text();
            if input.trim().is_empty() {
                app.set_status("Input kosong — tekan F3/F4 untuk load sample", true);
                return;
            }
            let result = match app.json_mode {
                JsonMode::Beautify => json_tool::beautify(&input),
                JsonMode::Minify   => json_tool::minify(&input),
            };
            match result.error {
                Some(e) => {
                    app.json_output.set(format!("❌ Parse Error\n\n{}", e));
                    app.set_status(format!("Error: {}", e), true);
                }
                None => {
                    app.json_output.set(result.output);
                    app.set_status(
                        format!("✓ {} | {} fields | depth {} | Tab→output", match app.json_mode { JsonMode::Beautify => "Beautified", JsonMode::Minify => "Minified" }, result.field_count, result.depth),
                        false,
                    );
                    app.focus = Focus::Output;
                }
            }
        }
        ActiveTab::Iso8583 => {
            let input = app.iso_input.get_text();
            if input.trim().is_empty() {
                app.set_status("Input kosong — tekan F3/F4 untuk load sample", true);
                return;
            }
            // Auto-detect format if user hasn't manually switched
            let detected = iso8583::detect_format(&input);
            let (result, mode_label) = match app.iso_mode {
                IsoMode::Hex => (iso8583::decode(&input), "HEX"),
                IsoMode::Raw => (iso8583::decode_raw(&input), "RAW/ASCII"),
            };
            let fc = result.fields.len();
            let ec = result.errors.len();
            let output = iso8583::format_result_with_mode(&result, mode_label);
            app.iso_output.set(output);
            let hint = if detected != match app.iso_mode { IsoMode::Hex => "hex", IsoMode::Raw => "raw" } {
                format!(" [coba F6 untuk ganti mode ke {}]", detected.to_uppercase())
            } else { String::new() };
            if ec > 0 {
                app.set_status(format!("⚠ {} fields, {} error(s){} — lihat output", fc, ec, hint), true);
            } else {
                app.set_status(format!("✓ MTI: {} | {} fields | mode={}{} | Tab→output", result.mti, fc, mode_label, hint), false);
            }
            app.focus = Focus::Output;
        }
    }
}
