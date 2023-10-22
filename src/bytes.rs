use std::fmt::{Display, Formatter};
use std::num::ParseFloatError;

// ICE Sizes, kibis of bits
const BYTE: usize = 1;
const KIBYTE: usize = 1 << 10;
const MIBYTE: usize = 1 << (2 * 10);
const GIBYTE: usize = 1 << (3 * 10);
const TIBYTE: usize = 1 << (4 * 10);
const PIBYTE: usize = 1 << (5 * 10);
const EIBYTE: usize = 1 << (6 * 10);

// SI Sizes
const IBYTE: usize = 1;
const KBYTE: usize = IBYTE * 1000;
const MBYTE: usize = KBYTE * 1000;
const GBYTE: usize = MBYTE * 1000;
const TBYTE: usize = GBYTE * 1000;
const PBYTE: usize = TBYTE * 1000;
const EBYTE: usize = PBYTE * 1000;

#[derive(Debug)]
pub enum ParseError {
    ParseFloat { source: ParseFloatError },
    UnknownUnit { unit: String },
    TooLarge { input: String },
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::ParseFloat { source: err } => {
                write!(f, "parse float part failed, {}", err)
            }
            ParseError::UnknownUnit { unit: u } => write!(f, "unknown unit \"{}\"", u),
            ParseError::TooLarge { input: i } => write!(f, "too large \"{}\"", i),
        }
    }
}

impl From<ParseFloatError> for ParseError {
    fn from(err: ParseFloatError) -> Self {
        Self::ParseFloat { source: err }
    }
}

/// bytes produces a human readable representation of an SI size
///
/// See also: `parse_bytes`
///
/// bytes(82854982) -> 83 MB
#[must_use]
pub fn bytes(s: usize) -> String {
    humanate_bytes(s, 1000.0, ["B", "kB", "MB", "GB", "TB", "PB", "EB"])
}

