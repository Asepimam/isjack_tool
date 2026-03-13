// ISO 8583 decoder
#![allow(dead_code)]

/// ISO 8583 Field Definition
#[derive(Clone)]
pub struct FieldDef {
    pub number: usize,
    pub name: &'static str,
    pub length_type: LengthType,
    pub data_type: DataType,
    pub max_len: usize,
}

#[derive(Clone)]
pub enum LengthType {
    Fixed,
    LLVar,       // 2-digit ASCII length prefix
    LLLVar,      // 3-digit ASCII length prefix
    TagLLLVar,   // 3-char tag + 3-digit ASCII length prefix (used by F48 in some bank systems)
}

#[derive(Clone)]
pub enum DataType {
    N,   // Numeric
    AN,  // Alphanumeric
    ANS, // Alphanumeric Special
    B,   // Binary (hex)
    Z,   // Track Data
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::N => write!(f, "N"),
            DataType::AN => write!(f, "AN"),
            DataType::ANS => write!(f, "ANS"),
            DataType::B => write!(f, "B"),
            DataType::Z => write!(f, "Z"),
        }
    }
}

/// Returns the field definition for fields 1-128
pub fn get_field_def(n: usize) -> Option<FieldDef> {
    use LengthType::*;
    use DataType::*;
    let def = match n {
        1  => FieldDef { number: 1,  name: "Bitmap Secondary",                  length_type: Fixed,  data_type: B,   max_len: 8  },
        2  => FieldDef { number: 2,  name: "PAN",                                length_type: LLVar,  data_type: N,   max_len: 19 },
        3  => FieldDef { number: 3,  name: "Processing Code",                    length_type: Fixed,  data_type: N,   max_len: 6  },
        4  => FieldDef { number: 4,  name: "Amount Transaction",                 length_type: Fixed,  data_type: N,   max_len: 12 },
        5  => FieldDef { number: 5,  name: "Amount Settlement",                  length_type: Fixed,  data_type: N,   max_len: 12 },
        6  => FieldDef { number: 6,  name: "Amount Cardholder Billing",          length_type: Fixed,  data_type: N,   max_len: 12 },
        7  => FieldDef { number: 7,  name: "Transmission Date & Time",           length_type: Fixed,  data_type: N,   max_len: 10 },
        8  => FieldDef { number: 8,  name: "Amount Cardholder Billing Fee",      length_type: Fixed,  data_type: N,   max_len: 8  },
        9  => FieldDef { number: 9,  name: "Conversion Rate Settlement",         length_type: Fixed,  data_type: N,   max_len: 8  },
        10 => FieldDef { number: 10, name: "Conversion Rate Cardholder",         length_type: Fixed,  data_type: N,   max_len: 8  },
        11 => FieldDef { number: 11, name: "STAN",                               length_type: Fixed,  data_type: N,   max_len: 6  },
        12 => FieldDef { number: 12, name: "Time Local Transaction",             length_type: Fixed,  data_type: N,   max_len: 6  },
        13 => FieldDef { number: 13, name: "Date Local Transaction",             length_type: Fixed,  data_type: N,   max_len: 4  },
        14 => FieldDef { number: 14, name: "Date Expiration",                    length_type: Fixed,  data_type: N,   max_len: 4  },
        15 => FieldDef { number: 15, name: "Date Settlement",                    length_type: Fixed,  data_type: N,   max_len: 4  },
        16 => FieldDef { number: 16, name: "Date Conversion",                    length_type: Fixed,  data_type: N,   max_len: 4  },
        17 => FieldDef { number: 17, name: "Date Capture",                       length_type: Fixed,  data_type: N,   max_len: 4  },
        18 => FieldDef { number: 18, name: "Merchant Type (MCC)",                length_type: Fixed,  data_type: N,   max_len: 4  },
        19 => FieldDef { number: 19, name: "Acquiring Institution Country Code", length_type: Fixed,  data_type: N,   max_len: 3  },
        20 => FieldDef { number: 20, name: "PAN Extended Country Code",          length_type: Fixed,  data_type: N,   max_len: 3  },
        21 => FieldDef { number: 21, name: "Forwarding Institution Country Code",length_type: Fixed,  data_type: N,   max_len: 3  },
        22 => FieldDef { number: 22, name: "POS Entry Mode",                     length_type: Fixed,  data_type: N,   max_len: 3  },
        23 => FieldDef { number: 23, name: "Card Sequence Number",               length_type: Fixed,  data_type: N,   max_len: 3  },
        24 => FieldDef { number: 24, name: "Network International Identifier",   length_type: Fixed,  data_type: N,   max_len: 3  },
        25 => FieldDef { number: 25, name: "POS Condition Code",                 length_type: Fixed,  data_type: N,   max_len: 2  },
        26 => FieldDef { number: 26, name: "POS PIN Capture Code",               length_type: Fixed,  data_type: N,   max_len: 2  },
        27 => FieldDef { number: 27, name: "Auth Identification Response Length",length_type: Fixed,  data_type: N,   max_len: 1  },
        28 => FieldDef { number: 28, name: "Amount Transaction Fee",             length_type: Fixed,  data_type: AN,  max_len: 9  }, // ISO x+n8
        29 => FieldDef { number: 29, name: "Amount Settlement Fee",              length_type: Fixed,  data_type: AN,  max_len: 9  }, // ISO x+n8
        30 => FieldDef { number: 30, name: "Amount Transaction Processing Fee",  length_type: Fixed,  data_type: AN,  max_len: 9  }, // ISO x+n8
        31 => FieldDef { number: 31, name: "Amount Settlement Processing Fee",   length_type: Fixed,  data_type: AN,  max_len: 9  }, // ISO x+n8
        32 => FieldDef { number: 32, name: "Acquiring Institution Code",         length_type: LLVar,  data_type: N,   max_len: 11 },
        33 => FieldDef { number: 33, name: "Forwarding Institution Code",        length_type: LLVar,  data_type: N,   max_len: 11 },
        34 => FieldDef { number: 34, name: "PAN Extended",                       length_type: LLVar,  data_type: Z,   max_len: 28 },
        35 => FieldDef { number: 35, name: "Track 2 Data",                       length_type: LLVar,  data_type: Z,   max_len: 37 },
        36 => FieldDef { number: 36, name: "Track 3 Data",                       length_type: LLLVar, data_type: Z,   max_len: 104},
        37 => FieldDef { number: 37, name: "RRN (Retrieval Reference Number)",   length_type: Fixed,  data_type: AN,  max_len: 12 },
        38 => FieldDef { number: 38, name: "Authorization ID Response",          length_type: Fixed,  data_type: AN,  max_len: 6  },
        39 => FieldDef { number: 39, name: "Response Code",                      length_type: Fixed,  data_type: AN,  max_len: 2  },
        40 => FieldDef { number: 40, name: "Service Restriction Code",           length_type: Fixed,  data_type: AN,  max_len: 3  },
        41 => FieldDef { number: 41, name: "Card Acceptor Terminal ID",          length_type: Fixed,  data_type: ANS, max_len: 8  },
        42 => FieldDef { number: 42, name: "Card Acceptor ID Code",              length_type: Fixed,  data_type: ANS, max_len: 15 },
        43 => FieldDef { number: 43, name: "Card Acceptor Name/Location",        length_type: Fixed,  data_type: ANS, max_len: 40 },
        44 => FieldDef { number: 44, name: "Additional Response Data",           length_type: LLVar,  data_type: AN,  max_len: 25 },
        45 => FieldDef { number: 45, name: "Track 1 Data",                       length_type: LLVar,  data_type: ANS, max_len: 76 },
        46 => FieldDef { number: 46, name: "Additional Data ISO",                length_type: LLLVar, data_type: AN,  max_len: 999},
        47 => FieldDef { number: 47, name: "Additional Data National",           length_type: LLLVar, data_type: AN,  max_len: 999},
        48 => FieldDef { number: 48, name: "Additional Data Private",            length_type: TagLLLVar, data_type: ANS, max_len: 999},
        49 => FieldDef { number: 49, name: "Currency Code Transaction",          length_type: Fixed,  data_type: AN,  max_len: 3  },
        50 => FieldDef { number: 50, name: "Currency Code Settlement",           length_type: Fixed,  data_type: AN,  max_len: 3  },
        51 => FieldDef { number: 51, name: "Currency Code Cardholder Billing",   length_type: Fixed,  data_type: AN,  max_len: 3  },
        52 => FieldDef { number: 52, name: "PIN Data",                           length_type: Fixed,  data_type: B,   max_len: 8  },
        53 => FieldDef { number: 53, name: "Security Related Control Info",      length_type: Fixed,  data_type: N,   max_len: 16 },
        54 => FieldDef { number: 54, name: "Additional Amounts",                 length_type: LLLVar, data_type: AN,  max_len: 120},
        55 => FieldDef { number: 55, name: "ICC Data / EMV Data",                length_type: LLLVar, data_type: B,   max_len: 255},
        56 => FieldDef { number: 56, name: "Reserved ISO",                       length_type: LLLVar, data_type: AN,  max_len: 999},
        57 => FieldDef { number: 57, name: "Reserved National",                  length_type: LLLVar, data_type: AN,  max_len: 999},
        58 => FieldDef { number: 58, name: "Reserved National",                  length_type: LLLVar, data_type: AN,  max_len: 999},
        59 => FieldDef { number: 59, name: "Reserved National",                  length_type: LLLVar, data_type: AN,  max_len: 999},
        60 => FieldDef { number: 60, name: "Reserved Private",                   length_type: LLLVar, data_type: ANS, max_len: 999},
        61 => FieldDef { number: 61, name: "Reserved Private",                   length_type: LLLVar, data_type: ANS, max_len: 999},
        62 => FieldDef { number: 62, name: "Reserved Private",                   length_type: LLLVar, data_type: ANS, max_len: 999},
        63 => FieldDef { number: 63, name: "Reserved Private",                   length_type: LLLVar, data_type: ANS, max_len: 999},
        64 => FieldDef { number: 64, name: "MAC (Message Authentication Code)",  length_type: Fixed,  data_type: B,   max_len: 8  },
        65 => FieldDef { number: 65, name: "Bitmap Extended",                    length_type: Fixed,  data_type: B,   max_len: 8  },
        66 => FieldDef { number: 66, name: "Settlement Code",                    length_type: Fixed,  data_type: N,   max_len: 1  },
        67 => FieldDef { number: 67, name: "Extended Payment Code",              length_type: Fixed,  data_type: N,   max_len: 2  },
        70 => FieldDef { number: 70, name: "Network Management Information Code",length_type: Fixed,  data_type: N,   max_len: 3  },
        74 => FieldDef { number: 74, name: "Credits Number",                     length_type: Fixed,  data_type: N,   max_len: 10 },
        75 => FieldDef { number: 75, name: "Credits Reversal Number",            length_type: Fixed,  data_type: N,   max_len: 10 },
        76 => FieldDef { number: 76, name: "Debits Number",                      length_type: Fixed,  data_type: N,   max_len: 10 },
        77 => FieldDef { number: 77, name: "Debits Reversal Number",             length_type: Fixed,  data_type: N,   max_len: 10 },
        90 => FieldDef { number: 90, name: "Original Data Elements",             length_type: Fixed,  data_type: N,   max_len: 42 },
        95 => FieldDef { number: 95, name: "Replacement Amounts",                length_type: Fixed,  data_type: AN,  max_len: 42 },
        96 => FieldDef { number: 96, name: "Message Security Code",              length_type: Fixed,  data_type: B,   max_len: 8  },
        100=> FieldDef { number: 100,name: "Receiving Institution Code",         length_type: LLVar,  data_type: N,   max_len: 11 },
        101=> FieldDef { number: 101,name: "File Name",                          length_type: LLVar,  data_type: ANS, max_len: 17 },
        102=> FieldDef { number: 102,name: "Account ID 1",                       length_type: LLVar,  data_type: ANS, max_len: 28 },
        103=> FieldDef { number: 103,name: "Account ID 2",                       length_type: LLVar,  data_type: ANS, max_len: 28 },
        104=> FieldDef { number: 104,name: "Transaction Description",            length_type: LLLVar, data_type: ANS, max_len: 100},
        128=> FieldDef { number: 128,name: "MAC Extended",                       length_type: Fixed,  data_type: B,   max_len: 8  },
        _  => return None,
    };
    Some(def)
}

