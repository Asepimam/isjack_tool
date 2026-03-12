//! Sample generator — builds valid ISO 8583 hex messages + JSON payloads
//! Run: cargo run --example gen_samples

// ─── ISO 8583 builder ────────────────────────────────────────────────────────

struct Iso8583Builder {
    mti: String,
    // field_number → (value_bytes)
    fields: std::collections::BTreeMap<usize, Vec<u8>>,
}

impl Iso8583Builder {
    fn new(mti: &str) -> Self {
        Self { mti: mti.to_string(), fields: std::collections::BTreeMap::new() }
    }

    /// Add a FIXED-length ASCII field
    fn fixed(&mut self, n: usize, value: &str, len: usize) -> &mut Self {
        assert_eq!(value.len(), len, "F{}: value '{}' must be exactly {} chars", n, value, len);
        self.fields.insert(n, value.as_bytes().to_vec());
        self
    }

    /// Add a LLVAR ASCII field (2-digit ASCII length prefix)
    fn llvar(&mut self, n: usize, value: &str) -> &mut Self {
        let mut data = format!("{:02}", value.len()).into_bytes();
        data.extend_from_slice(value.as_bytes());
        self.fields.insert(n, data);
        self
    }

    /// Add a LLLVAR ASCII field (3-digit ASCII length prefix)
    fn lllvar(&mut self, n: usize, value: &str) -> &mut Self {
        let mut data = format!("{:03}", value.len()).into_bytes();
        data.extend_from_slice(value.as_bytes());
        self.fields.insert(n, data);
        self
    }

    /// Build final hex string (all ASCII encoding)
    fn build(&self) -> String {
        // ── Build bitmaps ──
        let mut primary   = [0u8; 8];
        let mut secondary = [0u8; 8];
        let has_secondary = self.fields.keys().any(|&n| n > 64);

        if has_secondary {
            set_bit(&mut primary, 1); // bit 1 = secondary bitmap present
        }
        for &n in self.fields.keys() {
            if n <= 64 {
                set_bit(&mut primary, n);
            } else {
                set_bit(&mut secondary, n - 64);
            }
        }

        let mut out = String::new();

        // MTI (4 ASCII chars → 8 hex)
        for b in self.mti.as_bytes() {
            out.push_str(&format!("{:02X}", b));
        }

        // Primary bitmap (8 raw bytes → 16 hex)
        for b in &primary {
            out.push_str(&format!("{:02X}", b));
        }

        // Secondary bitmap if needed
        if has_secondary {
            for b in &secondary {
                out.push_str(&format!("{:02X}", b));
            }
        }

        // Fields in numeric order
        for (&_n, data) in &self.fields {
            for b in data {
                out.push_str(&format!("{:02X}", b));
            }
        }

        out
    }
}

fn set_bit(bitmap: &mut [u8; 8], bit: usize) {
    // bit is 1-indexed; MSB of byte 0 is bit 1
    let byte_idx = (bit - 1) / 8;
    let bit_pos  = 7 - ((bit - 1) % 8);
    bitmap[byte_idx] |= 1 << bit_pos;
}

// ─── Samples ─────────────────────────────────────────────────────────────────

fn sample_0200_purchase() -> (String, &'static str) {
    let mut b = Iso8583Builder::new("0200");
    b.llvar (2,  "4111111111111111")          // PAN
     .fixed (3,  "000000", 6)                 // Processing Code: Purchase
     .fixed (4,  "000000150000", 12)          // Amount: 1500.00
     .fixed (7,  "0312143025", 10)            // Transmission DateTime
     .fixed (11, "000001", 6)                 // STAN
     .fixed (12, "143025", 6)                 // Time Local
     .fixed (13, "0312", 4)                   // Date Local
     .fixed (14, "2512", 4)                   // Expiry: Dec 2025
     .fixed (18, "5411", 4)                   // MCC: Grocery Stores
     .fixed (22, "051", 3)                    // POS Entry: Chip
     .fixed (25, "00", 2)                     // POS Condition: Normal
     .fixed (37, "000000000001", 12)          // RRN
     .fixed (41, "TERM0001", 8)               // Terminal ID
     .fixed (42, "MERCHANT000001 ", 15)       // Merchant ID (padded)
     .fixed (43, "WARUNG MAKAN SEDERHANA  JAKARTA     ID  ", 40) // Name/Location
     .fixed (49, "360", 3);                   // Currency: IDR
    (b.build(), "0200 — Purchase Auth Request (IDR 1500.00, Chip)")
}

fn sample_0210_approved() -> (String, &'static str) {
    let mut b = Iso8583Builder::new("0210");
    b.llvar (2,  "4111111111111111")
     .fixed (3,  "000000", 6)
     .fixed (4,  "000000150000", 12)
     .fixed (7,  "0312143026", 10)
     .fixed (11, "000001", 6)
     .fixed (12, "143026", 6)
     .fixed (13, "0312", 4)
     .fixed (14, "2512", 4)
     .fixed (22, "051", 3)
     .fixed (25, "00", 2)
     .fixed (37, "000000000001", 12)
     .fixed (38, "AUTH01", 6)               // Auth ID
     .fixed (39, "00", 2)                   // Response: Approved
     .fixed (41, "TERM0001", 8)
     .fixed (42, "MERCHANT000001 ", 15)
     .fixed (49, "360", 3);
    (b.build(), "0210 — Purchase Auth Response (Approved 00)")
}

