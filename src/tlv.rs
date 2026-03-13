// ─── TLV / EMV Decoder ────────────────────────────────────────────────────────
// Supports BER-TLV encoding used in ISO 7816 / EMV chip card data (F55)

use std::collections::HashMap;

/// One decoded TLV node
pub struct TlvNode {
    pub tag: String,       // hex tag e.g. "9F26"
    pub name: String,
    pub class: TagClass,
    pub constructed: bool,
    pub length: usize,
    pub value_hex: String,
    pub value_display: String,
    pub depth: usize,
    pub children: Vec<TlvNode>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TagClass {
    Universal,
    Application,
    ContextSpecific,
    Private,
}
impl TagClass {
    fn label(&self) -> &'static str {
        match self {
            TagClass::Universal        => "UNIV",
            TagClass::Application      => "APPL",
            TagClass::ContextSpecific  => "CTXT",
            TagClass::Private          => "PRIV",
        }
    }
}

/// Parse hex string into TLV nodes
pub fn decode(hex_input: &str) -> Result<Vec<TlvNode>, String> {
    // Strip whitespace and uppercase
    let hex_raw: String = hex_input
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();

    // Detect non-hex chars and handle masking placeholders gracefully
    let bad: Vec<(usize, char)> = hex_raw
        .char_indices()
        .filter(|(_, c)| !c.is_ascii_hexdigit())
        .take(5)
        .collect();

    let hex: String = if !bad.is_empty() {
        let all_masking = bad.iter().all(|(_, c)| matches!(c, 'X' | '*' | '#' | '?'));
        if all_masking {
            // Auto-replace masking chars with F (EMV padding convention)
            hex_raw.chars().map(|c| match c {
                'X' | '*' | '#' | '?' => 'F',
                c => c,
            }).collect()
        } else {
            let sample: Vec<String> = bad.iter()
                .map(|(i, c)| format!("pos {}: {:?}", i, c))
                .collect();
            return Err(format!(
                "Input mengandung karakter non-hex: {}\nPastikan input adalah hex string murni (0-9, A-F).\nMasking char (X *) otomatis diganti F.",
                sample.join(", ")
            ));
        }
    } else {
        hex_raw
    };

    if hex.len() % 2 != 0 {
        return Err(format!(
            "Panjang hex ganjil: {} karakter (harus genap, setiap byte = 2 hex char)",
            hex.len()
        ));
    }

    let bytes = hex_to_bytes(&hex)?;
    parse_tlv(&bytes, 0)
}

fn parse_tlv(data: &[u8], depth: usize) -> Result<Vec<TlvNode>, String> {
    let dict = emv_dict();
    let mut nodes = Vec::new();
    let mut pos = 0usize;

    while pos < data.len() {
        // ── Read tag ──
        let tag_start = pos;
        let first_byte = data[pos];
        pos += 1;

        let class = match (first_byte >> 6) & 0x03 {
            0 => TagClass::Universal,
            1 => TagClass::Application,
            2 => TagClass::ContextSpecific,
            _ => TagClass::Private,
        };
        let constructed = (first_byte & 0x20) != 0;
        let tag_number_low = first_byte & 0x1F;

        // Multi-byte tag?
        if tag_number_low == 0x1F {
            while pos < data.len() {
                let b = data[pos];
                pos += 1;
                if (b & 0x80) == 0 {
                    break;
                }
            }
        }

        let tag_hex = bytes_to_hex(&data[tag_start..pos]);

        // ── Read length ──
        if pos >= data.len() {
            return Err(format!("Unexpected end after tag {}", tag_hex));
        }
        let len_byte = data[pos];
        pos += 1;

        let length = if len_byte <= 0x7F {
            len_byte as usize
        } else {
            let num_bytes = (len_byte & 0x7F) as usize;
            if pos + num_bytes > data.len() {
                return Err(format!("Not enough data for length of tag {}", tag_hex));
            }
            let mut l = 0usize;
            for _ in 0..num_bytes {
                l = (l << 8) | (data[pos] as usize);
                pos += 1;
            }
            l
        };

        // ── Read value ──
        if pos + length > data.len() {
            return Err(format!(
                "Tag {} claims length {} but only {} bytes remain",
                tag_hex, length, data.len() - pos
            ));
        }
        let value_bytes = &data[pos..pos + length];
        pos += length;

        let value_hex = bytes_to_hex(value_bytes);
        let info = dict.get(tag_hex.as_str()).copied();
        let name = info.map(|i| i.0).unwrap_or("Unknown / Private");
        let fmt  = info.map(|i| i.1).unwrap_or(TlvFmt::B);

        let value_display = format_value(value_bytes, fmt, &tag_hex);

        let children = if constructed {
            parse_tlv(value_bytes, depth + 1).unwrap_or_default()
        } else {
            Vec::new()
        };

        nodes.push(TlvNode {
            tag: tag_hex,
            name: name.to_string(),
            class,
            constructed,
            length,
            value_hex,
            value_display,
            depth,
            children,
        });
    }

    Ok(nodes)
}