pub struct ParsedField {
    pub number: usize,
    pub name: String,
    pub data_type: String,
    pub length: usize,
    pub value: String,
}

pub struct DecodeResult {
    pub mti: String,
    pub mti_description: String,
    pub primary_bitmap: String,
    pub secondary_bitmap: Option<String>,
    pub fields: Vec<ParsedField>,
    pub errors: Vec<String>,
}

/// Decode MTI description
fn describe_mti(mti: &str) -> String {
    if mti.len() < 4 {
        return "Unknown".to_string();
    }
    let class = match &mti[1..2] {
        "1" => "Authorization",
        "2" => "Financial",
        "3" => "File Action",
        "4" => "Reversal/Chargeback",
        "5" => "Reconciliation",
        "6" => "Administrative",
        "7" => "Fee Collection",
        "8" => "Network Management",
        _   => "Reserved",
    };
    let function = match &mti[2..3] {
        "0" => "Request",
        "1" => "Request Response",
        "2" => "Advice",
        "3" => "Advice Response",
        "4" => "Notification",
        "5" => "Notification Acknowledgement",
        "6" => "Instruction",
        "7" => "Instruction Acknowledgement",
        "8" => "Reserved",
        "9" => "Reserved",
        _   => "Unknown",
    };
    let origin = match &mti[3..4] {
        "0" => "Acquirer",
        "1" => "Acquirer Repeat",
        "2" => "Issuer",
        "3" => "Issuer Repeat",
        "4" => "Other",
        "5" => "Other Repeat",
        _   => "Unknown",
    };
    format!("{} {} from {}", class, function, origin)
}

