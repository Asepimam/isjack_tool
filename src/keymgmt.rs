// ─── Key Management Tools ─────────────────────────────────────────────────────
// 3DES (EDE), PIN Block ISO-0 / ISO-3, KCV, XOR utility

// ─────────────────────────────────────────────────────────────────────────────
// DES Core Implementation
// ─────────────────────────────────────────────────────────────────────────────

static IP: [u8; 64] = [
    58,50,42,34,26,18,10,2, 60,52,44,36,28,20,12,4,
    62,54,46,38,30,22,14,6, 64,56,48,40,32,24,16,8,
    57,49,41,33,25,17, 9,1, 59,51,43,35,27,19,11,3,
    61,53,45,37,29,21,13,5, 63,55,47,39,31,23,15,7,
];
static IP_INV: [u8; 64] = [
    40,8,48,16,56,24,64,32, 39,7,47,15,55,23,63,31,
    38,6,46,14,54,22,62,30, 37,5,45,13,53,21,61,29,
    36,4,44,12,52,20,60,28, 35,3,43,11,51,19,59,27,
    34,2,42,10,50,18,58,26, 33,1,41, 9,49,17,57,25,
];
static E: [u8; 48] = [
    32,1,2,3,4,5, 4,5,6,7,8,9, 8,9,10,11,12,13,
    12,13,14,15,16,17, 16,17,18,19,20,21, 20,21,22,23,24,25,
    24,25,26,27,28,29, 28,29,30,31,32,1,
];
static P: [u8; 32] = [
    16,7,20,21,29,12,28,17, 1,15,23,26,5,18,31,10,
    2,8,24,14,32,27,3,9, 19,13,30,6,22,11,4,25,
];
static PC1: [u8; 56] = [
    57,49,41,33,25,17,9, 1,58,50,42,34,26,18,
    10,2,59,51,43,35,27, 19,11,3,60,52,44,36,
    63,55,47,39,31,23,15, 7,62,54,46,38,30,22,
    14,6,61,53,45,37,29, 21,13,5,28,20,12,4,
];
static PC2: [u8; 48] = [
    14,17,11,24,1,5, 3,28,15,6,21,10, 23,19,12,4,26,8,
    16,7,27,20,13,2, 41,52,31,37,47,55, 30,40,51,45,33,48,
    44,49,39,56,34,53, 46,42,50,36,29,32,
];
static SHIFTS: [u8; 16] = [1,1,2,2,2,2,2,2,1,2,2,2,2,2,2,1];
static SBOXES: [[u8; 64]; 8] = [
    [14,4,13,1,2,15,11,8,3,10,6,12,5,9,0,7,0,15,7,4,14,2,13,1,10,6,12,11,9,5,3,8,4,1,14,8,13,6,2,11,15,12,9,7,3,10,5,0,15,12,8,2,4,9,1,7,5,11,3,14,10,0,6,13],
    [15,1,8,14,6,11,3,4,9,7,2,13,12,0,5,10,3,13,4,7,15,2,8,14,12,0,1,10,6,9,11,5,0,14,7,11,10,4,13,1,5,8,12,6,9,3,2,15,13,8,10,1,3,15,4,2,11,6,7,12,0,5,14,9],
    [10,0,9,14,6,3,15,5,1,13,12,7,11,4,2,8,13,7,0,9,3,4,6,10,2,8,5,14,12,11,15,1,13,6,4,9,8,15,3,0,11,1,2,12,5,10,14,7,1,10,13,0,6,9,8,7,4,15,14,3,11,5,2,12],
    [7,13,14,3,0,6,9,10,1,2,8,5,11,12,4,15,13,8,11,5,6,15,0,3,4,7,2,12,1,10,14,9,10,6,9,0,12,11,7,13,15,1,3,14,5,2,8,4,3,15,0,6,10,1,13,8,9,4,5,11,12,7,2,14],
    [2,12,4,1,7,10,11,6,8,5,3,15,13,0,14,9,14,11,2,12,4,7,13,1,5,0,15,10,3,9,8,6,4,2,1,11,10,13,7,8,15,9,12,5,6,3,0,14,11,8,12,7,1,14,2,13,6,15,0,9,10,4,5,3],
    [12,1,10,15,9,2,6,8,0,13,3,4,14,7,5,11,10,15,4,2,7,12,9,5,6,1,13,14,0,11,3,8,9,14,15,5,2,8,12,3,7,0,4,10,1,13,11,6,4,3,2,12,9,5,15,10,11,14,1,7,6,0,8,13],
    [4,11,2,14,15,0,8,13,3,12,9,7,5,10,6,1,13,0,11,7,4,9,1,10,14,3,5,12,2,15,8,6,1,4,11,13,12,3,7,14,10,15,6,8,0,5,9,2,6,11,13,8,1,4,10,7,9,5,0,15,14,2,3,12],
    [13,2,8,4,6,15,11,1,10,9,3,14,5,0,12,7,1,15,13,8,10,3,7,4,12,5,6,11,0,14,9,2,7,11,4,1,9,12,14,2,0,6,10,13,15,3,5,8,2,1,14,7,4,10,8,13,15,12,9,0,3,5,6,11],
];