// ─── Value Formatter ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum TlvFmt {
    B,    // Binary/hex
    N,    // BCD Numeric
    AN,   // ASCII
    ANS,  // ASCII + special chars
    CN,   // Compressed numeric (right-justified, 'F' padding)
    DOL,  // Data Object List
    Bit,  // Bitmask (show bit descriptions)
}

fn format_value(bytes: &[u8], fmt: TlvFmt, tag: &str) -> String {
    match fmt {
        TlvFmt::AN | TlvFmt::ANS => {
            let s: String = bytes.iter().map(|&b| {
                if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }
            }).collect();
            s
        }
        TlvFmt::N => {
            // BCD: each nibble is a digit
            bytes.iter().map(|&b| format!("{:02X}", b)).collect::<String>()
        }
        TlvFmt::CN => {
            // Compressed numeric: remove trailing F padding
            let s: String = bytes.iter().map(|&b| format!("{:02X}", b)).collect();
            s.trim_end_matches('F').to_string()
        }
        TlvFmt::DOL => {
            // Data Object List: parse tag+length pairs
            let mut out = String::new();
            let mut i = 0;
            while i < bytes.len() {
                let t_start = i;
                let first = bytes[i]; i += 1;
                if (first & 0x1F) == 0x1F {
                    while i < bytes.len() {
                        let b = bytes[i]; i += 1;
                        if (b & 0x80) == 0 { break; }
                    }
                }
                let t_hex = bytes_to_hex(&bytes[t_start..i]);
                if i >= bytes.len() { break; }
                let l = bytes[i] as usize; i += 1;
                let dict = emv_dict();
                let name = dict.get(t_hex.as_str()).map(|x| x.0).unwrap_or("?");
                out.push_str(&format!("  {} ({}, {} bytes)\n", t_hex, name, l));
            }
            out
        }
        TlvFmt::Bit => {
            annotate_bitmask(bytes, tag)
        }
        TlvFmt::B => {
            bytes_to_hex(bytes)
        }
    }
}

