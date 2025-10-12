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
pub enum Error<'a> {
    ParseFloat(ParseFloatError),
    UnknownUnit { unit: &'a str },
    TooLarge { input: &'a str },
}

impl<'a> std::error::Error for Error<'a> {}

impl<'a> Display for Error<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ParseFloat(err) => {
                write!(f, "parse float part failed, {}", err)
            }
            Error::UnknownUnit { unit } => write!(f, "unknown unit \"{}\"", unit),
            Error::TooLarge { input } => write!(f, "too large \"{}\"", input),
        }
    }
}

impl<'a> From<ParseFloatError> for Error<'a> {
    fn from(err: ParseFloatError) -> Self {
        Self::ParseFloat(err)
    }
}

/// bytes produces a human-readable representation of an SI size
///
/// See also: `parse_bytes`
///
/// bytes(82854982) -> 83 MB
#[must_use]
pub fn bytes(s: usize) -> String {
    humanate_bytes(s, 1000.0, ["B", "kB", "MB", "GB", "TB", "PB", "EB"])
}

/// ibytes produces a human-readable representation of an IEC size.
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
/// Return `Error` if the input is not valid.
pub fn parse_bytes(input: &str) -> Result<usize, Error<'_>> {
    let mut last_digit = 0;

    for ch in input.chars() {
        if !(ch.is_ascii_digit() || ch == '.') {
            break;
        }

        last_digit += 1;
    }

    let flt = &input[..last_digit].parse::<f64>()?;
    let unit = input[last_digit..].trim();

    let scale = match unit.len() {
        0 => BYTE,
        1 => calculate_scale(unit, 1000, &["b", "k", "m", "g", "t", "p", "e"])
            .ok_or(Error::UnknownUnit { unit })?,
        2 => calculate_scale(unit, 1000, &["", "kb", "mb", "gb", "tb", "pb", "eb"])
            .or_else(|| calculate_scale(unit, 1024, &["", "ki", "mi", "gi", "ti", "pi", "ei"]))
            .ok_or(Error::UnknownUnit { unit })?,
        3 => calculate_scale(unit, 1024, &["", "kib", "mib", "gib", "tib", "pib", "eib"])
            .ok_or(Error::UnknownUnit { unit })?,
        _ => return Err(Error::UnknownUnit { unit }),
    };

    Ok((flt * scale as f64) as usize)
}

fn calculate_scale(input: &str, base: usize, units: &[&str]) -> Option<usize> {
    units.iter().enumerate().find_map(|(index, unit)| {
        if input.eq_ignore_ascii_case(unit) {
            Some(base.pow(index as u32))
        } else {
            None
        }
    })
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
    use super::{ibytes, parse_bytes};
    use serde_core::{Deserialize, Deserializer, Serializer, de};

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<usize, D::Error> {
        let s: &str = Deserialize::deserialize(deserializer)?;
        parse_bytes(s.as_ref()).map_err(de::Error::custom)
    }

    pub fn serialize<S: Serializer>(u: &usize, s: S) -> Result<S::Ok, S::Error> {
        let b = ibytes(*u);
        s.serialize_str(&b)
    }
}

#[cfg(feature = "serde")]
pub mod serde_option {
    use super::{ibytes, parse_bytes};
    use serde_core::{Deserialize, Deserializer, Serializer, de};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<usize>, D::Error> {
        let s: Option<&str> = Option::deserialize(deserializer)?;
        match s {
            None => Ok(None),
            Some(s) => {
                let size = parse_bytes(s).map_err(de::Error::custom)?;
                Ok(Some(size))
            }
        }
    }

    pub fn serialize<S: Serializer>(u: &Option<usize>, s: S) -> Result<S::Ok, S::Error> {
        match u {
            Some(v) => s.serialize_str(ibytes(*v).as_str()),
            None => s.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let tests = [
            ("42", 42),
            ("42b", 42),
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
            ("1005.03 MB", 1005030000),
            // Large testing, breaks when too much larger than
            // this.
            ("12.5 EB", (12.5 * EBYTE as f64) as usize),
            ("12.5 E", (12.5 * EBYTE as f64) as usize),
            ("12.5 EiB", (12.5 * EIBYTE as f64) as usize),
        ];

        for (input, want) in tests {
            let value = parse_bytes(input).unwrap();
            assert_eq!(value, want, "input: {}", input);
        }
    }

    #[test]
    fn stringify() {
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