fn permute(input: u64, table: &[u8], in_bits: u8, out_bits: u8) -> u64 {
    let mut out = 0u64;
    for (i, &p) in table.iter().enumerate() {
        let bit = (input >> (in_bits - p)) & 1;
        out |= bit << (out_bits as usize - 1 - i);
    }
    out
}

fn des_subkeys(key_bytes: &[u8; 8]) -> [[u8; 6]; 16] {
    let key = u64::from_be_bytes(*key_bytes);
    let cd = permute(key, &PC1, 64, 56);
    let mut c = ((cd >> 28) & 0x0FFFFFFF) as u32;
    let mut d = (cd & 0x0FFFFFFF) as u32;
    let mut subkeys = [[0u8; 6]; 16];
    for round in 0..16 {
        let s = SHIFTS[round] as usize;
        c = ((c << s) | (c >> (28 - s))) & 0x0FFFFFFF;
        d = ((d << s) | (d >> (28 - s))) & 0x0FFFFFFF;
        let cd56 = ((c as u64) << 28) | (d as u64);
        let sk = permute(cd56, &PC2, 56, 48);
        subkeys[round] = sk.to_be_bytes()[2..].try_into().unwrap();
    }
    subkeys
}

fn des_f(r: u32, subkey: &[u8; 6]) -> u32 {
    let r64 = r as u64;
    let er = permute(r64, &E, 32, 48);
    let sk = u64::from_be_bytes([0, 0, subkey[0], subkey[1], subkey[2], subkey[3], subkey[4], subkey[5]]);
    let xored = er ^ sk;
    let mut sout = 0u32;
    for i in 0..8 {
        let six = ((xored >> (42 - i * 6)) & 0x3F) as usize;
        let row = ((six >> 4) & 2) | (six & 1);
        let col = (six >> 1) & 0xF;
        let s = SBOXES[i][row * 16 + col];
        sout = (sout << 4) | s as u32;
    }
    permute(sout as u64, &P, 32, 32) as u32
}

fn des_block(block: &[u8; 8], subkeys: &[[u8; 6]; 16]) -> [u8; 8] {
    let input = u64::from_be_bytes(*block);
    let ip = permute(input, &IP, 64, 64);
    let mut l = (ip >> 32) as u32;
    let mut r = ip as u32;
    for sk in subkeys.iter() {
        let new_r = l ^ des_f(r, sk);
        l = r;
        r = new_r;
    }
    let pre = ((r as u64) << 32) | (l as u64);
    let out = permute(pre, &IP_INV, 64, 64);
    out.to_be_bytes()
}

pub fn des_encrypt(block: &[u8; 8], key: &[u8; 8]) -> [u8; 8] {
    let sk = des_subkeys(key);
    des_block(block, &sk)
}

pub fn des_decrypt(block: &[u8; 8], key: &[u8; 8]) -> [u8; 8] {
    let sk = des_subkeys(key);
    let mut rev = [[0u8; 6]; 16];
    for i in 0..16 { rev[i] = sk[15 - i]; }
    des_block(block, &rev)
}

/// 3DES ECB — encrypt  (EDE: enc K1, dec K2, enc K3)
pub fn tdes_encrypt(block: &[u8; 8], key: &[u8; 16]) -> [u8; 8] {
    let k1: &[u8; 8] = key[0..8].try_into().unwrap();
    let k2: &[u8; 8] = key[8..16].try_into().unwrap();
    let t1 = des_encrypt(block, k1);
    let t2 = des_decrypt(&t1, k2);
    des_encrypt(&t2, k1)  // 2-key 3DES (K1=K3)
}