fn annotate_bitmask(bytes: &[u8], tag: &str) -> String {
    let hex = bytes_to_hex(bytes);
    let desc = match tag {
        "9F27" => {
            let b0 = bytes[0];
            let cid = match b0 & 0xC0 {
                0x40 => "TC (Transaction Certificate)",
                0x80 => "ARQC (Auth Request Cryptogram)",
                0x00 => "AAC (App Auth Cryptogram - declined)",
                _    => "Unknown CID",
            };
            format!("{} → {}", hex, cid)
        }
        "82" => {
            let mut bits = Vec::new();
            if bytes.len() >= 2 {
                let w = ((bytes[0] as u16) << 8) | (bytes[1] as u16);
                if (w >> 15) & 1 == 1 { bits.push("SDA supported"); }
                if (w >> 14) & 1 == 1 { bits.push("DDA supported"); }
                if (w >> 13) & 1 == 1 { bits.push("Cardholder verification"); }
                if (w >> 12) & 1 == 1 { bits.push("Terminal risk mgmt"); }
                if (w >> 11) & 1 == 1 { bits.push("Issuer auth required"); }
                if (w >>  8) & 1 == 1 { bits.push("CDA supported"); }
            }
            format!("{} [{}]", hex, bits.join(", "))
        }
        "95" => {
            let mut bits = Vec::new();
            if !bytes.is_empty() {
                let b = bytes[0];
                if (b >> 7) & 1 == 1 { bits.push("Offline data auth not performed"); }
                if (b >> 6) & 1 == 1 { bits.push("SDA failed"); }
                if (b >> 5) & 1 == 1 { bits.push("ICC data missing"); }
                if (b >> 4) & 1 == 1 { bits.push("Card appears on hotlist"); }
                if (b >> 3) & 1 == 1 { bits.push("DDA failed"); }
                if (b >> 2) & 1 == 1 { bits.push("CDA failed"); }
            }
            if !bits.is_empty() {
                format!("{} [{}]", hex, bits.join(", "))
            } else {
                format!("{} [All OK]", hex)
            }
        }
        "9F10" => {
            // Issuer Application Data
            if bytes.len() >= 1 {
                let len = bytes[0] as usize;
                format!("{} (IAD len={}, CVR starts at byte 6)", hex, len)
            } else { hex }
        }
        _ => hex,
    };
    desc
}

// ─── EMV Tag Dictionary ───────────────────────────────────────────────────────