/// Parse bitmap bytes into field bits (1-indexed: bit 1..=64)
fn parse_bitmap(hex: &str) -> Result<Vec<bool>, String> {
    if hex.len() < 16 {
        return Err(format!("Bitmap too short: {} hex chars (need 16)", hex.len()));
    }
    let mut bits = Vec::new();
    for i in (0..16).step_by(2) {
        let byte_str = &hex[i..i+2];
        let byte = u8::from_str_radix(byte_str, 16)
            .map_err(|_| format!("Invalid hex in bitmap: '{}'", byte_str))?;
        for bit_pos in (0..8).rev() {
            bits.push((byte >> bit_pos) & 1 == 1);
        }
    }
    Ok(bits) // 64 booleans for bits 1..=64
}

/// Main ISO 8583 decode function
/// Input: hex string of the raw ISO 8583 message (ASCII encoding assumed)
pub fn decode(hex_input: &str) -> DecodeResult {
    // Normalize: remove spaces, newlines, convert to uppercase
    let hex: String = hex_input.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();

    let mut result = DecodeResult {
        mti: String::new(),
        mti_description: String::new(),
        primary_bitmap: String::new(),
        secondary_bitmap: None,
        fields: Vec::new(),
        errors: Vec::new(),
    };

    if hex.len() < 20 {
        result.errors.push(format!(
            "Input too short: {} hex chars. Need at least 20 (4 MTI + 16 bitmap)",
            hex.len()
        ));
        return result;
    }

    // Validate all chars are hex
    for (i, c) in hex.chars().enumerate() {
        if !c.is_ascii_hexdigit() {
            result.errors.push(format!("Invalid hex character '{}' at position {}", c, i));
            return result;
        }
    }

    let mut pos = 0usize;

    // ── MTI (4 hex pairs = 4 bytes = 4 ASCII chars) ──
    let mti_hex = &hex[pos..pos + 8];
    pos += 8;
    // Try to decode as ASCII
    let mti_bytes: Vec<u8> = (0..mti_hex.len()).step_by(2)
        .filter_map(|i| u8::from_str_radix(&mti_hex[i..i+2], 16).ok())
        .collect();
    let mti_ascii = String::from_utf8_lossy(&mti_bytes).to_string();
    result.mti = mti_ascii.clone();
    result.mti_description = describe_mti(&mti_ascii);

    // ── Primary Bitmap (16 hex chars = 8 bytes) ──
    if pos + 16 > hex.len() {
        result.errors.push("Not enough data for primary bitmap".to_string());
        return result;
    }
    let primary_bm_hex = &hex[pos..pos + 16];
    pos += 16;
    result.primary_bitmap = primary_bm_hex.to_string();

    let primary_bits = match parse_bitmap(primary_bm_hex) {
        Ok(b) => b,
        Err(e) => {
            result.errors.push(e);
            return result;
        }
    };

    // Check bit 1: if set, secondary bitmap present
    let has_secondary = primary_bits[0]; // bit 1 (index 0)
    let mut all_bits = primary_bits.clone(); // bits 1..=64

    if has_secondary {
        if pos + 16 > hex.len() {
            result.errors.push("Bit 1 set (secondary bitmap) but not enough data".to_string());
        } else {
            let secondary_bm_hex = &hex[pos..pos + 16];
            pos += 16;
            result.secondary_bitmap = Some(secondary_bm_hex.to_string());
            match parse_bitmap(secondary_bm_hex) {
                Ok(sec_bits) => {
                    all_bits.extend(sec_bits); // bits 65..=128
                }
                Err(e) => result.errors.push(e),
            }
        }
    }

    // ── Parse fields ──
    for bit_idx in 1..all_bits.len() { // skip bit 0 (field 1 = secondary bitmap)
        let field_num = bit_idx + 1; // bit index 1 → field 2, etc.
        if bit_idx >= all_bits.len() || !all_bits[bit_idx] {
            continue;
        }

        let def = match get_field_def(field_num) {
            Some(d) => d,
            None => {
                // Unknown field - try to skip? We can't without knowing length.
                result.errors.push(format!("Field {:03} set in bitmap but no definition found - cannot continue parsing", field_num));
                break;
            }
        };

        // Determine data length in bytes
        let data_len_bytes = match def.length_type {
            LengthType::Fixed => def.max_len,
            LengthType::LLVar => {
                // Read 2 ASCII bytes (4 hex chars) for length
                if pos + 4 > hex.len() {
                    result.errors.push(format!("Field {:03}: not enough data for LL prefix", field_num));
                    break;
                }
                let len_hex = &hex[pos..pos + 4];
                pos += 4;
                let len_bytes: Vec<u8> = (0..len_hex.len()).step_by(2)
                    .filter_map(|i| u8::from_str_radix(&len_hex[i..i+2], 16).ok())
                    .collect();
                let len_str = String::from_utf8_lossy(&len_bytes).to_string();
                match len_str.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        result.errors.push(format!("Field {:03}: invalid LL length '{}'", field_num, len_str));
                        break;
                    }
                }
            }
            LengthType::LLLVar => {
                // Read 3 ASCII bytes (6 hex chars) for length
                if pos + 6 > hex.len() {
                    result.errors.push(format!("Field {:03}: not enough data for LLL prefix", field_num));
                    break;
                }
                let len_hex = &hex[pos..pos + 6];
                pos += 6;
                let len_bytes: Vec<u8> = (0..len_hex.len()).step_by(2)
                    .filter_map(|i| u8::from_str_radix(&len_hex[i..i+2], 16).ok())
                    .collect();
                let len_str = String::from_utf8_lossy(&len_bytes).to_string();
                match len_str.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        result.errors.push(format!("Field {:03}: invalid LLL length '{}'", field_num, len_str));
                        break;
                    }
                }
            }
            LengthType::TagLLLVar => {
                // Tag(3 bytes=6 hex) + Length(3 bytes=6 hex) — used by F48 in some bank systems
                if pos + 12 > hex.len() {
                    result.errors.push(format!("Field {:03}: not enough data for Tag+LLL prefix", field_num));
                    break;
                }
                // Skip the tag (3 bytes = 6 hex chars)
                let _tag_hex = &hex[pos..pos + 6];
                pos += 6;
                // Read length (3 bytes = 6 hex chars)
                let len_hex = &hex[pos..pos + 6];
                pos += 6;
                let len_bytes: Vec<u8> = (0..len_hex.len()).step_by(2)
                    .filter_map(|i| u8::from_str_radix(&len_hex[i..i+2], 16).ok())
                    .collect();
                let len_str = String::from_utf8_lossy(&len_bytes).to_string();
                match len_str.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        result.errors.push(format!("Field {:03}: invalid TagLLL length '{}'", field_num, len_str));
                        break;
                    }
                }
            }
        };

        // For binary fields, data_len_bytes is raw bytes
        // For others, data_len_bytes is character count = byte count (ASCII)
        let hex_to_consume = data_len_bytes * 2;

        if pos + hex_to_consume > hex.len() {
            result.errors.push(format!(
                "Field {:03}: needs {} hex chars but only {} remain",
                field_num,
                hex_to_consume,
                hex.len() - pos
            ));
            break;
        }

        let field_hex = &hex[pos..pos + hex_to_consume];
        pos += hex_to_consume;

        // Decode value
        let value = match def.data_type {
            DataType::B => format!("0x{}", field_hex),
            _ => {
                // ASCII decode
                let bytes: Vec<u8> = (0..field_hex.len()).step_by(2)
                    .filter_map(|i| u8::from_str_radix(&field_hex[i..i+2], 16).ok())
                    .collect();
                let s = String::from_utf8_lossy(&bytes).to_string();
                // Also annotate special fields
                match field_num {
                    3  => annotate_processing_code(&s),
                    22 => annotate_pos_entry_mode(&s),
                    39 => annotate_response_code(&s),
                    49 | 50 | 51 => annotate_currency_code(&s),
                    _ => s,
                }
            }
        };

        result.fields.push(ParsedField {
            number: field_num,
            name: def.name.to_string(),
            data_type: def.data_type.to_string(),
            length: data_len_bytes,
            value,
        });
    }

    result
}

