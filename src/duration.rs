// Port from Go's std time package

use std::fmt::{Display, Formatter};
use std::time::Duration;

const NANOSECOND: i64 = 1;
const MICROSECOND: i64 = 1000 * NANOSECOND;
const MILLISECOND: i64 = 1000 * MICROSECOND;
const SECOND: i64 = 1000 * MILLISECOND;
const MINUTE: i64 = 60 * SECOND;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const WEEK: i64 = 7 * DAY;

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum Error {
    BadInteger,
    InvalidDuration,
    MissingUnit,
    UnknownUnit,
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Error::BadInteger => "bad integer",
            Error::InvalidDuration => "invalid duration",
            Error::MissingUnit => "missing unit",
            Error::UnknownUnit => "unknown unit",
        };

        write!(f, "{}", msg)
    }
}

/// leading_int consumes the leading [0-9]* from s
fn leading_int(s: &[u8]) -> Result<(u64, &[u8]), Error> {
    let mut consumed = 0;
    let o = s
        .iter()
        .take_while(|c| **c >= b'0' && **c <= b'9')
        .try_fold(0u64, |x, &c| {
            consumed += 1;

            if x > u64::MAX / 10 {
                None
            } else {
                Some(10 * x + c as u64 - b'0' as u64)
            }
        });

    match o {
        Some(v) => Ok((v, &s[consumed..])),
        None => Err(Error::BadInteger),
    }
}

/// leading_fraction consumes the leader [0-9]* from s.
/// It is used only for fractions, so does not return an error on overflow,
/// it just stops accumulating precision.
fn leading_fraction(s: &[u8]) -> (i64, f64, &[u8]) {
    let mut consumed = 0;
    let mut scale = 1.0;
    let mut overflow = false;

    let o = s
        .iter()
        .take_while(|c| **c >= b'0' && **c <= b'9')
        .try_fold(0, |x, &c| {
            consumed += 1;

            if overflow {
                return Some(x);
            }

            if x > i64::MAX / 10 {
                overflow = true;
                return Some(x);
            }

            let y = x * 10 + c as i64 - b'0' as i64;
            if y < 0 {
                overflow = true;
                return Some(x);
            }

            scale *= 10.0;
            Some(y)
        })
        .unwrap();

    (o, scale, &s[consumed..])
}

/// parse_duration parses a duration string.
/// A duration string is a possibly signed sequence of decimal numbers,
/// each with optional fraction and a unit suffix, such as "300ms", "-1.5h" or "2h45m".
/// Valid time units are "ns", "us" (or "µs"), "ms", "s", "m", "h", "d", "w".
pub fn parse_duration(text: &str) -> Result<Duration, Error> {
    let d = parse(text)?;

    Ok(Duration::from_nanos(d as u64))
}