fn emv_dict() -> HashMap<&'static str, (&'static str, TlvFmt)> {
    let mut m = HashMap::new();
    // Core file / application
    m.insert("6F", ("FCI Template",                            TlvFmt::B));
    m.insert("84", ("DF Name / AID",                           TlvFmt::AN));
    m.insert("A5", ("FCI Proprietary Template",                TlvFmt::B));
    m.insert("70", ("EMV Record / Response Template",          TlvFmt::B));
    m.insert("77", ("Response Message Template Format 2",      TlvFmt::B));
    m.insert("80", ("Response Message Template Format 1",      TlvFmt::B));

    // Card data
    m.insert("5A", ("Application PAN",                         TlvFmt::CN));
    m.insert("5F24", ("Application Expiration Date (YYMMDD)", TlvFmt::N));
    m.insert("5F25", ("Application Effective Date (YYMMDD)",  TlvFmt::N));
    m.insert("5F20", ("Cardholder Name",                       TlvFmt::AN));
    m.insert("5F28", ("Issuer Country Code",                   TlvFmt::N));
    m.insert("5F2A", ("Transaction Currency Code",             TlvFmt::N));
    m.insert("5F2D", ("Language Preference",                   TlvFmt::AN));
    m.insert("5F30", ("Service Code",                          TlvFmt::N));
    m.insert("5F34", ("Application PAN Sequence Number",       TlvFmt::N));
    m.insert("5F50", ("Issuer URL",                            TlvFmt::ANS));
    m.insert("57", ("Track 2 Equivalent Data",                 TlvFmt::CN));
    m.insert("9F1F", ("Track 1 Discretionary Data",            TlvFmt::AN));
    m.insert("9F20", ("Track 2 Discretionary Data",            TlvFmt::CN));

    // Application
    m.insert("4F", ("Application Identifier (AID)",            TlvFmt::B));
    m.insert("50", ("Application Label",                       TlvFmt::AN));
    m.insert("61", ("Application Template",                    TlvFmt::B));
    m.insert("87", ("Application Priority Indicator",          TlvFmt::B));
    m.insert("9F12", ("Application Preferred Name",            TlvFmt::AN));
    m.insert("82", ("Application Interchange Profile (AIP)",   TlvFmt::Bit));
    m.insert("94", ("Application File Locator (AFL)",          TlvFmt::B));
    m.insert("9F07", ("Application Usage Control",             TlvFmt::B));
    m.insert("9F08", ("Application Version Number (ICC)",      TlvFmt::N));
    m.insert("9F09", ("Application Version Number (Terminal)", TlvFmt::N));
    m.insert("9F36", ("Application Transaction Counter (ATC)", TlvFmt::N));
    m.insert("9F4F", ("Log Format (DOL)",                      TlvFmt::DOL));

    // Amounts
    m.insert("9F02", ("Amount Authorised (numeric)",           TlvFmt::N));
    m.insert("9F03", ("Amount Other (numeric)",                TlvFmt::N));
    m.insert("9F04", ("Amount Other (binary)",                 TlvFmt::B));

    // Transaction context
    m.insert("9A",   ("Transaction Date (YYMMDD)",             TlvFmt::N));
    m.insert("9F21", ("Transaction Time (HHMMSS)",             TlvFmt::N));
    m.insert("9C",   ("Transaction Type",                      TlvFmt::N));
    m.insert("9F1A", ("Terminal Country Code",                 TlvFmt::N));
    m.insert("5F2A", ("Transaction Currency Code",             TlvFmt::N));
    m.insert("9F15", ("Merchant Category Code (MCC)",          TlvFmt::N));
    m.insert("9F16", ("Merchant Identifier",                   TlvFmt::AN));
    m.insert("9F4E", ("Merchant Name and Location",            TlvFmt::ANS));

    // Terminal
    m.insert("9F1B", ("Terminal Floor Limit",                  TlvFmt::B));
    m.insert("9F1C", ("Terminal Identification",               TlvFmt::AN));
    m.insert("9F1D", ("Terminal Risk Management Data",         TlvFmt::B));
    m.insert("9F1E", ("Interface Device Serial Number",        TlvFmt::AN));
    m.insert("9F33", ("Terminal Capabilities",                 TlvFmt::B));
    m.insert("9F35", ("Terminal Type",                         TlvFmt::N));
    m.insert("9F40", ("Additional Terminal Capabilities",      TlvFmt::B));
    m.insert("9F6D", ("Mag-stripe Application Version Number", TlvFmt::B));

    // Cryptograms & security
    m.insert("9F26", ("Application Cryptogram (AC/ARQC/TC)",   TlvFmt::B));
    m.insert("9F27", ("Cryptogram Information Data (CID)",     TlvFmt::Bit));
    m.insert("9F10", ("Issuer Application Data (IAD)",         TlvFmt::Bit));
    m.insert("9F37", ("Unpredictable Number",                   TlvFmt::B));
    m.insert("9F45", ("Data Authentication Code",              TlvFmt::B));
    m.insert("9F4B", ("Signed Dynamic Application Data",       TlvFmt::B));
    m.insert("9F4C", ("ICC Dynamic Number",                    TlvFmt::B));
    m.insert("9F69", ("Card Authentication Related Data",      TlvFmt::B));
    m.insert("9F6E", ("Form Factor Indicator / 3rd Party Data",TlvFmt::B));

    // Risk & verification
    m.insert("8E", ("Cardholder Verification Method (CVM) List", TlvFmt::B));
    m.insert("8F", ("Certification Authority Public Key Index",TlvFmt::N));
    m.insert("90", ("Issuer Public Key Certificate",           TlvFmt::B));
    m.insert("92", ("Issuer Public Key Remainder",             TlvFmt::B));
    m.insert("93", ("Signed Static Application Data",          TlvFmt::B));
    m.insert("9F32", ("Issuer Public Key Exponent",            TlvFmt::B));
    m.insert("9F46", ("ICC Public Key Certificate",            TlvFmt::B));
    m.insert("9F47", ("ICC Public Key Exponent",               TlvFmt::B));
    m.insert("9F48", ("ICC Public Key Remainder",              TlvFmt::B));
    m.insert("9F49", ("Dynamic Data Auth Data Object List (DDOL)", TlvFmt::DOL));
    m.insert("97",   ("Transaction Certificate Data Object List (TDOL)", TlvFmt::DOL));

    // TVR & CVR
    m.insert("95", ("Terminal Verification Results (TVR)",     TlvFmt::Bit));
    m.insert("9B", ("Transaction Status Information (TSI)",    TlvFmt::Bit));

    // PIN
    m.insert("9F34", ("CVM Results",                           TlvFmt::B));

    // Issuer
    m.insert("9F38", ("Processing Options Data Object List (PDOL)", TlvFmt::DOL));
    m.insert("9F39", ("Point-of-Service (POS) Entry Mode",     TlvFmt::N));
    m.insert("9F41", ("Transaction Sequence Counter",          TlvFmt::N));
    m.insert("9F42", ("Application Currency Code",             TlvFmt::N));
    m.insert("9F43", ("Application Currency Exponent",         TlvFmt::N));
    m.insert("9F44", ("Application Currency Exponent",         TlvFmt::N));
    m.insert("9F53", ("Transaction Category Code",             TlvFmt::AN));
    m.insert("9F5A", ("Application Program Identifier",        TlvFmt::B));
    m.insert("9F5B", ("Issuer Script Results",                 TlvFmt::B));
    m.insert("9F5C", ("DS Requested Operator ID",              TlvFmt::B));
    m.insert("9F72", ("Contactless Reader Capabilities",       TlvFmt::B));
    m.insert("9F74", ("VLP Issuer Authorization Code",         TlvFmt::AN));

    // Mastercard / Visa proprietary
    m.insert("9F60", ("CVC3 (Track 1)",                        TlvFmt::N));
    m.insert("9F61", ("CVC3 (Track 2)",                        TlvFmt::N));
    m.insert("9F6B", ("Track 2 Data",                          TlvFmt::CN));
    m.insert("9F6C", ("Mag-stripe Application Version Number", TlvFmt::B));
    m.insert("DF78", ("Contactless Floor Limit",               TlvFmt::N));

    m
}