fn annotate_processing_code(s: &str) -> String {
    if s.len() < 2 { return s.to_string(); }
    let txn_type = match &s[0..2] {
        "00" => "Purchase",
        "01" => "Withdraw",
        "09" => "Purchase + Cashback",
        "20" => "Refund",
        "28" => "Load",
        "31" => "Balance Inquiry",
        "40" => "Transfer",
        _    => "Unknown",
    };
    format!("{} ({})", s, txn_type)
}

fn annotate_pos_entry_mode(s: &str) -> String {
    if s.len() < 2 { return s.to_string(); }
    let mode = match &s[0..2] {
        "00" => "Unknown",
        "01" => "Manual (Key Entered)",
        "02" => "Magnetic Stripe",
        "05" => "ICC (Chip)",
        "07" => "Contactless ICC",
        "10" => "Credentials on File",
        "90" => "Magnetic Stripe (Full Track)",
        "91" => "Contactless Magnetic Stripe",
        _    => "Unknown",
    };
    format!("{} ({})", s, mode)
}

fn annotate_response_code(s: &str) -> String {
    let desc = match s {
        "00" => "Approved",
        "01" => "Refer to Card Issuer",
        "02" => "Refer to Special Conditions",
        "03" => "Invalid Merchant",
        "04" => "Pick-Up Card",
        "05" => "Do Not Honour",
        "06" => "Error",
        "07" => "Pick-Up Card, Special Conditions",
        "08" => "Honour With ID",
        "09" => "Request in Progress",
        "10" => "Partial Approval",
        "11" => "VIP Approval",
        "12" => "Invalid Transaction",
        "13" => "Invalid Amount",
        "14" => "Invalid Card Number",
        "15" => "No Such Issuer",
        "19" => "Re-Enter Transaction",
        "20" => "Invalid Response",
        "21" => "No Action Taken",
        "25" => "Unable to Locate Record",
        "30" => "Format Error",
        "41" => "Lost Card",
        "43" => "Stolen Card",
        "51" => "Insufficient Funds",
        "52" => "No Checking Account",
        "53" => "No Savings Account",
        "54" => "Expired Card",
        "55" => "Incorrect PIN",
        "56" => "No Card Record",
        "57" => "Transaction Not Permitted to Cardholder",
        "58" => "Transaction Not Permitted to Terminal",
        "59" => "Suspected Fraud",
        "61" => "Exceeds Withdrawal Amount Limit",
        "62" => "Restricted Card",
        "63" => "Security Violation",
        "65" => "Exceeds Withdrawal Frequency Limit",
        "68" => "Response Received Too Late",
        "75" => "Allowable PIN Tries Exceeded",
        "76" => "Invalid/Non-existent Account",
        "77" => "Invalid Date",
        "78" => "Blocked, First Used",
        "79" => "Lifecycle Change",
        "80" => "Network Error",
        "85" => "No Reason to Decline",
        "91" => "Issuer or Switch Inoperative",
        "92" => "Unable to Route",
        "93" => "Cannot Complete; Violation of Law",
        "94" => "Duplicate Transmission",
        "95" => "Reconcile Error",
        "96" => "System Malfunction",
        "98" => "Exceeds Cash Limit",
        _    => "Unknown",
    };
    format!("{} - {}", s, desc)
}