fn parse(text: &str) -> Result<i64, Error> {
    // [-+]?([0-9]*(\.[0-9]*)?[a-z]+)+
    let mut d = 0u64;
    let mut neg = false;
    let mut s = text.as_bytes();

    // Consume [-+]?
    if !s.is_empty() {
        let c = s[0];
        if c == b'-' || c == b'+' {
            neg = c == b'-';
            s = &s[1..];
        }
    }

    // Special case: if all that is left is "0", this is zero
    if s.len() == 1 && s[0] == b'0' {
        return Ok(0);
    }

    if s.is_empty() {
        return Err(Error::InvalidDuration);
    }

    while !s.is_empty() {
        let mut f = 0;
        let mut scale = 1.0;

        // The next character must be [0-9.]
        let c = s[0];
        if !(c == b'.' || c.is_ascii_digit()) {
            return Err(Error::InvalidDuration);
        }

        // Consume [0-9]*
        let pl = s.len();
        let (l, remain) = leading_int(s)?;
        let mut v = l;
        s = remain;
        let pre = pl != s.len();

        // Consume (\.[0-9]*)?
        let mut post = false;
        if !s.is_empty() && s[0] == b'.' {
            s = &s[1..];
            let pl = s.len();
            let (lf, ls, remain) = leading_fraction(s);
            f = lf;
            scale = ls;
            s = remain;
            post = pl != s.len();
        }
        if !pre && !post {
            // no digits (e.g. ".s" or "-.s")
            return Err(Error::InvalidDuration);
        }

        // Consume unit
        let mut i = 0;
        while i < s.len() {
            let c = s[i];
            if c == b'.' || c.is_ascii_digit() {
                break;
            }

            i += 1;
        }

        if i == 0 {
            return Err(Error::MissingUnit);
        }
        let u = &s[..i];
        s = &s[i..];
        let unit = match u {
            [b'n', b's'] => NANOSECOND,
            [b'u', b's'] => MICROSECOND,
            // "µs" U+00B5
            [194, 181, 115] => MICROSECOND,
            // "μs" U+03BC
            [206, 188, 115] => MICROSECOND,
            [b'm', b's'] => MILLISECOND,
            [b's'] => SECOND,
            [b'm'] => MINUTE,
            [b'h'] => HOUR,
            [b'd'] => DAY,
            [b'w'] => WEEK,
            _ => 0,
        } as u64;
        if unit == 0 {
            return Err(Error::UnknownUnit);
        }

        if v > (1 << 63) / unit {
            return Err(Error::InvalidDuration);
        }

        v *= unit;
        if f > 0 {
            // float64 is needed to be nanosecond accurate for fractions of hours.
            // v >= 0 && (f * unit / scale) <= 3.6e+12 (ns/h, h is the largest unit)
            v = v
                .checked_add((f as f64 * (unit as f64 / scale)) as u64)
                .ok_or(Error::InvalidDuration)?;
        }

        d += v;
        if d > 1 << 63 {
            return Err(Error::InvalidDuration);
        }
    }

    if neg {
        return Ok(-(d as i64));
    }

    if d > (1 << 63) - 1 {
        return Err(Error::InvalidDuration);
    }

    Ok(d as i64)
}

pub fn duration(d: &Duration) -> String {
    to_string(d.as_nanos() as i64)
}

/// duration returns a string representing the duration in the form "72h3m0.5s".
/// Leading zero units are omitted. As a special case, durations less than one
/// second format use a smaller unit (milli-, micro-, or nanoseconds) to ensure
/// that the leading digit is non-zero. The zero duration formats as 0s.
pub fn to_string(d: i64) -> String {
    // Largest time is 2540400h10m10.000000000s
    let mut w = 32;
    let mut buf = [0u8; 32];
    let neg = d < 0;

    let d = d as u64;
    let mut u = d;

    if u < SECOND as u64 {
        // Special case: if duration is smaller thant a second,
        // use smaller units, like 1.2ms
        w -= 1;
        buf[w] = b's';
        w -= 1;

        let prec = if u == 0 {
            return "0s".to_string();
        } else if u < MICROSECOND as u64 {
            // print nanoseconds
            buf[w] = b'n';
            0
        } else if u < MILLISECOND as u64 {
            // print microseconds

            /*
            // U+00B5 'µ' micro sign == 0xC2 0xB5
            w -= 1; // Need room for two bytes
            buf[w + 1] = 0xC2;
            buf[w + 2] = 0xB5;
            */

            buf[w] = b'u';
            3
        } else {
            // print milliseconds
            buf[w] = b'm';
            6
        };

        let (_w, _u) = fmt_frac(&mut buf[..w], u, prec);
        w = _w;
        u = _u;
        w = fmt_int(&mut buf[..w], u);
    } else {
        if u % SECOND as u64 != 0 {
            w -= 1;
            buf[w] = b's';

            let (_w, _u) = fmt_frac(&mut buf[..w], u, 9);
            w = _w;
            u = _u;

            // u is now integer seconds
            w = fmt_int(&mut buf[..w], u % 60);
        } else {
            u /= SECOND as u64;

            let n = u % 60;
            if n != 0 {
                w -= 1;
                buf[w] = b's';

                // u is now integer seconds
                w = fmt_int(&mut buf[..w], u % 60);
            }
        }

        u /= 60;

        // u is now integer minutes
        if u > 0 {
            if u % 60 != 0 {
                w -= 1;
                buf[w] = b'm';
                w = fmt_int(&mut buf[..w], u % 60);
            }

            u /= 60;

            // u is now integer hours
            // Stop at hours because days can be different lengths.
            if u > 0 {
                w -= 1;
                buf[w] = b'h';
                w = fmt_int(&mut buf[..w], u)
            }
        }
    }

    if neg {
        w -= 1;
        buf[w] = b'-';
    }

    String::from_utf8_lossy(&buf[w..]).to_string()
}