// ─── Output Formatter ─────────────────────────────────────────────────────────

pub fn format_nodes(nodes: &[TlvNode]) -> String {
    let mut out = String::new();
    out.push_str("╔══════════════════════════════════════════════════════════╗\n");
    out.push_str("║              TLV / EMV DECODE RESULT                    ║\n");
    out.push_str("╚══════════════════════════════════════════════════════════╝\n\n");
    format_nodes_recursive(nodes, &mut out, 0);
    out
}

fn format_nodes_recursive(nodes: &[TlvNode], out: &mut String, _depth: usize) {
    for node in nodes {
        let indent = "  ".repeat(node.depth);
        let constr_mark = if node.constructed { " ▼" } else { "" };
        let class_label = node.class.label();

        out.push_str(&format!(
            "{}┌─ [{}] {} · {} byte(s){}\n",
            indent, node.tag, class_label, node.length, constr_mark
        ));
        out.push_str(&format!(
            "{}│  Name  : {}\n",
            indent, node.name
        ));
        if !node.children.is_empty() {
            out.push_str(&format!("{}│  Value : (constructed — see children)\n", indent));
        } else if node.value_display != node.value_hex && !node.value_display.is_empty() {
            out.push_str(&format!("{}│  Hex   : {}\n", indent, node.value_hex));
            out.push_str(&format!("{}│  Value : {}\n", indent, node.value_display));
        } else {
            out.push_str(&format!("{}│  Value : {}\n", indent, node.value_hex));
        }
        out.push_str(&format!("{}└─\n", indent));

        if !node.children.is_empty() {
            format_nodes_recursive(&node.children, out, node.depth + 1);
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| format!("Invalid hex at {}: '{}'", i, &hex[i..i + 2]))
        })
        .collect()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02X}", b)).collect()
}
