/// ISO 8583 Encoder — builds a hex-encoded ISO 8583 message (ASCII encoding)
///
/// Usage:
///   let mut enc = Encoder::new("0200");
///   enc.set(2,  "4111111111111111");
///   enc.set(3,  "000000");
///   enc.set(4,  "000000012345");
///   enc.set(11, "000001");
///   enc.set(39, "00");
///   let hex = enc.encode();

use std::collections::BTreeMap;
use crate::iso8583::{get_field_def, LengthType, DataType};

pub struct Encoder {
    mti: String,
    fields: BTreeMap<usize, String>, // field_num → value string
}

#[derive(Debug)]
pub struct EncodeResult {
    pub hex: String,
    pub mti: String,
    pub primary_bitmap: String,
    pub secondary_bitmap: Option<String>,
    pub fields_encoded: Vec<(usize, String, String)>, // (num, name, value)
    pub errors: Vec<String>,
}

impl Encoder {
    pub fn new(mti: &str) -> Self {
        Self {
            mti: mti.to_string(),
            fields: BTreeMap::new(),
        }
    }

    /// Set a field value (raw ASCII string for AN/ANS/N/Z fields)
    pub fn set(&mut self, field: usize, value: &str) {
        self.fields.insert(field, value.to_string());
    }

    /// Remove a field
    pub fn remove(&mut self, field: usize) {
        self.fields.remove(&field);
    }

    /// Build the ISO 8583 hex message
    pub fn encode(&self) -> EncodeResult {
        let mut result = EncodeResult {
            hex: String::new(),
            mti: self.mti.clone(),
            primary_bitmap: String::new(),
            secondary_bitmap: None,
            fields_encoded: Vec::new(),
            errors: Vec::new(),
        };

        // Validate MTI (must be 4 ASCII chars)
        if self.mti.len() != 4 {
            result.errors.push(format!("MTI must be 4 characters, got '{}'", self.mti));
            return result;
        }

        // Determine if secondary bitmap is needed (