fn annotate_currency_code(s: &str) -> String {
    let name = match s {
        "840" => "USD - US Dollar",
        "978" => "EUR - Euro",
        "826" => "GBP - British Pound",
        "392" => "JPY - Japanese Yen",
        "156" => "CNY - Chinese Yuan",
        "356" => "INR - Indian Rupee",
        "036" => "AUD - Australian Dollar",
        "124" => "CAD - Canadian Dollar",
        "756" => "CHF - Swiss Franc",
        "360" => "IDR - Indonesian Rupiah",
        "702" => "SGD - Singapore Dollar",
        "764" => "THB - Thai Baht",
        "458" => "MYR - Malaysian Ringgit",
        "704" => "VND - Vietnamese Dong",
        "682" => "SAR - Saudi Riyal",
        "784" => "AED - UAE Dirham",
        _     => "Unknown",
    };
    format!("{} ({})", s, name)
}

/// Format decode result as a readable string
pub fn format_result(r: &DecodeResult) -> String {
    let mut out = String::new();

    out.push_str("╔══════════════════════════════════════════════════╗\n");
    out.push_str("║           ISO 8583 DECODE RESULT                 ║\n");
    out.push_str("╚══════════════════════════════════════════════════╝\n\n");

    out.push_str(&format!("  MTI  : {} → {}\n", r.mti, r.mti_description));
    out.push_str(&format!("  P.BMP: {}\n", r.primary_bitmap));
    if let Some(ref sbm) = r.secondary_bitmap {
        out.push_str(&format!("  S.BMP: {}\n", sbm));
    }
    out.push_str("\n");

    // Bitmap visualization
    out.push_str("  Bitmap fields set:\n");
    let bmp_set: Vec<usize> = r.fields.iter().map(|f| f.number).collect();
    let bmp_line: Vec<String> = bmp_set.iter().map(|n| format!("{:03}", n)).collect();
    out.push_str(&format!("  [ {} ]\n\n", bmp_line.join(" ")));

    out.push_str("─────┬────┬──────────────────────────────────────────────────────\n");
    out.push_str(" FLD │ Tp │ Value\n");
    out.push_str("─────┼────┼──────────────────────────────────────────────────────\n");

    for field in &r.fields {
        let name_short = if field.name.len() > 30 {
            format!("{}…", &field.name[..29])
        } else {
            field.name.clone()
        };
        out.push_str(&format!(
            " {:03} │ {:2} │ {}\n     │    │ ↳ {} (len={})\n",
            field.number,
            field.data_type,
            field.value,
            name_short,
            field.length,
        ));
    }

    out.push_str("─────┴────┴──────────────────────────────────────────────────────\n");

    if !r.errors.is_empty() {
        out.push_str("\n⚠ ERRORS:\n");
        for e in &r.errors {
            out.push_str(&format!("  • {}\n", e));
        }
    }

    out
}