// fmt_frac formats the fraction of v / 10 ** prec (e.g., ".12345") into the
// tail of buf, omitting trailing zeros. It omits the decimal point too when
// the fraction is 0. It returns the index where the output bytes begin and
// the value v / 10 ** prec
fn fmt_frac(buf: &mut [u8], mut v: u64, prec: i32) -> (usize, u64) {
    // Omit trailing zeros up to and including decimal point
    let mut w = buf.len();
    let mut print = false;
    for _i in 0..prec {
        let digit = v % 10;
        print = print || digit != 0;
        if print {
            w -= 1;
            buf[w] = digit as u8 + b'0';
        }

        v /= 10;
    }

    if print {
        w -= 1;
        buf[w] = b'.';
    }

    (w, v)
}

// fmt_int formats v into the tail of buf.
// It returns the index where the output begins.
fn fmt_int(buf: &mut [u8], mut v: u64) -> usize {
    let mut w = buf.len();
    if v == 0 {
        w -= 1;
        buf[w] = b'0';
    } else {
        while v > 0 {
            w -= 1;
            buf[w] = (v % 10) as u8 + b'0';
            v /= 10;
        }
    }

    w
}

#[cfg(feature = "serde")]
pub mod serde {
    use std::borrow::Cow;

    use super::{duration, parse_duration};
    use serde::{Deserializer, Serializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<std::time::Duration, D::Error> {
        let s: Cow<str> = serde::__private::de::borrow_cow_str(deserializer)?;
        parse_duration(&s).map_err(serde::de::Error::custom)
    }

    pub fn serialize<S: Serializer>(d: &std::time::Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&duration(d))
    }
}

