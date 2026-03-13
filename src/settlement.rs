// ─── Settlement & Reconciliation ──────────────────────────────────────────────
// Parse CSV transaction log, calculate settlement totals, find discrepancies

#[derive(Clone, Debug)]
pub struct Transaction {
    pub id:          String,
    pub mti:         String,
    pub stan:        String,
    pub rrn:         String,
    pub datetime:    String,
    pub amount:      i64,    // in smallest currency unit (e.g. cents/sen)
    pub currency:    String,
    pub response:    String,
    pub merchant_id: String,
    pub terminal_id: String,
    pub card_masked: String,
    pub txn_type:    TxnType,
    pub status:      TxnStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TxnType {
    Purchase,
    Refund,
    Reversal,
    Auth,
    Other(String),
}
impl TxnType {
    pub fn label(&self) -> &str {
        match self {
            TxnType::Purchase    => "PURCHASE ",
            TxnType::Refund      => "REFUND   ",
            TxnType::Reversal    => "REVERSAL ",
            TxnType::Auth        => "AUTH     ",
            TxnType::Other(s)    => s.as_str(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TxnStatus {
    Approved,
    Declined,
    Reversed,
    Timeout,
    Unknown,
}
impl TxnStatus {
    pub fn from_rc(rc: &str) -> Self {
        match rc {
            "00" => TxnStatus::Approved,
            "68" => TxnStatus::Timeout,
            s if !s.is_empty() => TxnStatus::Declined,
            _    => TxnStatus::Unknown,
        }
    }
    pub fn label(&self) -> &str {
        match self {
            TxnStatus::Approved  => "APPROVED",
            TxnStatus::Declined  => "DECLINED",
            TxnStatus::Reversed  => "REVERSED",
            TxnStatus::Timeout   => "TIMEOUT ",
            TxnStatus::Unknown   => "UNKNOWN ",
        }
    }
}

/// Parse CSV settlement file
/// Expected columns (flexible header detection):
///   id/txn_id, mti, stan, rrn, datetime/date, amount, currency, response/rc, merchant_id, terminal_id, pan/card, type
pub fn parse_csv(input: &str) -> (Vec<Transaction>, Vec<String>) {
    let mut txns = Vec::new();
    let mut errors = Vec::new();
    let mut lines = input.lines().peekable();

    // Detect header
    let header_line = match lines.next() {
        Some(h) => h.to_lowercase(),
        None => { errors.push("Input kosong".to_string()); return (txns, errors); }
    };

    let cols: Vec<&str> = header_line.split(',').map(|s| s.trim()).collect();

    let find_col = |names: &[&str]| -> Option<usize> {
        names.iter().find_map(|&n| cols.iter().position(|c| c.contains(n)))
    };

    let col_id       = find_col(&["id", "txnid", "txn_id", "seq"]);
    let col_mti      = find_col(&["mti"]);
    let col_stan     = find_col(&["stan", "trace"]);
    let col_rrn      = find_col(&["rrn", "retrieval"]);
    let col_dt       = find_col(&["datetime", "date", "time", "timestamp"]);
    let col_amount   = find_col(&["amount", "amt", "total"]);
    let col_currency = find_col(&["currency", "curr", "ccy"]);
    let col_rc       = find_col(&["response", "rc", "code", "resp"]);
    let col_mid      = find_col(&["merchant_id", "merchant", "mid"]);
    let col_tid      = find_col(&["terminal_id", "terminal", "tid"]);
    let col_pan      = find_col(&["pan", "card", "masked"]);
    let col_type     = find_col(&["type", "txn_type", "kind"]);

    for (row_num, line) in lines.enumerate() {
        if line.trim().is_empty() || line.trim().starts_with('#') { continue; }

        let fields: Vec<&str> = line.splitn(20, ',').collect();
        let get = |idx: Option<usize>| -> &str {
            idx.and_then(|i| fields.get(i)).map(|s| s.trim()).unwrap_or("")
        };

        let amount_str = get(col_amount).replace([' ', '_', '.'], "");
        let amount = amount_str.parse::<i64>().unwrap_or_else(|_| {
            // Try decimal: "1500.50" → 150050
            if let Ok(f) = amount_str.parse::<f64>() {
                (f * 100.0).round() as i64
            } else {
                errors.push(format!("Row {}: cannot parse amount '{}'", row_num+2, get(col_amount)));
                0
            }
        });

        let rc   = get(col_rc);
        let mti  = get(col_mti);
        let ttype_str = get(col_type);

        let txn_type = match (mti, ttype_str) {
            (_, t) if t.to_lowercase().contains("refund") || t.to_lowercase().contains("credit") => TxnType::Refund,
            (_, t) if t.to_lowercase().contains("rev")   => TxnType::Reversal,
            (_, t) if t.to_lowercase().contains("auth")  => TxnType::Auth,
            ("0200", _) | ("0210", _) => TxnType::Purchase,
            ("0400", _) | ("0410", _) => TxnType::Reversal,
            ("0100", _) | ("0110", _) => TxnType::Auth,
            _ => TxnType::Other(if ttype_str.is_empty() { mti.to_string() } else { ttype_str.to_string() }),
        };

        txns.push(Transaction {
            id:          get(col_id).to_string(),
            mti:         mti.to_string(),
            stan:        get(col_stan).to_string(),
            rrn:         get(col_rrn).to_string(),
            datetime:    get(col_dt).to_string(),
            amount,
            currency:    get(col_currency).to_string(),
            response:    rc.to_string(),
            merchant_id: get(col_mid).to_string(),
            terminal_id: get(col_tid).to_string(),
            card_masked: get(col_pan).to_string(),
            txn_type,
            status:      TxnStatus::from_rc(rc),
        });
    }

    (txns, errors)
}

/// Settlement summary report
pub struct SettlementReport {
    pub total_count:    usize,
    pub approved_count: usize,
    pub declined_count: usize,
    pub reversed_count: usize,

    pub debit_count:    usize,
    pub debit_total:    i64,
    pub credit_count:   usize,
    pub credit_total:   i64,
    pub net_total:      i64,

    pub currency:       String,
    pub by_terminal:    Vec<TerminalSummary>,
    pub by_merchant:    Vec<MerchantSummary>,
    pub unmatched:      Vec<String>,   // RRN of transactions with no matching reversal pair
}

#[derive(Clone)]
pub struct TerminalSummary {
    pub tid:            String,
    pub count:          usize,
    pub approved:       usize,
    pub debit_total:    i64,
    pub credit_total:   i64,
}

#[derive(Clone)]
pub struct MerchantSummary {
    pub mid:            String,
    pub count:          usize,
    pub net_total:      i64,
}

pub fn generate_report(txns: &[Transaction]) -> SettlementReport {
    use std::collections::HashMap;

    let mut report = SettlementReport {
        total_count:    txns.len(),
        approved_count: 0,
        declined_count: 0,
        reversed_count: 0,
        debit_count:    0,
        debit_total:    0,
        credit_count:   0,
        credit_total:   0,
        net_total:      0,
        currency:       String::new(),
        by_terminal:    Vec::new(),
        by_merchant:    Vec::new(),
        unmatched:      Vec::new(),
    };

    let mut tid_map: HashMap<String, TerminalSummary> = HashMap::new();
    let mut mid_map: HashMap<String, MerchantSummary> = HashMap::new();
    let mut rrn_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut rev_rrn: std::collections::HashSet<String> = std::collections::HashSet::new();

    for t in txns {
        if !t.currency.is_empty() { report.currency = t.currency.clone(); }

        match t.status {
            TxnStatus::Approved => report.approved_count += 1,
            TxnStatus::Declined => { report.declined_count += 1; continue; }
            _                   => {}
        }

        let is_credit = matches!(t.txn_type, TxnType::Refund | TxnType::Reversal);

        if is_credit {
            report.credit_count += 1;
            report.credit_total += t.amount;
            report.reversed_count += 1;
            if !t.rrn.is_empty() { rev_rrn.insert(t.rrn.clone()); }
        } else {
            report.debit_count += 1;
            report.debit_total += t.amount;
            if !t.rrn.is_empty() { rrn_set.insert(t.rrn.clone()); }
        }

        // By terminal
        let tid = if t.terminal_id.is_empty() { "UNKNOWN".to_string() } else { t.terminal_id.clone() };
        let te = tid_map.entry(tid.clone()).or_insert(TerminalSummary {
            tid: tid, count: 0, approved: 0, debit_total: 0, credit_total: 0,
        });
        te.count += 1;
        te.approved += 1;
        if is_credit { te.credit_total += t.amount; } else { te.debit_total += t.amount; }

        // By merchant
        let mid = if t.merchant_id.is_empty() { "UNKNOWN".to_string() } else { t.merchant_id.clone() };
        let me = mid_map.entry(mid.clone()).or_insert(MerchantSummary { mid, count: 0, net_total: 0 });
        me.count += 1;
        if is_credit { me.net_total -= t.amount; } else { me.net_total += t.amount; }
    }

    report.net_total = report.debit_total - report.credit_total;

    // Unmatched: purchases that have no corresponding reversal by RRN
    report.unmatched = rrn_set.difference(&rev_rrn)
        .take(50)
        .cloned()
        .collect();

    let mut terminals: Vec<_> = tid_map.into_values().collect();
    terminals.sort_by(|a, b| b.count.cmp(&a.count));
    report.by_terminal = terminals;

    let mut merchants: Vec<_> = mid_map.into_values().collect();
    merchants.sort_by(|a, b| b.net_total.cmp(&a.net_total));
    report.by_merchant = merchants;

    report
}

pub fn format_amount(amount: i64, currency: &str) -> String {
    let whole  = amount / 100;
    let cents  = (amount % 100).abs();
    let sym = match currency {
        "360" | "IDR" => "Rp",
        "840" | "USD" => "$",
        "978" | "EUR" => "€",
        "702" | "SGD" => "S$",
        _             => currency,
    };
    if currency == "360" || currency == "IDR" {
        // IDR has no cents
        format!("{} {:>14}", sym, format_thousands(whole))
    } else {
        format!("{} {:>12}.{:02}", sym, format_thousands(whole), cents)
    }
}

fn format_thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { result.push('.'); }
        result.push(c);
    }
    if n < 0 { result.push('-'); }
    result.chars().rev().collect()
}

pub fn format_report(r: &SettlementReport, txns: &[Transaction]) -> String {
    let mut out = String::new();

    out.push_str("╔════════════════════════════════════════════════════════════╗\n");
    out.push_str("║          SETTLEMENT & RECONCILIATION REPORT                ║\n");
    out.push_str("╚════════════════════════════════════════════════════════════╝\n\n");

    out.push_str("── SUMMARY ─────────────────────────────────────────────────────\n");
    out.push_str(&format!("  Total Transactions : {:>6}\n",  r.total_count));
    out.push_str(&format!("  Approved           : {:>6}  ({:.1}%)\n", r.approved_count,
        if r.total_count > 0 { 100.0 * r.approved_count as f64 / r.total_count as f64 } else { 0.0 }));
    out.push_str(&format!("  Declined           : {:>6}\n",  r.declined_count));
    out.push_str(&format!("  Reversed/Refunded  : {:>6}\n\n", r.reversed_count));

    out.push_str("── FINANCIALS ───────────────────────────────────────────────────\n");
    out.push_str(&format!("  Debit  ({:>4} txn)  : {}\n", r.debit_count, format_amount(r.debit_total, &r.currency)));
    out.push_str(&format!("  Credit ({:>4} txn)  : {}\n", r.credit_count, format_amount(r.credit_total, &r.currency)));
    out.push_str(&format!("  ─────────────────────\n"));
    out.push_str(&format!("  NET SETTLEMENT      : {}\n\n", format_amount(r.net_total, &r.currency)));

    if !r.by_terminal.is_empty() {
        out.push_str("── BY TERMINAL ──────────────────────────────────────────────────\n");
        out.push_str(&format!("  {:<16} {:>5} {:>5} {}\n", "Terminal ID", "Count", "Appr", "Net Debit"));
        for t in r.by_terminal.iter().take(15) {
            out.push_str(&format!("  {:<16} {:>5} {:>5}  {}\n",
                t.tid, t.count, t.approved,
                format_amount(t.debit_total - t.credit_total, &r.currency)));
        }
        out.push_str("\n");
    }

    if !r.by_merchant.is_empty() {
        out.push_str("── BY MERCHANT ──────────────────────────────────────────────────\n");
        out.push_str(&format!("  {:<16} {:>5}  {}\n", "Merchant ID", "Count", "Net"));
        for m in r.by_merchant.iter().take(15) {
            out.push_str(&format!("  {:<16} {:>5}  {}\n",
                m.mid, m.count, format_amount(m.net_total, &r.currency)));
        }
        out.push_str("\n");
    }

    if !r.unmatched.is_empty() {
        out.push_str("── UNMATCHED PURCHASES (no reversal found) ───────────────────\n");
        for rrn in &r.unmatched {
            out.push_str(&format!("  RRN: {}\n", rrn));
        }
        out.push_str("\n");
    }

    // Raw transaction list (last 50)
    out.push_str("── TRANSACTION LIST (latest 50) ─────────────────────────────\n");
    out.push_str(&format!("  {:<6} {:<9} {:<8} {:<12} {:<16} {}\n",
        "Stan", "Type", "Status", "Amount", "Terminal", "RRN"));
    out.push_str(&format!("  {}\n", "─".repeat(60)));

    for t in txns.iter().take(50) {
        out.push_str(&format!("  {:<6} {:<9} {:<8}  {:>12}  {:<14}  {}\n",
            t.stan,
            t.txn_type.label(),
            t.status.label(),
            format_amount(t.amount, &r.currency),
            t.terminal_id,
            t.rrn,
        ));
    }

    out
}

/// Sample CSV data for demonstration
pub const SAMPLE_CSV: &str = "\
id,mti,stan,rrn,datetime,amount,currency,response,merchant_id,terminal_id,pan,type
TXN001,0200,000001,RRN000001,2026-03-12 09:01:00,150000,IDR,00,MERCH001,TERM001,411111****1111,purchase
TXN002,0200,000002,RRN000002,2026-03-12 09:03:00,75000,IDR,00,MERCH001,TERM001,512345****6789,purchase
TXN003,0200,000003,RRN000003,2026-03-12 09:07:00,250000,IDR,51,MERCH002,TERM002,411111****1111,purchase
TXN004,0200,000004,RRN000004,2026-03-12 09:12:00,500000,IDR,00,MERCH002,TERM002,556617****4321,purchase
TXN005,0200,000005,RRN000005,2026-03-12 09:15:00,320000,IDR,00,MERCH001,TERM003,411111****9999,purchase
TXN006,0200,000006,RRN000006,2026-03-12 09:20:00,180000,IDR,05,MERCH003,TERM001,512345****1234,purchase
TXN007,0400,000007,RRN000004,2026-03-12 09:25:00,500000,IDR,00,MERCH002,TERM002,556617****4321,reversal
TXN008,0200,000008,RRN000008,2026-03-12 09:30:00,95000,IDR,00,MERCH003,TERM004,411111****5678,purchase
TXN009,0200,000009,RRN000009,2026-03-12 09:35:00,1200000,IDR,00,MERCH001,TERM001,411111****1111,purchase
TXN010,0200,000010,RRN000010,2026-03-12 09:40:00,450000,IDR,54,MERCH002,TERM003,512345****9876,purchase\
";