/// ibytes produces a human readable representation of an IEC size.
///
/// ibytes((82854982) -> 79 MiB
#[must_use]
pub fn ibytes(s: usize) -> String {
    humanate_bytes(s, 1024.0, ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"])
}

/// `parse_bytes` parses a string representation of bytes into the number of bytes it represents
///
/// parse_bytes("42 MB") -> Ok(42000000)
/// parse_bytes("42 mib") -> Ok(44040192)
///
/// # Errors
///
/// Return `ParseError` if the input is not valid.
pub fn parse_bytes(s: &str) -> Result<usize, ParseError> {
    let mut last_digit = 0;
    let mut has_comma = false;

    for c in s.chars() {
        if !(c.is_ascii_digit() || c == '.' || c == ',') {
            break;
        }

        if c == ',' {
            has_comma = true;
        }

        last_digit += 1;
    }

    let num = &s[..last_digit];
    let mut tn = num.to_string();
    if has_comma {
        tn = num.replace(',', "");
    }

    let f = tn.parse::<f64>()?;
    let extra = &s[last_digit..];
    let extra = extra.trim().to_lowercase();

    let m = match extra.as_str() {
        "b" | "" => BYTE,
        "kib" | "ki" => KIBYTE,
        "kb" | "k" => KBYTE,
        "mib" | "mi" => MIBYTE,
        "mb" | "m" => MBYTE,
        "gib" | "gi" => GIBYTE,
        "gb" | "g" => GBYTE,
        "tib" | "ti" => TIBYTE,
        "tb" | "t" => TBYTE,
        "pib" | "pi" => PIBYTE,
        "pb" | "p" => PBYTE,
        "eib" | "ei" => EIBYTE,
        "eb" | "e" => EBYTE,
        _ => {
            return Err(ParseError::UnknownUnit {
                unit: extra.clone(),
            });
        }
    };

    Ok((f * m as f64) as usize)
}

#[inline]
fn logn(n: f64, b: f64) -> f64 {
    n.log2() / b.log2()
}

fn humanate_bytes(s: usize, base: f64, sizes: [&str; 7]) -> String {
    if s < 10 {
        return format!("{}B", s);
    }

    let e = logn(s as f64, base).floor();
    let suffix = sizes[e as usize];
    let val = s as f64 / base.powf(e) * 10.0 + 0.5;
    let val = val.floor() / 10.0;

    format!("{}{}", val, suffix)
}

#[cfg(feature = "serde")]
pub mod serde {
    use std::borrow::Cow;

    use super::{ibytes, parse_bytes};
    use serde::{Deserializer, Serializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<usize, D::Error> {
        let s: Cow<str> = serde::__private::de::borrow_cow_str(deserializer)?;
        parse_bytes(&s).map_err(serde::de::Error::custom)
    }

    pub fn serialize<S: Serializer>(u: &usize, s: S) -> Result<S::Ok, S::Error> {
        let b = ibytes(*u);
        s.serialize_str(&b)
    }
}

#[cfg(feature = "serde")]
pub mod serde_option {
    use super::{bytes, parse_bytes};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<usize>, D::Error> {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            None => Ok(None),
            Some(s) => {
                let size = parse_bytes(&s).map_err(serde::de::Error::custom)?;
                Ok(Some(size))
            }
        }
    }

    pub fn serialize<S: Serializer>(u: &Option<usize>, s: S) -> Result<S::Ok, S::Error> {
        match u {
            Some(v) => s.serialize_str(bytes(*v).as_str()),
            None => s.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bytes() {
        let tests = [
            ("42", 42),
            ("42MB", 42000000),
            ("42MiB", 44040192),
            ("42mb", 42000000),
            ("42mib", 44040192),
            ("42MIB", 44040192),
            ("42 MB", 42000000),
            ("42 MiB", 44040192),
            ("42 mb", 42000000),
            ("42 mib", 44040192),
            ("42 MIB", 44040192),
            ("42.5MB", 42500000),
            ("42.5MiB", 44564480),
            ("42.5 MB", 42500000),
            ("42.5 MiB", 44564480),
            // No need to say B
            ("42M", 42000000),
            ("42Mi", 44040192),
            ("42m", 42000000),
            ("42mi", 44040192),
            ("42MI", 44040192),
            ("42 M", 42000000),
            ("42 Mi", 44040192),
            ("42 m", 42000000),
            ("42 mi", 44040192),
            ("42 MI", 44040192),
            ("42.5M", 42500000),
            ("42.5Mi", 44564480),
            ("42.5 M", 42500000),
            ("42.5 Mi", 44564480),
            // Bug #42
            ("1,005.03 MB", 1005030000),
            // Large testing, breaks when too much larger than
            // this.
            ("12.5 EB", (12.5 * EBYTE as f64) as usize),
            ("12.5 E", (12.5 * EBYTE as f64) as usize),
            ("12.5 EiB", (12.5 * EIBYTE as f64) as usize),
        ];

        for (input, want) in tests {
            let value = parse_bytes(input).unwrap();
            assert_eq!(value, want as usize, "input: {}", input);
        }
    }

    #[test]
    fn test_bytes() {
        let tests = [
            ("bytes(0)", bytes(0), "0B"),
            ("bytes(1)", bytes(1), "1B"),
            ("bytes(803)", bytes(803), "803B"),
            ("bytes(999)", bytes(999), "999B"),
            ("bytes(1024)", bytes(1024), "1kB"),
            ("bytes(9999)", bytes(9999), "10kB"),
            ("bytes(1MB - 1)", bytes(MBYTE - BYTE), "1000kB"),
            ("bytes(1MB)", bytes(1024 * 1024), "1MB"),
            ("bytes(1GB - 1K)", bytes(GBYTE - KBYTE), "1000MB"),
            ("bytes(1GB)", bytes(GBYTE), "1GB"),
            ("bytes(1TB - 1M)", bytes(TBYTE - MBYTE), "1000GB"),
            ("bytes(10MB)", bytes(9999 * 1000), "10MB"),
            ("bytes(1TB)", bytes(TBYTE), "1TB"),
            ("bytes(1PB - 1T)", bytes(PBYTE - TBYTE), "999TB"),
            ("bytes(1PB)", bytes(PBYTE), "1PB"),
            ("bytes(1PB - 1T)", bytes(EBYTE - PBYTE), "999PB"),
            ("bytes(1EB)", bytes(EBYTE), "1EB"),
            // Overflows.
            // ("bytes(1EB - 1P)", Bytes((KBYTE*EBYTE)-PBYTE), "1023EB"),
            ("bytes(0)", ibytes(0), "0B"),
            ("bytes(1)", ibytes(1), "1B"),
            ("bytes(803)", ibytes(803), "803B"),
            ("bytes(1023)", ibytes(1023), "1023B"),
            ("bytes(1024)", ibytes(1024), "1KiB"),
            ("bytes(1MB - 1)", ibytes(MIBYTE - IBYTE), "1024KiB"),
            ("bytes(1MB)", ibytes(1024 * 1024), "1MiB"),
            ("bytes(1GB - 1K)", ibytes(GIBYTE - KIBYTE), "1024MiB"),
            ("bytes(1GB)", ibytes(GIBYTE), "1GiB"),
            ("bytes(1TB - 1M)", ibytes(TIBYTE - MIBYTE), "1024GiB"),
            ("bytes(1TB)", ibytes(TIBYTE), "1TiB"),
            ("bytes(1PB - 1T)", ibytes(PIBYTE - TIBYTE), "1023TiB"),
            ("bytes(1PB)", ibytes(PIBYTE), "1PiB"),
            ("bytes(1PB - 1T)", ibytes(EIBYTE - PIBYTE), "1023PiB"),
            ("bytes(1EiB)", ibytes(EIBYTE), "1EiB"),
            // Overflows.
            // ("bytes(1EB - 1P)", ibytes(((KIBYTE*EIBYTE)-PIBYTE), "1023EB"),
            (
                "bytes(5.5GiB)",
                ibytes((5.5 * GIBYTE as f64) as usize),
                "5.5GiB",
            ),
            (
                "bytes(5.5GB)",
                bytes((5.5 * GBYTE as f64) as usize),
                "5.5GB",
            ),
        ];

        for (name, got, want) in tests {
            assert_eq!(got, want, "want in {name:?}, got {got}, want {want}");
        }
    }
}