// ─────────────────────────────────────────────────────────────────────────────
// RAW / ASCII MODE DECODER
// Format: MTI(4 ASCII) + Bitmap(16/32 HEX chars) + Field data (raw ASCII)
// This is "semi-ASCII" encoding common in many local bank/fintech systems.
// ─────────────────────────────────────────────────────────────────────────────

/// Auto-detect whether input is fully-hex or raw-ASCII format.
/// Heuristic: if all chars are hex AND the first 4 chars decode to printable
/// ASCII digits → likely hex-encoded. Otherwise likely raw format.
pub fn detect_format(input: &str) -> &'static str {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.len() < 8 {
        return "hex";
    }
    // If all chars are hex digits, try to decode MTI as hex
    let all_hex = cleaned.chars().all(|c| c.is_ascii_hexdigit());
    if all_hex {
        // Decode first 8 hex chars as 4 bytes
        let mti_bytes: Vec<u8> = (0..8).step_by(2)
            .filter_map(|i| u8::from_str_radix(&cleaned[i..i+2], 16).ok())
            .collect();
        // If decoded bytes look like an MTI ("0xxx" or "1xxx" etc, all ASCII digits)
        let all_digits = mti_bytes.iter().all(|b| b.is_ascii_digit());
        if all_digits {
            return "hex";
        }
    }
    "raw"
}

