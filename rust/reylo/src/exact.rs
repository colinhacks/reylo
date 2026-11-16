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
    use sqlx::error::BoxDynError;
    use sqlx::postgres::{PgHasArrayType, PgTypeInfo, PgValueRef, Postgres};
    use sqlx::Type;

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
    use sqlx::error::BoxDynError;
    use sqlx::postgres::{PgValueFormat, PgValueRef, Postgres};
    use ::chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Utc};
    use sqlx::Decode;

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
    use sqlx::error::BoxDynError;
    use sqlx::postgres::{PgValueFormat, PgValueRef, Postgres};
    use ::time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time};
    use sqlx::Decode;

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