pub fn tdes_encrypt_3key(block: &[u8; 8], key: &[u8; 24]) -> [u8; 8] {
    let k1: &[u8; 8] = key[0..8].try_into().unwrap();
    let k2: &[u8; 8] = key[8..16].try_into().unwrap();
    let k3: &[u8; 8] = key[16..24].try_into().unwrap();
    let t1 = des_encrypt(block, k1);
    let t2 = des_decrypt(&t1, k2);
    des_encrypt(&t2, k3)
}

pub fn tdes_decrypt(block: &[u8; 8], key: &[u8; 16]) -> [u8; 8] {
    let k1: &[u8; 8] = key[0..8].try_into().unwrap();
    let k2: &[u8; 8] = key[8..16].try_into().unwrap();
    let t1 = des_decrypt(block, k1);
    let t2 = des_encrypt(&t1, k2);
    des_decrypt(&t2, k1)
}

pub fn tdes_decrypt_3key(block: &[u8; 8], key: &[u8; 24]) -> [u8; 8] {
    let k1: &[u8; 8] = key[0..8].try_into().unwrap();
    let k2: &[u8; 8] = key[8..16].try_into().unwrap();
    let k3: &[u8; 8] = key[16..24].try_into().unwrap();
    let t1 = des_decrypt(block, k3);
    let t2 = des_encrypt(&t1, k2);
    des_decrypt(&t2, k1)
}

// ─────────────────────────────────────────────────────────────────────────────
// KCV (Key Check Value) — encrypt 8 zero bytes, take first 3 bytes
// ─────────────────────────────────────────────────────────────────────────────

pub fn kcv(key_hex: &str) -> Result<String, String> {
    let key_bytes = parse_hex(key_hex)?;
    let zero_block = [0u8; 8];
    let result = match key_bytes.len() {
        8  => des_encrypt(&zero_block, key_bytes[0..8].try_into().unwrap()),
        16 => tdes_encrypt(&zero_block, key_bytes[0..16].try_into().unwrap()),
        24 => tdes_encrypt_3key(&zero_block, key_bytes[0..24].try_into().unwrap()),
        n  => return Err(format!("Key must be 8/16/24 bytes, got {}", n)),
    };
    Ok(to_hex(&result[0..3]))
}

// ─────────────────────────────────────────────────────────────────────────────
// PIN Block  (ISO 9564 Format 0 and Format 3)
// ─────────────────────────────────────────────────────────────────────────────

/// Build ISO Format 0 PIN block: XOR(PIN_field, PAN_field)
/// Returns 8-byte hex PIN block
pub fn build_pin_block_iso0(pin: &str, pan: &str) -> Result<String, String> {
    // Validate
    if pin.len() < 4 || pin.len() > 12 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("PIN must be 4-12 digits, got '{}'", pin));
    }
    // PIN field: 0 + len(1) + PIN + pad with F
    let pin_hex = format!("0{}{:F<14}", pin.len(), pin);
    // PAN field: 0000 + rightmost 12 digits of PAN (excluding check digit)
    let pan_digits: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
    if pan_digits.len() < 13 {
        return Err(format!("PAN too short: {} digits", pan_digits.len()));
    }
    // Take digits from pos [len-13] to [len-1] (excl. check digit)
    let pan_body: &str = &pan_digits[pan_digits.len()-13..pan_digits.len()-1];
    let pan_field = format!("0000{}", pan_body);

    // XOR
    let pin_b = parse_hex(&pin_hex)?;
    let pan_b = parse_hex(&pan_field)?;
    if pin_b.len() != 8 || pan_b.len() != 8 {
        return Err("Internal length error".to_string());
    }
    let block: Vec<u8> = pin_b.iter().zip(pan_b.iter()).map(|(a,b)| a^b).collect();
    Ok(to_hex(&block))
}