/// Decode ISO 8583 in raw/ASCII format:
///   MTI        = 4 ASCII chars  (e.g. "0200")
///   P.Bitmap   = 16 HEX chars   (e.g. "FA3A401188810100")
///   S.Bitmap   = 16 HEX chars   (only if bit-1 of P.Bitmap is set)
///   Field data = raw ASCII, each field by its own length rule
pub fn decode_raw(input: &str) -> DecodeResult {
    // Strip only carriage returns and newlines; spaces inside hex bitmaps
    // are intentional separators some tools add — also strip those.
    let raw: String = input.chars()
        .filter(|c| *c != '\r' && *c != '\n')
        .collect();

    let chars: Vec<char> = raw.chars().collect();

    let mut result = DecodeResult {
        mti: String::new(),
        mti_description: String::new(),
        primary_bitmap: String::new(),
        secondary_bitmap: None,
        fields: Vec::new(),
        errors: Vec::new(),
    };

    if chars.len() < 20 {
        result.errors.push(format!(
            "Input terlalu pendek: {} chars (butuh min 20: 4 MTI + 16 bitmap)",
            chars.len()
        ));
        return result;
    }

    let mut pos = 0usize;

    // ── MTI (4 ASCII chars) ──
    let mti: String = chars[pos..pos + 4].iter().collect();
    pos += 4;
    result.mti = mti.clone();
    result.mti_description = describe_mti(&mti);

    // ── Primary Bitmap (16 hex chars → 8 bytes) ──
    let primary_bm_hex: String = chars[pos..pos + 16].iter().collect();
    pos += 16;
    result.primary_bitmap = primary_bm_hex.to_uppercase();

    let primary_bits = match parse_bitmap(&primary_bm_hex.to_uppercase()) {
        Ok(b) => b,
        Err(e) => {
            result.errors.push(format!("Primary bitmap error: {}", e));
            return result;
        }
    };

    let has_secondary = primary_bits[0];
    let mut all_bits = primary_bits.clone();

    if has_secondary {
        if pos + 16 > chars.len() {
            result.errors.push("Secondary bitmap expected (bit 1 set) tapi data tidak cukup".to_string());
        } else {
            let secondary_bm_hex: String = chars[pos..pos + 16].iter().collect();
            pos += 16;
            result.secondary_bitmap = Some(secondary_bm_hex.to_uppercase().clone());
            match parse_bitmap(&secondary_bm_hex.to_uppercase()) {
                Ok(sec_bits) => all_bits.extend(sec_bits),
                Err(e) => result.errors.push(format!("Secondary bitmap error: {}", e)),
            }
        }
    }

    // ── Parse Fields (raw ASCII) ──
    for bit_idx in 1..all_bits.len() {
        if !all_bits[bit_idx] { continue; }
        let field_num = bit_idx + 1;

        let def = match get_field_def(field_num) {
            Some(d) => d,
            None => {
                // Unknown field — try LLLVAR as best guess for private fields 93-127
                if field_num >= 93 && field_num <= 127 {
                    // Try to read as LLLVAR
                    if pos + 3 > chars.len() {
                        result.errors.push(format!(
                            "Field {:03}: private field, data tidak cukup untuk baca panjang", field_num
                        ));
                        break;
                    }
                    let len_str: String = chars[pos..pos + 3].iter().collect();
                    if let Ok(l) = len_str.parse::<usize>() {
                        pos += 3;
                        let val: String = chars[pos..pos.saturating_add(l).min(chars.len())].iter().collect();
                        pos += l.min(chars.len() - pos);
                        result.fields.push(ParsedField {
                            number: field_num,
                            name: format!("Private/Reserved Field {}", field_num),
                            data_type: "ANS".to_string(),
                            length: l,
                            value: val,
                        });
                        continue;
                    }
                }
                result.errors.push(format!(
                    "Field {:03}: tidak ada definisi standar — parsing berhenti di sini", field_num
                ));
                break;
            }
        };

        // Determine data length in chars
        let data_len = match def.length_type {
            LengthType::Fixed => def.max_len,
            LengthType::LLVar => {
                if pos + 2 > chars.len() {
                    result.errors.push(format!("Field {:03}: data tidak cukup untuk LL prefix", field_num));
                    break;
                }
                let len_str: String = chars[pos..pos + 2].iter().collect();
                pos += 2;
                match len_str.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        result.errors.push(format!(
                            "Field {:03}: LL prefix '{}' bukan angka", field_num, len_str
                        ));
                        break;
                    }
                }
            }
            LengthType::LLLVar => {
                if pos + 3 > chars.len() {
                    result.errors.push(format!("Field {:03}: data tidak cukup untuk LLL prefix", field_num));
                    break;
                }
                let len_str: String = chars[pos..pos + 3].iter().collect();
                pos += 3;
                match len_str.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        result.errors.push(format!(
                            "Field {:03}: LLL prefix '{}' bukan angka", field_num, len_str
                        ));
                        break;
                    }
                }
            }
            LengthType::TagLLLVar => {
                // TAG(3 chars) + LEN(3 chars) — format khusus F48 beberapa sistem bank
                if pos + 6 > chars.len() {
                    result.errors.push(format!("Field {:03}: data tidak cukup untuk Tag+LLL prefix", field_num));
                    break;
                }
                // Skip tag (3 chars), read length (3 chars)
                pos += 3; // skip tag
                let len_str: String = chars[pos..pos + 3].iter().collect();
                pos += 3;
                match len_str.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        result.errors.push(format!(
                            "Field {:03}: TagLLL prefix '{}' bukan angka", field_num, len_str
                        ));
                        break;
                    }
                }
            }
        };

        // For binary fields (like PIN block, MAC), data is still hex text
        // For everything else, read raw chars
        let (actual_chars, value) = match def.data_type {
            DataType::B => {
                let hex_chars = data_len * 2;
                if pos + hex_chars > chars.len() {
                    result.errors.push(format!("Field {:03} (Binary): data tidak cukup", field_num));
                    break;
                }
                let raw_hex: String = chars[pos..pos + hex_chars].iter().collect();
                pos += hex_chars;
                (hex_chars, format!("0x{}", raw_hex.to_uppercase()))
            }
            _ => {
                // ASCII/text field
                let avail = data_len.min(chars.len() - pos);
                let raw: String = chars[pos..pos + avail].iter().collect();
                pos += avail;
                if avail < data_len {
                    result.errors.push(format!(
                        "Field {:03}: butuh {} chars, hanya {} tersedia",
                        field_num, data_len, avail
                    ));
                }
                // Escape non-printable chars so they show up in output
                let display: String = raw.chars().map(|c| {
                    if c.is_ascii_control() || !c.is_ascii() {
                        format!("[0x{:02X}]", c as u32)
                    } else {
                        c.to_string()
                    }
                }).collect();

                // Warn if a numeric field has non-digit chars (possible BCD encoding)
                let annotated = if matches!(def.data_type, DataType::N) && raw.chars().any(|c| !c.is_ascii_digit()) {
                    format!("{} ⚠BCD?", display)
                } else {
                    match field_num {
                        3  => annotate_processing_code(&display),
                        22 => annotate_pos_entry_mode(&display),
                        39 => annotate_response_code(&display),
                        49 | 50 | 51 => annotate_currency_code(&display),
                        _ => display,
                    }
                };
                (avail, annotated)
            }
        };

        let _ = actual_chars;

        result.fields.push(ParsedField {
            number: field_num,
            name: def.name.to_string(),
            data_type: def.data_type.to_string(),
            length: data_len,
            value,
        });
    }

    result
}

