//! Decimal input is converted once; all tariff arithmetic then uses checked integers.
use crate::error::GatewayError;

pub(crate) const MAX_MICROS: i64 = 9_007_199_254_740_991;
const PPM: u128 = 1_000_000;

pub(crate) fn amount_error() -> GatewayError {
    GatewayError::bad_request("Cash amount or billing multiplier is out of range")
        .with_code("cash_amount_invalid")
}

/// Accepts decimal/scientific notation only when exactly representable to six places.
pub(crate) fn multiplier_ppm(value: &str) -> Result<u64, GatewayError> {
    let value = value.trim();
    let (coefficient, exponent) = value
        .split_once(['e', 'E'])
        .map(|(coefficient, exponent)| {
            exponent
                .parse::<i32>()
                .map(|exponent| (coefficient, exponent))
        })
        .transpose()
        .map_err(|_| amount_error())?
        .unwrap_or((value, 0));
    if coefficient.len() > 40 || exponent.unsigned_abs() > 18 {
        return Err(amount_error());
    }
    let (whole, fraction) = coefficient.split_once('.').unwrap_or((coefficient, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(amount_error());
    }
    let digits = format!("{whole}{fraction}")
        .parse::<u128>()
        .map_err(|_| amount_error())?;
    let power = 6 + exponent - fraction.len() as i32;
    let scaled = if power >= 0 {
        digits.checked_mul(10u128.checked_pow(power as u32).ok_or_else(amount_error)?)
    } else {
        let divisor = 10u128
            .checked_pow((-power) as u32)
            .ok_or_else(amount_error)?;
        if digits % divisor != 0 {
            return Err(amount_error());
        }
        Some(digits / divisor)
    }
    .filter(|value| *value <= 1_000_000 * PPM)
    .ok_or_else(amount_error)?;
    u64::try_from(scaled).map_err(|_| amount_error())
}

pub(crate) fn charge(
    prompt: u64,
    completion: u64,
    prompt_rate: i64,
    completion_rate: i64,
    group_ppm: u64,
    account_ppm: u64,
) -> Result<i64, GatewayError> {
    if !(0..=MAX_MICROS).contains(&prompt_rate) || !(0..=MAX_MICROS).contains(&completion_rate) {
        return Err(amount_error());
    }
    let numerator = u128::from(prompt)
        .checked_mul(prompt_rate as u128)
        .and_then(|prompt| {
            u128::from(completion)
                .checked_mul(completion_rate as u128)
                .and_then(|completion| prompt.checked_add(completion))
        })
        .and_then(|value| value.checked_mul(u128::from(group_ppm)))
        .and_then(|value| value.checked_mul(u128::from(account_ppm)))
        .ok_or_else(amount_error)?;
    let denominator = 1000 * PPM * PPM;
    // Round once, upwards to a micro-dollar; never silently turn a positive charge into zero.
    let micros = numerator / denominator + u128::from(numerator % denominator != 0);
    if micros > MAX_MICROS as u128 {
        return Err(amount_error());
    }
    Ok(micros as i64)
}