fn sample_0200_declined() -> (String, &'static str) {
    let mut b = Iso8583Builder::new("0210");
    b.llvar (2,  "5500005555555559")
     .fixed (3,  "000000", 6)
     .fixed (4,  "000002500000", 12)
     .fixed (7,  "0312150000", 10)
     .fixed (11, "000002", 6)
     .fixed (12, "150000", 6)
     .fixed (13, "0312", 4)
     .fixed (14, "2301", 4)                   // Expired card (Jan 2023)
     .fixed (22, "021", 3)                    // POS Entry: Magstripe
     .fixed (25, "00", 2)
     .fixed (37, "000000000002", 12)
     .fixed (38, "      ", 6)                 // No auth ID
     .fixed (39, "54", 2)                     // Response: Expired Card
     .fixed (41, "TERM0002", 8)
     .fixed (42, "ONLINE-SHOP-001", 15)
     .fixed (49, "360", 3);
    (b.build(), "0210 — Auth Response (Declined 54: Expired Card)")
}

fn sample_0400_reversal() -> (String, &'static str) {
    let mut b = Iso8583Builder::new("0400");
    b.llvar (2,  "4111111111111111")
     .fixed (3,  "000000", 6)
     .fixed (4,  "000000150000", 12)
     .fixed (7,  "0312143100", 10)
     .fixed (11, "000003", 6)
     .fixed (12, "143100", 6)
     .fixed (13, "0312", 4)
     .fixed (14, "2512", 4)
     .fixed (22, "051", 3)
     .fixed (25, "00", 2)
     .fixed (37, "000000000003", 12)
     .fixed (38, "AUTH01", 6)
     .fixed (39, "00", 2)
     .fixed (41, "TERM0001", 8)
     .fixed (42, "MERCHANT000001 ", 15)
     .fixed (49, "360", 3)
     .fixed (90, "020000000000010312143025000000000000000000", 42); // Original data elements
    (b.build(), "0400 — Reversal Request (original STAN 000001)")
}

fn sample_0800_echo() -> (String, &'static str) {
    let mut b = Iso8583Builder::new("0800");
    b.fixed (7,  "0312000000", 10)
     .fixed (11, "999999", 6)
     .fixed (70, "301", 3);  // Network Management: Sign-On
    (b.build(), "0800 — Network Management (Sign-On Request)")
}

// ─── JSON samples ─────────────────────────────────────────────────────────────

fn json_transaction() -> (&'static str, &'static str) {
    (
        r#"{"transaction":{"id":"TXN-20260312-001","type":"purchase","status":"approved","amount":{"value":150000,"currency":"IDR","formatted":"Rp 1.500,00"},"timestamp":"2026-03-12T14:30:25+07:00","merchant":{"id":"MERCHANT000001","name":"Warung Makan Sederhana","category":{"code":"5411","description":"Grocery Stores & Supermarkets"},"location":{"address":"Jl. Sudirman No. 1","city":"Jakarta","country":"ID","postal_code":"10220"}},"card":{"pan_masked":"411111******1111","expiry":"12/25","entry_mode":"chip","scheme":"VISA"},"terminal":{"id":"TERM0001","type":"EDC","acquirer_id":"BNI001"},"auth":{"stan":"000001","rrn":"000000000001","approval_code":"AUTH01","response_code":"00","response_desc":"Approved"},"fees":{"mdr":0.0070,"mdr_amount":1050,"net_amount":148950}}}"#,
        "E-Commerce Transaction Object"
    )
}

fn json_iso_config() -> (&'static str, &'static str) {
    (
        r#"{"iso8583":{"version":"1987","encoding":"ASCII","fields":{"2":{"name":"PAN","type":"LLVAR","data_type":"N","max_length":19,"sensitive":true},"3":{"name":"Processing Code","type":"FIXED","data_type":"N","length":6},"4":{"name":"Amount Transaction","type":"FIXED","data_type":"N","length":12},"7":{"name":"Transmission Date & Time","type":"FIXED","data_type":"N","length":10},"11":{"name":"STAN","type":"FIXED","data_type":"N","length":6},"12":{"name":"Time Local Transaction","type":"FIXED","data_type":"N","length":6},"13":{"name":"Date Local Transaction","type":"FIXED","data_type":"N","length":4},"22":{"name":"POS Entry Mode","type":"FIXED","data_type":"N","length":3},"37":{"name":"RRN","type":"FIXED","data_type":"AN","length":12},"38":{"name":"Auth ID Response","type":"FIXED","data_type":"AN","length":6},"39":{"name":"Response Code","type":"FIXED","data_type":"AN","length":2},"41":{"name":"Terminal ID","type":"FIXED","data_type":"ANS","length":8},"42":{"name":"Merchant ID","type":"FIXED","data_type":"ANS","length":15},"49":{"name":"Currency Code","type":"FIXED","data_type":"AN","length":3}},"response_codes":{"00":"Approved","01":"Refer to Card Issuer","05":"Do Not Honour","12":"Invalid Transaction","14":"Invalid Card Number","51":"Insufficient Funds","54":"Expired Card","55":"Incorrect PIN","91":"Issuer Inoperative","96":"System Malfunction"},"mti":{"0200":"Financial Transaction Request","0210":"Financial Transaction Response","0400":"Reversal Request","0410":"Reversal Response","0800":"Network Management Request","0810":"Network Management Response"}}}"#,
        "ISO 8583 Field Configuration"
    )
}

