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
