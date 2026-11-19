//! [`Exact`]: a value decoded into `T` where `T` holds it, and refused where it does not, never
//! changed and never a panic.
//!
//! A Postgres `date`, `timestamp` and `timestamptz` may hold infinity and -infinity and years from
//! 4713 BC to past 200,000 AD, and a `time` holds `24:00:00`; the type descriptor says so of every
//! such column (`infinity`, the bounds). chrono's and time's types hold none of these, and sqlx's
//! decoders meet them by adding to an epoch: chrono's addition panics past its range and wraps a
//! time of day at midnight, so `24:00:00` arrives as `00:00:00`. The macros decode such a column
//! through `Exact`, which reads the value itself and refuses what the Rust type cannot hold, so a
//! read is exact or an error naming the column.

/// A value of `T`, decoded exactly: what `T` cannot hold is an error when the row is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Exact<T>(pub T);

// The readers the date and time decoders share, which only `chrono` and `time` call.
#[cfg(feature = "postgres")]
#[cfg_attr(not(any(feature = "chrono", feature = "time")), allow(dead_code, unused_macros, unused_imports))]
mod postgres {
    use super::Exact;
    use crate::sqlx::error::BoxDynError;
    use crate::sqlx::postgres::{PgHasArrayType, PgTypeInfo, PgValueRef, Postgres};
    use crate::sqlx::Type;

    impl<T: Type<Postgres>> Type<Postgres> for Exact<T> {
        fn type_info() -> PgTypeInfo {
            T::type_info()
        }

        fn compatible(ty: &PgTypeInfo) -> bool {
            T::compatible(ty)
        }
    }

    impl<T: PgHasArrayType> PgHasArrayType for Exact<T> {
        fn array_type_info() -> PgTypeInfo {
            T::array_type_info()
        }

        fn array_compatible(ty: &PgTypeInfo) -> bool {
            T::array_compatible(ty)
        }
    }

    /// Days from 2000-01-01, as Postgres sends a `date`; `i32::MAX` and `i32::MIN` are its
    /// infinities.
    pub(super) fn days(value: &PgValueRef<'_>) -> Result<i32, BoxDynError> {
        let days = i32::from_be_bytes(value.as_bytes()?.try_into()?);
        finite(days == i32::MAX, days == i32::MIN)?;
        Ok(days)
    }

    /// Microseconds from 2000-01-01 00:00:00, as Postgres sends a `timestamp` (and a `timestamptz`,
    /// in UTC); `i64::MAX` and `i64::MIN` are its infinities.
    pub(super) fn micros(value: &PgValueRef<'_>) -> Result<i64, BoxDynError> {
        let micros = i64::from_be_bytes(value.as_bytes()?.try_into()?);
        finite(micros == i64::MAX, micros == i64::MIN)?;
        Ok(micros)
    }

    /// Microseconds from midnight, as Postgres sends a `time`, up to 24:00:00 itself.
    pub(super) fn time_of_day(value: &PgValueRef<'_>) -> Result<i64, BoxDynError> {
        let micros = i64::from_be_bytes(value.as_bytes()?.try_into()?);
        if micros >= 86_400_000_000 {
            return Err("24:00:00 is no Rust time of day: read the column as text (`x::text`) to keep it".into());
        }
        Ok(micros)
    }

    /// A value in text, which Postgres writes `infinity` and `24:00:00` in too.
    pub(super) fn text<'r>(value: &PgValueRef<'r>) -> Result<&'r str, BoxDynError> {
        let text = value.as_str()?;
        finite(text == "infinity", text == "-infinity")?;
        if text.starts_with("24:00:00") {
            return Err("24:00:00 is no Rust time of day: read the column as text (`x::text`) to keep it".into());
        }
        Ok(text)
    }

    fn finite(infinity: bool, minus_infinity: bool) -> Result<(), BoxDynError> {
        match (infinity, minus_infinity) {
            (true, _) => Err("infinity is no Rust date or time: read the column as text (`x::text`) to keep it".into()),
            (_, true) => Err("-infinity is no Rust date or time: read the column as text (`x::text`) to keep it".into()),
            _ => Ok(()),
        }
    }

    pub(super) fn out_of_range(ty: &str) -> BoxDynError {
        format!("the value is out of {ty}'s range: read the column as text (`x::text`) to keep it").into()
    }

    /// The decoders: each reads a binary value itself and hands a text one, once checked, to sqlx's.
    macro_rules! exact {
        ($ty:ty, |$value:ident| $binary:expr) => {
            impl<'r> Decode<'r, Postgres> for Exact<$ty> {
                fn decode($value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
                    Ok(Exact(match $value.format() {
                        PgValueFormat::Binary => $binary,
                        PgValueFormat::Text => {
                            text(&$value)?;
                            <$ty as Decode<'r, Postgres>>::decode($value)?
                        }
                    }))
                }
            }
        };
    }
    pub(super) use exact;
}