/// Format decode result with mode label
pub fn format_result_with_mode(r: &DecodeResult, mode: &str) -> String {
    let mut out = String::new();

    out.push_str("╔══════════════════════════════════════════════════╗\n");
    out.push_str(&format!("║      ISO 8583 DECODE RESULT [{:<14}]   ║\n", mode));
    out.push_str("╚══════════════════════════════════════════════════╝\n\n");

    out.push_str(&format!("  MTI  : {} → {}\n", r.mti, r.mti_description));
    out.push_str(&format!("  P.BMP: {}\n", r.primary_bitmap));
    if let Some(ref sbm) = r.secondary_bitmap {
        out.push_str(&format!("  S.BMP: {}\n", sbm));
    }
    out.push_str("\n");

    let bmp_set: Vec<usize> = r.fields.iter().map(|f| f.number).collect();
    let bmp_line: Vec<String> = bmp_set.iter().map(|n| format!("{:03}", n)).collect();
    out.push_str(&format!("  Fields set: [ {} ]\n\n", bmp_line.join(" ")));

    out.push_str("─────┬────┬──────────────────────────────────────────────────────\n");
    out.push_str(" FLD │ Tp │ Value\n");
    out.push_str("─────┼────┼──────────────────────────────────────────────────────\n");

    for field in &r.fields {
        let name_short = if field.name.len() > 35 {
            format!("{}…", &field.name[..34])
        } else {
            field.name.clone()
        };
        out.push_str(&format!(
            " {:03} │ {:2} │ {}\n     │    │ ↳ {} (len={})\n",
            field.number,
            field.data_type,
            field.value,
            name_short,
            field.length,
        ));
    }

    out.push_str("─────┴────┴──────────────────────────────────────────────────────\n");

    if !r.errors.is_empty() {
        out.push_str(&format!("\n⚠ {} WARNINGS/ERRORS:\n", r.errors.len()));
        for e in &r.errors {
            out.push_str(&format!("  • {}\n", e));
        }
    }

    out
}
