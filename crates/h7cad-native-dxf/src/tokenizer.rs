use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxfToken {
    pub code: GroupCode,
    pub raw_value: String,
}

impl DxfToken {
    pub fn new(code: GroupCode, raw_value: impl Into<String>) -> Self {
        Self {
            code,
            raw_value: raw_value.into(),
        }
    }

    pub fn decode(&self) -> Result<DxfValue, DxfDecodeError> {
        self.code.decode_value(&self.raw_value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupCode(i16);

impl GroupCode {
    pub fn new(value: i16) -> Result<Self, DxfParseError> {
        if value < 0 {
            return Err(DxfParseError::InvalidGroupCode(value.to_string()));
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> i16 {
        self.0
    }

    pub fn decode_value(self, raw: &str) -> Result<DxfValue, DxfDecodeError> {
        let trimmed = raw.trim();
        let kind = self.value_kind();

        match kind {
            GroupValueKind::Str => Ok(DxfValue::Str(raw.to_string())),
            GroupValueKind::Short => parse_number(trimmed, self, DxfValue::Short),
            GroupValueKind::Int => parse_number(trimmed, self, DxfValue::Int),
            GroupValueKind::Long => parse_number(trimmed, self, DxfValue::Long),
            GroupValueKind::Double => parse_number(trimmed, self, DxfValue::Double),
            GroupValueKind::Bool => match trimmed {
                "0" => Ok(DxfValue::Bool(false)),
                "1" => Ok(DxfValue::Bool(true)),
                _ => Err(DxfDecodeError::new(self, raw, "expected boolean 0 or 1")),
            },
            GroupValueKind::Binary => decode_hex(trimmed, self).map(DxfValue::Binary),
        }
    }

    pub fn value_kind(self) -> GroupValueKind {
        match self.0 {
            310..=319 | 1004 => GroupValueKind::Binary,
            0..=9
            | 100
            | 102
            | 105
            | 300..=309
            | 320..=369
            | 390..=399
            | 410..=419
            | 430..=439
            | 470..=481
            | 999
            | 1000..=1009 => GroupValueKind::Str,
            10..=59
            | 110..=149
            | 210..=239
            | 460..=469
            | 1010..=1059 => GroupValueKind::Double,
            60..=79
            | 170..=179
            | 270..=289
            | 370..=389
            | 400..=409
            | 1060..=1070 => GroupValueKind::Short,
            90..=99
            | 160..=169
            | 420..=429
            | 440..=459
            | 1071 => GroupValueKind::Long,
            290..=299 => GroupValueKind::Bool,
            _ => GroupValueKind::Int,
        }
    }
}

impl fmt::Display for GroupCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for GroupCode {
    type Err = DxfParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let value = trimmed
            .parse::<i16>()
            .map_err(|_| DxfParseError::InvalidGroupCode(trimmed.to_string()))?;
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupValueKind {
    Str,
    Short,
    Int,
    Long,
    Double,
    Bool,
    Binary,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DxfValue {
    Str(String),
    Short(i16),
    Int(i32),
    Long(i64),
    Double(f64),
    Bool(bool),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxfParseError {
    UnexpectedEndOfInput {
        expected: &'static str,
        line: usize,
    },
    InvalidGroupCode(String),
}

impl fmt::Display for DxfParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEndOfInput { expected, line } => {
                write!(f, "unexpected end of input at line {line}, expected {expected}")
            }
            Self::InvalidGroupCode(value) => write!(f, "invalid DXF group code `{value}`"),
        }
    }
}

impl std::error::Error for DxfParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxfDecodeError {
    pub code: GroupCode,
    pub raw_value: String,
    pub reason: &'static str,
}

impl DxfDecodeError {
    pub(crate) fn new(code: GroupCode, raw_value: impl Into<String>, reason: &'static str) -> Self {
        Self {
            code,
            raw_value: raw_value.into(),
            reason,
        }
    }
}

impl fmt::Display for DxfDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to decode group code {} from `{}`: {}",
            self.code, self.raw_value, self.reason
        )
    }
}

impl std::error::Error for DxfDecodeError {}

pub struct DxfTokenizer<'a> {
    lines: std::str::Lines<'a>,
    next_line: usize,
}

impl<'a> DxfTokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines(),
            next_line: 1,
        }
    }
}

impl<'a> Iterator for DxfTokenizer<'a> {
    type Item = Result<DxfToken, DxfParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        let code_line = self.lines.next()?;
        let code_line_number = self.next_line;
        self.next_line += 1;

        let value_line = match self.lines.next() {
            Some(line) => {
                self.next_line += 1;
                line
            }
            None => {
                return Some(Err(DxfParseError::UnexpectedEndOfInput {
                    expected: "value line",
                    line: code_line_number + 1,
                }))
            }
        };

        Some(
            GroupCode::from_str(code_line)
                .map(|code| DxfToken::new(code, value_line))
                .map_err(|_| DxfParseError::InvalidGroupCode(code_line.trim().to_string())),
        )
    }
}

pub fn tokenize_dxf(input: &str) -> Result<Vec<DxfToken>, DxfParseError> {
    DxfTokenizer::new(input).collect()
}

fn parse_number<T>(
    raw: &str,
    code: GroupCode,
    map: impl FnOnce(T) -> DxfValue,
) -> Result<DxfValue, DxfDecodeError>
where
    T: FromStr,
{
    raw.parse::<T>()
        .map(map)
        .map_err(|_| DxfDecodeError::new(code, raw, "invalid numeric value"))
}