#[cfg(all(feature = "postgres", feature = "chrono"))]
mod chrono {
    use super::postgres::{days, exact, micros, out_of_range, text, time_of_day};
    use super::Exact;
    use crate::sqlx::error::BoxDynError;
    use crate::sqlx::postgres::{PgValueFormat, PgValueRef, Postgres};
    use ::chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Utc};
    use crate::sqlx::Decode;

    fn epoch() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2000, 1, 1).and_then(|d| d.and_hms_opt(0, 0, 0)).expect("2000-01-01 is a date")
    }

    fn date_time(value: &PgValueRef<'_>) -> Result<NaiveDateTime, BoxDynError> {
        epoch().checked_add_signed(TimeDelta::microseconds(micros(value)?)).ok_or_else(|| out_of_range("chrono"))
    }

    exact!(NaiveDate, |value| epoch().date().checked_add_signed(TimeDelta::try_days(days(&value)?.into()).ok_or_else(|| out_of_range("chrono"))?).ok_or_else(|| out_of_range("chrono"))?);
    exact!(NaiveDateTime, |value| date_time(&value)?);
    exact!(DateTime<Utc>, |value| date_time(&value)?.and_utc());
    exact!(NaiveTime, |value| {
        let micros = time_of_day(&value)?;
        NaiveTime::from_num_seconds_from_midnight_opt((micros / 1_000_000) as u32, (micros % 1_000_000 * 1000) as u32).ok_or_else(|| out_of_range("chrono"))?
    });
}

#[cfg(all(feature = "postgres", feature = "time"))]
mod time {
    use super::postgres::{days, exact, micros, out_of_range, text, time_of_day};
    use super::Exact;
    use crate::sqlx::error::BoxDynError;
    use crate::sqlx::postgres::{PgValueFormat, PgValueRef, Postgres};
    use ::time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time};
    use crate::sqlx::Decode;

    fn epoch() -> PrimitiveDateTime {
        Date::from_calendar_date(2000, Month::January, 1).expect("2000-01-01 is a date").midnight()
    }

    fn date_time(value: &PgValueRef<'_>) -> Result<PrimitiveDateTime, BoxDynError> {
        epoch().checked_add(Duration::microseconds(micros(value)?)).ok_or_else(|| out_of_range("time"))
    }

    exact!(Date, |value| epoch().date().checked_add(Duration::days(days(&value)?.into())).ok_or_else(|| out_of_range("time"))?);
    exact!(PrimitiveDateTime, |value| date_time(&value)?);
    exact!(OffsetDateTime, |value| date_time(&value)?.assume_utc());
    exact!(Time, |value| {
        let micros = time_of_day(&value)?;
        let (h, m, s, us) = (micros / 3_600_000_000, micros / 60_000_000 % 60, micros / 1_000_000 % 60, micros % 1_000_000);
        Time::from_hms_micro(h as u8, m as u8, s as u8, us as u32)?
    });
}

/// A Postgres `numeric` as a `rust_decimal::Decimal`, read from the value itself: one past the
/// Decimal's 96 bits or 28 decimal places, NaN and the infinities are errors. sqlx 0.8's own decoder
/// panics on the first.
#[cfg(all(feature = "postgres", feature = "rust_decimal"))]
mod decimal {
    use super::postgres::out_of_range;
    use super::Exact;
    use crate::sqlx::error::BoxDynError;
    use crate::sqlx::postgres::{PgValueFormat, PgValueRef, Postgres};
    use crate::sqlx::types::Decimal;
    use crate::sqlx::Decode;