/// Encrypt a PIN block with a PIN Encryption Key (ZPK) using 3DES
pub fn encrypt_pin_block(pin_block_hex: &str, zpk_hex: &str) -> Result<String, String> {
    let pb = parse_hex(pin_block_hex)?;
    let zk = parse_hex(zpk_hex)?;
    if pb.len() != 8 { return Err("PIN block must be 8 bytes".to_string()); }
    let block: &[u8; 8] = pb[0..8].try_into().unwrap();
    let encrypted = match zk.len() {
        8  => des_encrypt(block, zk[0..8].try_into().unwrap()),
        16 => tdes_encrypt(block, zk[0..16].try_into().unwrap()),
        24 => tdes_encrypt_3key(block, zk[0..24].try_into().unwrap()),
        n  => return Err(format!("ZPK must be 8/16/24 bytes, got {}", n)),
    };
    Ok(to_hex(&encrypted))
}

/// Decrypt a PIN block with a ZPK and extract the PIN
pub fn decrypt_pin_block(encrypted_hex: &str, zpk_hex: &str, pan: &str) -> Result<String, String> {
    let eb = parse_hex(encrypted_hex)?;
    let zk = parse_hex(zpk_hex)?;
    if eb.len() != 8 { return Err("Encrypted block must be 8 bytes".to_string()); }
    let block: &[u8; 8] = eb[0..8].try_into().unwrap();
    let decrypted = match zk.len() {
        8  => des_decrypt(block, zk[0..8].try_into().unwrap()),
        16 => tdes_decrypt(block, zk[0..16].try_into().unwrap()),
        24 => tdes_decrypt_3key(block, zk[0..24].try_into().unwrap()),
        n  => return Err(format!("ZPK must be 8/16/24 bytes, got {}", n)),
    };

    // Reverse XOR with PAN field to get PIN field
    let pan_digits: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
    if pan_digits.len() < 13 {
        return Err("PAN too short".to_string());
    }
    let pan_body = &pan_digits[pan_digits.len()-13..pan_digits.len()-1];
    let pan_field_hex = format!("0000{}", pan_body);
    let pan_b = parse_hex(&pan_field_hex)?;
    let pin_field: Vec<u8> = decrypted.iter().zip(pan_b.iter()).map(|(a,b)| a^b).collect();

    // Parse PIN field: 0 + len(1 nibble) + PIN + F padding
    let pf_hex = to_hex(&pin_field);
    let format_nibble = u8::from_str_radix(&pf_hex[0..1], 16).unwrap_or(0);
    if format_nibble != 0 {
        return Err(format!("Not ISO-0 format (first nibble={})", format_nibble));
    }
    let pin_len = usize::from_str_radix(&pf_hex[1..2], 16).unwrap_or(0);
    if pin_len < 4 || pin_len > 12 {
        return Err(format!("Invalid PIN length nibble: {}", pin_len));
    }
    let pin_digits = &pf_hex[2..2+pin_len];
    Ok(pin_digits.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// XOR Utility
// ─────────────────────────────────────────────────────────────────────────────

pub fn xor_hex(a: &str, b: &str) -> Result<String, String> {
    let ab = parse_hex(a)?;
    let bb = parse_hex(b)?;
    if ab.len() != bb.len() {
        return Err(format!("Length mismatch: {} vs {} bytes", ab.len(), bb.len()));
    }
    Ok(to_hex(&ab.iter().zip(bb.iter()).map(|(x,y)| x^y).collect::<Vec<_>>()))
}

// ─────────────────────────────────────────────────────────────────────────────
// 3DES ECB Encrypt/Decrypt (direct hex input)
// ─────────────────────────────────────────────────────────────────────────────

pub fn tdes_ecb_encrypt_hex(data_hex: &str, key_hex: &str) -> Result<String, String> {
    let data = parse_hex(data_hex)?;
    let key  = parse_hex(key_hex)?;
    if data.len() % 8 != 0 {
        return Err(format!("Data length must be multiple of 8 bytes, got {}", data.len()));
    }
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(8) {
        let block: &[u8; 8] = chunk.try_into().unwrap();
        let enc = match key.len() {
            16 => tdes_encrypt(block, key[0..16].try_into().unwrap()),
            24 => tdes_encrypt_3key(block, key[0..24].try_into().unwrap()),
            8  => des_encrypt(block, key[0..8].try_into().unwrap()),
            n  => return Err(format!("Key must be 8/16/24 bytes, got {}", n)),
        };
        out.extend_from_slice(&enc);
    }
    Ok(to_hex(&out))
}

pub fn tdes_ecb_decrypt_hex(data_hex: &str, key_hex: &str) -> Result<String, String> {
    let data = parse_hex(data_hex)?;
    let key  = parse_hex(key_hex)?;
    if data.len() % 8 != 0 {
        return Err(format!("Data length must be multiple of 8 bytes, got {}", data.len()));
    }
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(8) {
        let block: &[u8; 8] = chunk.try_into().unwrap();
        let dec = match key.len() {
            16 => tdes_decrypt(block, key[0..16].try_into().unwrap()),
            24 => tdes_decrypt_3key(block, key[0..24].try_into().unwrap()),
            8  => des_decrypt(block, key[0..8].try_into().unwrap()),
            n  => return Err(format!("Key must be 8/16/24 bytes, got {}", n)),
        };
        out.extend_from_slice(&dec);
    }
    Ok(to_hex(&out))
}

// ─────────────────────────────────────────────────────────────────────────────
// Luhn Validator + BIN Info
// ─────────────────────────────────────────────────────────────────────────────

pub fn luhn_check(pan: &str) -> bool {
    let digits: Vec<u32> = pan.chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| c.to_digit(10).unwrap())
        .collect();
    if digits.len() < 13 { return false; }
    let sum: u32 = digits.iter().rev().enumerate().map(|(i, &d)| {
        if i % 2 == 1 { let v = d * 2; if v > 9 { v - 9 } else { v } } else { d }
    }).sum();
    sum % 10 == 0
}

pub fn bin_info(pan: &str) -> String {
    let digits: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 6 { return "PAN terlalu pendek".to_string(); }
    let bin6: u32 = digits[0..6].parse().unwrap_or(0);

    let (scheme, issuer, card_type) = match bin6 {
        400000..=499999 => ("VISA",       detect_visa_issuer(bin6),    "Credit/Debit"),
        510000..=559999 => ("MASTERCARD", detect_mc_issuer(bin6),      "Credit/Debit"),
        560000..=569999 => ("MAESTRO",    "Maestro",                   "Debit"),
        370000..=379999 => ("AMEX",       "American Express",          "Credit"),
        340000..=349999 => ("AMEX",       "American Express",          "Credit"),
        601100..=601199 => ("DISCOVER",   "Discover",                  "Credit"),
        353000..=358999 => ("JCB",        "JCB",                       "Credit/Debit"),
        622126..=622925 => ("UNIONPAY",   "UnionPay",                  "Credit/Debit"),
        _               => ("UNKNOWN",    "Unknown Issuer",            "Unknown"),
    };

    let masked = mask_pan(&digits);
    let valid  = if luhn_check(&digits) { "✓ Valid Luhn" } else { "✗ Invalid Luhn" };
    format!(
        "PAN    : {}\nScheme : {}  Type: {}\nIssuer : {}\nLuhn   : {}\nLength : {} digits",
        masked, scheme, card_type, issuer, valid, digits.len()
    )
}

fn detect_visa_issuer(bin: u32) -> &'static str {
    match bin {
        402690..=402699 => "BCA (Bank Central Asia)",
        410505..=410506 => "BNI (Bank Negara Indonesia)",
        441776..=441777 => "Mandiri",
        421539..=421539 => "BRI (Bank Rakyat Indonesia)",
        426220..=426229 => "CIMB Niaga",
        _               => "Visa Issuer",
    }
}

fn detect_mc_issuer(bin: u32) -> &'static str {
    match bin {
        521076..=521076 => "BCA (Bank Central Asia)",
        546000..=546009 => "BNI (Bank Negara Indonesia)",
        556617..=556617 => "Mandiri",
        _               => "Mastercard Issuer",
    }
}

pub fn mask_pan(pan: &str) -> String {
    let digits: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 13 { return pan.to_string(); }
    let show_start = 6;
    let show_end   = 4;
    let mask_len   = digits.len() - show_start - show_end;
    format!("{}{}{}", &digits[..show_start], "*".repeat(mask_len), &digits[digits.len()-show_end..])
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).map(|c| c.to_ascii_uppercase()).collect();
    if clean.len() % 2 != 0 {
        return Err(format!("Odd hex length: {}", clean.len()));
    }
    (0..clean.len()).step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i+2], 16).map_err(|_| format!("Invalid hex at {}: '{}'", i, &clean[i..i+2])))
        .collect()
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02X}", b)).collect()
}