// ---------------------------------------------------------------------------
// Binary DXF Tokenizer
// ---------------------------------------------------------------------------

pub const BINARY_DXF_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\x00";

pub struct BinaryDxfTokenizer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinaryDxfTokenizer<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, DxfParseError> {
        if data.len() < BINARY_DXF_SENTINEL.len()
            || &data[..BINARY_DXF_SENTINEL.len()] != BINARY_DXF_SENTINEL
        {
            return Err(DxfParseError::InvalidGroupCode(
                "missing Binary DXF sentinel".into(),
            ));
        }
        Ok(Self {
            data,
            pos: BINARY_DXF_SENTINEL.len(),
        })
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn read_u8(&mut self) -> Option<u8> {
        if self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            Some(b)
        } else {
            None
        }
    }

    fn read_i16_le(&mut self) -> Option<i16> {
        if self.remaining() < 2 {
            return None;
        }
        let val = i16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Some(val)
    }

    fn read_i32_le(&mut self) -> Option<i32> {
        if self.remaining() < 4 {
            return None;
        }
        let bytes: [u8; 4] = self.data[self.pos..self.pos + 4].try_into().ok()?;
        self.pos += 4;
        Some(i32::from_le_bytes(bytes))
    }

    fn read_i64_le(&mut self) -> Option<i64> {
        if self.remaining() < 8 {
            return None;
        }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8].try_into().ok()?;
        self.pos += 8;
        Some(i64::from_le_bytes(bytes))
    }

    fn read_f64_le(&mut self) -> Option<f64> {
        if self.remaining() < 8 {
            return None;
        }
        let bytes: [u8; 8] = self.data[self.pos..self.pos + 8].try_into().ok()?;
        self.pos += 8;
        Some(f64::from_le_bytes(bytes))
    }

    fn read_null_terminated_string(&mut self) -> Option<String> {
        let start = self.pos;
        while self.pos < self.data.len() {
            if self.data[self.pos] == 0 {
                let s = String::from_utf8_lossy(&self.data[start..self.pos]).into_owned();
                self.pos += 1;
                return Some(s);
            }
            self.pos += 1;
        }
        None
    }

    fn read_binary_chunk(&mut self) -> Option<Vec<u8>> {
        let len = self.read_u8()? as usize;
        if self.remaining() < len {
            return None;
        }
        let chunk = self.data[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Some(chunk)
    }

    fn read_group_code(&mut self) -> Option<Result<i16, DxfParseError>> {
        let first = self.read_u8()?;
        if first == 255 {
            Some(self.read_i16_le().ok_or_else(|| {
                DxfParseError::UnexpectedEndOfInput {
                    expected: "extended group code",
                    line: self.pos,
                }
            }))
        } else {
            Some(Ok(first as i16))
        }
    }

    fn read_value_for_code(&mut self, code: i16) -> Option<String> {
        let gc = GroupCode(code);
        match gc.value_kind() {
            GroupValueKind::Str => self.read_null_terminated_string(),
            GroupValueKind::Double => self.read_f64_le().map(|v| format!("{v}")),
            GroupValueKind::Short => self.read_i16_le().map(|v| format!("{v}")),
            GroupValueKind::Long | GroupValueKind::Int => self.read_i32_le().map(|v| format!("{v}")),
            GroupValueKind::Bool => self.read_u8().map(|v| format!("{v}")),
            GroupValueKind::Binary => {
                self.read_binary_chunk().map(|bytes| {
                    bytes.iter().map(|b| format!("{b:02X}")).collect::<String>()
                })
            }
        }
    }
}

impl<'a> Iterator for BinaryDxfTokenizer<'a> {
    type Item = Result<DxfToken, DxfParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining() == 0 {
            return None;
        }

        let code_result = self.read_group_code()?;
        let code = match code_result {
            Ok(c) => c,
            Err(e) => return Some(Err(e)),
        };

        let gc = match GroupCode::new(code) {
            Ok(gc) => gc,
            Err(e) => return Some(Err(e)),
        };

        let value = match self.read_value_for_code(code) {
            Some(v) => v,
            None => {
                return Some(Err(DxfParseError::UnexpectedEndOfInput {
                    expected: "value for group code",
                    line: self.pos,
                }))
            }
        };

        Some(Ok(DxfToken::new(gc, value)))
    }
}

pub fn is_binary_dxf(data: &[u8]) -> bool {
    data.len() >= BINARY_DXF_SENTINEL.len()
        && &data[..BINARY_DXF_SENTINEL.len()] == BINARY_DXF_SENTINEL
}

fn decode_hex(raw: &str, code: GroupCode) -> Result<Vec<u8>, DxfDecodeError> {
    if raw.len() % 2 != 0 {
        return Err(DxfDecodeError::new(
            code,
            raw,
            "binary data must have an even number of hex digits",
        ));
    }

    let mut bytes = Vec::with_capacity(raw.len() / 2);
    let mut chars = raw.as_bytes().chunks_exact(2);
    for pair in &mut chars {
        let byte = std::str::from_utf8(pair)
            .ok()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            .ok_or_else(|| DxfDecodeError::new(code, raw, "invalid hexadecimal binary data"))?;
        bytes.push(byte);
    }
    Ok(bytes)
}