    impl<'r> Decode<'r, Postgres> for Exact<Decimal> {
        fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
            Ok(Exact(match value.format() {
                PgValueFormat::Binary => binary(value.as_bytes()?)?,
                PgValueFormat::Text => {
                    let text = value.as_str()?;
                    special(text)?;
                    Decimal::from_str_exact(text).map_err(|_| out_of_range("rust_decimal"))?
                }
            }))
        }
    }

    fn special(text: &str) -> Result<(), BoxDynError> {
        match text {
            "NaN" => Err("NaN is no Rust decimal: read the column as text (`x::text`) to keep it".into()),
            "Infinity" | "-Infinity" => Err(format!("{text} is no Rust decimal: read the column as text (`x::text`) to keep it").into()),
            _ => Ok(()),
        }
    }

    /// The binary `numeric`: the count of its base-10000 digits, the weight of the first, its sign
    /// and its scale, each 16 bits, then the digits.
    fn binary(bytes: &[u8]) -> Result<Decimal, BoxDynError> {
        let field = |at: usize| -> Result<u16, BoxDynError> { Ok(u16::from_be_bytes(bytes.get(at..at + 2).ok_or("a numeric shorter than its header")?.try_into()?)) };
        let (count, weight, sign, kept) = (usize::from(field(0)?), field(2)? as i16, field(4)?, u32::from(field(6)?));
        match sign {
            0x0000 | 0x4000 => {}
            0xC000 => special("NaN")?,
            0xD000 => special("Infinity")?,
            _ => special("-Infinity")?,
        }
        let mut mantissa: i128 = 0;
        for i in 0..count {
            let digit = i128::from(field(8 + 2 * i)?);
            mantissa = mantissa.checked_mul(10_000).and_then(|m| m.checked_add(digit)).ok_or_else(|| out_of_range("rust_decimal"))?;
        }
        // The digits are the mantissa times 10000 to the power of the last digit's weight.
        let exponent = 4 * (i32::from(weight) - count as i32 + 1);
        let mut scale = 0u32;
        if exponent >= 0 {
            let power = 10i128.checked_pow(exponent as u32).ok_or_else(|| out_of_range("rust_decimal"))?;
            mantissa = mantissa.checked_mul(power).ok_or_else(|| out_of_range("rust_decimal"))?;
        } else {
            scale = exponent.unsigned_abs();
        }
        // The scale Postgres keeps: the zeros a digit pads past it dropped, and those it shows kept.
        while scale > kept && mantissa % 10 == 0 {
            mantissa /= 10;
            scale -= 1;
        }
        while scale < kept {
            mantissa = mantissa.checked_mul(10).ok_or_else(|| out_of_range("rust_decimal"))?;
            scale += 1;
        }
        if sign == 0x4000 {
            mantissa = -mantissa;
        }
        Decimal::try_from_i128_with_scale(mantissa, scale).map_err(|_| out_of_range("rust_decimal"))
    }

    #[cfg(test)]
    mod tests {
        use super::{binary, Decimal};
        use std::str::FromStr;

        /// The binary `numeric` of a count, weight, sign, scale and digits.
        fn numeric(weight: i16, sign: u16, scale: u16, digits: &[u16]) -> Vec<u8> {
            let mut out = Vec::new();
            for field in [digits.len() as u16, weight as u16, sign, scale].into_iter().chain(digits.iter().copied()) {
                out.extend_from_slice(&field.to_be_bytes());
            }
            out
        }

        #[test]
        fn a_numeric_reads_exactly_or_is_refused() {
            // 1.50: one digit before the point, 5000 after it, with a scale of 2.
            assert_eq!(binary(&numeric(0, 0, 2, &[1, 5000])).unwrap(), Decimal::from_str("1.50").unwrap());
            assert_eq!(binary(&numeric(0, 0x4000, 2, &[1, 5000])).unwrap(), Decimal::from_str("-1.50").unwrap());
            // 120000: 12 in the 10000s place.
            assert_eq!(binary(&numeric(1, 0, 0, &[12])).unwrap(), Decimal::from(120_000));
            // 0.0001
            assert_eq!(binary(&numeric(-1, 0, 4, &[1])).unwrap(), Decimal::from_str("0.0001").unwrap());
            assert_eq!(binary(&numeric(0, 0, 0, &[])).unwrap(), Decimal::ZERO);
            // 10^40 is past 96 bits, and NaN is no decimal.
            assert!(binary(&numeric(10, 0, 0, &[1])).is_err());
            assert!(binary(&numeric(0, 0xC000, 0, &[])).is_err());
        }
    }
}

/// A Postgres `numeric` as a `bigdecimal::BigDecimal`, which holds every finite one: sqlx's own
/// decoder, with which NaN and the infinities are errors.
#[cfg(all(feature = "postgres", feature = "bigdecimal"))]
impl<'r> crate::sqlx::Decode<'r, crate::sqlx::Postgres> for Exact<crate::sqlx::types::BigDecimal> {
    fn decode(value: crate::sqlx::postgres::PgValueRef<'r>) -> Result<Self, crate::sqlx::error::BoxDynError> {
        Ok(Exact(<crate::sqlx::types::BigDecimal as crate::sqlx::Decode<'r, crate::sqlx::Postgres>>::decode(value)?))
    }
}

/// A MySQL text column as a `String`, whatever its collation: sqlx 0.8 refuses text with a binary
/// collation (`utf8mb4_bin`) for a `String`, which the engine has typed as text, and 0.9 reads it.
#[cfg(feature = "mysql")]
mod mysql {
    use super::Exact;
    use crate::sqlx::error::BoxDynError;
    use crate::sqlx::mysql::{MySql, MySqlTypeInfo, MySqlValueRef};
    use crate::sqlx::{Decode, Type};

    impl Type<MySql> for Exact<String> {
        fn type_info() -> MySqlTypeInfo {
            <String as Type<MySql>>::type_info()
        }

        fn compatible(ty: &MySqlTypeInfo) -> bool {
            <String as Type<MySql>>::compatible(ty) || <Vec<u8> as Type<MySql>>::compatible(ty)
        }
    }

    impl<'r> Decode<'r, MySql> for Exact<String> {
        fn decode(value: MySqlValueRef<'r>) -> Result<Self, BoxDynError> {
            let bytes = <&[u8] as Decode<'r, MySql>>::decode(value)?;
            Ok(Exact(std::str::from_utf8(bytes)?.to_owned()))
        }
    }
}