#[cfg(feature = "serde")]
pub mod serde_option {
    use super::{duration, parse_duration};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<std::time::Duration>, D::Error> {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(text) => {
                let duration = parse_duration(&text).map_err(serde::de::Error::custom)?;
                Ok(Some(duration))
            }
            None => Ok(None),
        }
    }

    pub fn serialize<S: Serializer>(
        d: &Option<std::time::Duration>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => s.serialize_str(&duration(d)),
            None => s.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_leading_int() {
        let (x, remain) = leading_int("12h".as_bytes()).unwrap();
        assert_eq!(x, 12u64);
        assert_eq!(String::from_utf8_lossy(remain), "h".to_string());
    }

    #[test]
    fn test_leading_int_overflow() {
        let err = leading_int("999999999999999999999".as_bytes()).unwrap_err();
        assert_eq!(err, Error::BadInteger)
    }

    #[test]
    fn test_parse_duration() {
        let tests = [
            // simple
            ("0", 0),
            ("5s", 5 * SECOND),
            ("30s", 30 * SECOND),
            ("1478s", 1478 * SECOND),
            // sign
            ("-5s", -5 * SECOND),
            ("+5s", 5 * SECOND),
            ("-0", 0),
            ("+0", 0),
            // decimal
            ("5.0s", 5 * SECOND),
            ("5.6s", 5 * SECOND + 600 * MILLISECOND),
            ("5.s", 5 * SECOND),
            (".5s", 500 * MILLISECOND),
            ("1.0s", SECOND),
            ("1.00s", SECOND),
            ("1.004s", SECOND + 4 * MILLISECOND),
            ("1.0040s", SECOND + 4 * MILLISECOND),
            ("100.00100s", 100 * SECOND + MILLISECOND),
            // different units
            ("10ns", 10 * NANOSECOND),
            ("11us", 11 * MICROSECOND),
            ("12µs", 12 * MICROSECOND),                       // U+00B5
            ("12µs10ns", 12 * MICROSECOND + 10 * NANOSECOND), // U+00B5
            ("12μs", 12 * MICROSECOND),                       // U+03BC
            ("12μs10ns", 12 * MICROSECOND + 10 * NANOSECOND), // U+03BC
            ("13ms", 13 * MILLISECOND),
            ("14s", 14 * SECOND),
            ("15m", 15 * MINUTE),
            ("16h", 16 * HOUR),
            // composite durations
            ("3h30m", 3 * HOUR + 30 * MINUTE),
            ("10.5s4m", 4 * MINUTE + 10 * SECOND + 500 * MILLISECOND),
            ("-2m3.4s", -(2 * MINUTE + 3 * SECOND + 400 * MILLISECOND)),
            (
                "1h2m3s4ms5us6ns",
                HOUR + 2 * MINUTE + 3 * SECOND + 4 * MILLISECOND + 5 * MICROSECOND + 6 * NANOSECOND,
            ),
            (
                "39h9m14.425s",
                39 * HOUR + 9 * MINUTE + 14 * SECOND + 425 * MILLISECOND,
            ),
            // large value
            ("52763797000ns", 52763797000 * NANOSECOND),
            // more than 9 digits after decimal point, see https://golang.org/issue/6617
            ("0.3333333333333333333h", 20 * MINUTE),
            // 9007199254740993 = 1<<53+1 cannot be stored precisely in a float64
            ("9007199254740993ns", ((1 << 53) + 1) * NANOSECOND),
            // largest duration that can be represented by int64 in nanoseconds
            ("9223372036854775807ns", i64::MAX * NANOSECOND),
            ("9223372036854775.807us", i64::MAX * NANOSECOND),
            ("9223372036s854ms775us807ns", i64::MAX * NANOSECOND),
            // large negative value
            // todo: ( "-9223372036854775807ns", -1 << 63 + NANOSECOND ),
            // huge string; issue 15011.
            ("0.100000000000000000000h", 6 * MINUTE),
            // This value tests the first overflow check in leadingFraction.
            (
                "0.830103483285477580700h",
                49 * MINUTE + 48 * SECOND + 372539827 * NANOSECOND,
            ),
        ];

        for (input, want) in tests {
            let got = parse(input).expect(&format!("parse {input} success"));
            assert_eq!(got, want, "input: {}", input);
        }
    }

    #[test]
    fn parse_us() {
        let input = "12µs"; // U+00B5
        let _d = parse_duration(input).unwrap();

        let input = "12μs"; // U+03BC
        let _d = parse_duration(input).unwrap();
    }

    #[test]
    fn test_leading_fraction() {
        let (f, scale, r) = leading_fraction("6s".as_bytes());
        assert_eq!(6, f);
        assert_eq!(10.0, scale);
        assert_eq!(r, "s".as_bytes());
    }

    #[test]
    fn test_duration_to_string() {
        let tests = vec![
            ("0s", 0),
            ("1ns", NANOSECOND),
            ("1.1us", 1100 * NANOSECOND),
            ("2.2ms", 2200 * MICROSECOND),
            ("3.3s", 3300 * MILLISECOND),
            ("4m5s", 4 * MINUTE + 5 * SECOND),
            ("4m5.001s", 4 * MINUTE + 5001 * MILLISECOND),
            ("1h", HOUR),
            ("2h3m", 2 * HOUR + 3 * MINUTE),
            ("1h2m3s", HOUR + 2 * MINUTE + 3 * SECOND),
            (
                "1h2m3.4s",
                HOUR + 2 * MINUTE + 3 * SECOND + 400 * MILLISECOND,
            ),
            ("1m", MINUTE),
            ("5h6m7.001s", 5 * HOUR + 6 * MINUTE + 7001 * MILLISECOND),
            ("8m0.000000001s", 8 * MINUTE + NANOSECOND),
            ("2562047h47m16.854775807s", i64::MAX),
            ("-2562047h47m16.854775808s", i64::MIN),
        ];

        for (want, input) in tests {
            let d = Duration::from_nanos(input as u64);
            assert_eq!(duration(&d), want, "want {want}")
        }
    }
}