fn json_bank_api() -> (&'static str, &'static str) {
    (
        r#"{"api":{"version":"v2","base_url":"https://api.bank.co.id/payment","auth":{"type":"OAuth2","token_endpoint":"/oauth/token","scopes":["payment:read","payment:write","settlement:read"]},"endpoints":[{"method":"POST","path":"/transactions/authorize","description":"Authorize a card transaction","request":{"content_type":"application/json","body":{"required":["amount","currency","card","merchant"],"amount":{"type":"number","min":100,"max":100000000},"currency":{"type":"string","enum":["IDR","USD","SGD"]}}},"response":{"200":{"status":"approved","transaction_id":"string","approval_code":"string"},"402":{"status":"declined","reason_code":"string","reason_desc":"string"}}},{"method":"POST","path":"/transactions/void","description":"Void/Reverse a transaction","request":{"body":{"required":["original_transaction_id","reason"]}},"response":{"200":{"status":"voided"},"404":{"error":"Transaction not found"}}}],"rate_limits":{"requests_per_minute":600,"requests_per_day":100000},"timeout_ms":30000,"retry":{"max_attempts":3,"backoff_ms":500}}}"#,
        "Bank Payment API Spec"
    )
}

fn json_settlement() -> (&'static str, &'static str) {
    (
        r#"{"settlement":{"batch_id":"BATCH-20260312","date":"2026-03-12","acquirer":{"institution_code":"014","name":"BCA","bin_range":["400000-499999","510000-559999"]},"summary":{"total_transactions":1547,"approved":1489,"declined":58,"reversed":12,"total_amount":{"debit":{"count":1245,"amount":187650000},"credit":{"count":244,"amount":32100000},"net":{"amount":155550000,"currency":"IDR"}},"by_card_scheme":{"VISA":{"count":892,"amount":98750000},"MASTERCARD":{"count":597,"amount":88900000}},"by_mcc":{"5411":{"description":"Grocery","count":456,"amount":34200000},"5812":{"description":"Restaurant","count":312,"amount":18900000},"4816":{"description":"Digital Goods","count":231,"amount":45600000}}},"status":"pending_upload","generated_at":"2026-03-12T23:59:59+07:00"}}"#,
        "Daily Settlement Batch"
    )
}

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let separator = "═".repeat(80);

    println!("\n{}", separator);
    println!("  ISO 8583 & JSON SAMPLE DATA GENERATOR");
    println!("  Generated: 2026-03-12  |  For use with iso_json_tool TUI");
    println!("{}\n", separator);

    // ── ISO 8583 samples ──
    println!("┌─────────────────────────────────────────┐");
    println!("│         ISO 8583 HEX SAMPLES            │");
    println!("│  (copy hex string → paste into F2 tab)  │");
    println!("└─────────────────────────────────────────┘\n");

    let iso_samples: Vec<(String, &str)> = vec![
        sample_0200_purchase(),
        sample_0210_approved(),
        sample_0200_declined(),
        sample_0400_reversal(),
        sample_0800_echo(),
    ];

    for (i, (hex, label)) in iso_samples.iter().enumerate() {
        println!("── Sample ISO-{} ──────────────────────────────────────────────────────", i + 1);
        println!("  Description : {}", label);
        println!("  Length      : {} hex chars ({} bytes)", hex.len(), hex.len() / 2);
        println!("  Hex String  :\n{}\n", hex);
    }

    // ── JSON samples ──
    println!("\n┌──────────────────────────────────────────┐");
    println!("│          JSON MINIFY SAMPLES             │");
    println!("│  (copy JSON → paste into F1 tab → F5)   │");
    println!("└──────────────────────────────────────────┘\n");

    let json_samples = vec![
        json_transaction(),
        json_iso_config(),
        json_bank_api(),
        json_settlement(),
    ];

    for (i, (json, label)) in json_samples.iter().enumerate() {
        println!("── Sample JSON-{} ─────────────────────────────────────────────────────", i + 1);
        println!("  Description : {}", label);
        println!("  Size        : {} chars", json.len());
        println!("  JSON        :\n{}\n", json);
    }

    println!("{}", separator);
    println!("  USAGE: cargo run --example gen_samples > samples.txt");
    println!("{}\n", separator);
}
