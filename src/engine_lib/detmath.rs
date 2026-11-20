//! Port of upstream `packages/engine-lib/src/DetMath.ts`: `exp`, `log`, `pow`, `atan2` built from
//! `+ - * /` and bit manipulation only, so results are bit-identical on every platform (and to
//! upstream). The engine must use these instead of `f64::powf`/`exp`/`ln`/`atan2`.

const LN2: f64 = std::f64::consts::LN_2;
const LOG2E: f64 = std::f64::consts::LOG2_E;
const SQRT2: f64 = std::f64::consts::SQRT_2;
const PI: f64 = std::f64::consts::PI;
const PI_2: f64 = std::f64::consts::FRAC_PI_2;
const PI_4: f64 = std::f64::consts::FRAC_PI_4;
const TAN_PI_8: f64 = 0.414_213_562_373_095_03;

/// `2^n` for integer `n`; saturates to infinity / 0.
pub fn pow2(n: i64) -> f64 {
    if n > 1023 {
        return f64::INFINITY;
    }
    if n < -1022 {
        return 0.0;
    }
    f64::from_bits(((n + 1023) as u64) << 52)
}

/// `e^x`.
pub fn exp(x: f64) -> f64 {
    if x > 709.0 {
        return f64::INFINITY;
    }
    if x < -708.0 {
        return 0.0;
    }
    let n = (x * LOG2E + 0.5).floor();
    let r = x - n * LN2;
    let p = 1.0
        + r * (1.0
            + r * (1.0 / 2.0
                + r * (1.0 / 6.0
                    + r * (1.0 / 24.0
                        + r * (1.0 / 120.0
                            + r * (1.0 / 720.0 + r * (1.0 / 5040.0 + r * (1.0 / 40320.0))))))));
    p * pow2(n as i64)
}

/// Natural log of `x > 0` (`-inf` for 0, NaN for negatives).
pub fn log(x: f64) -> f64 {
    if x <= 0.0 {
        return if x == 0.0 {
            f64::NEG_INFINITY
        } else {
            f64::NAN
        };
    }
    let mut x = x;
    let mut e: i64 = 0;
    if x < 2.225_073_858_507_201_4e-308 {
        x *= 18_014_398_509_481_984.0; // 2^54
        e = -54;
    }
    let bits = x.to_bits();
    e += ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    if m > SQRT2 {
        m *= 0.5;
        e += 1;
    }
    let s = (m - 1.0) / (m + 1.0);
    let z = s * s;
    let series = 1.0
        + z * (1.0 / 3.0
            + z * (1.0 / 5.0
                + z * (1.0 / 7.0 + z * (1.0 / 9.0 + z * (1.0 / 11.0 + z * (1.0 / 13.0))))));
    e as f64 * LN2 + 2.0 * s * series
}

/// `x^y` for `x >= 0`.
pub fn pow(x: f64, y: f64) -> f64 {
    if y == 0.0 || x == 1.0 {
        return 1.0;
    }
    if x == 0.0 {
        return if y > 0.0 { 0.0 } else { f64::INFINITY };
    }
    if x < 0.0 {
        return f64::NAN;
    }
    exp(y * log(x))
}

fn atan_unit(z: f64) -> f64 {
    let mut z = z;
    let mut base = 0.0;
    if z > TAN_PI_8 {
        base = PI_4;
        z = (z - 1.0) / (z + 1.0);
    }
    let w = z * z;
    let series = 1.0
        - w * (1.0 / 3.0
            - w * (1.0 / 5.0
                - w * (1.0 / 7.0
                    - w * (1.0 / 9.0
                        - w * (1.0 / 11.0 - w * (1.0 / 13.0 - w * (1.0 / 15.0 - w / 17.0)))))));
    base + z * series
}

/// Angle of `(x, y)` in `(-pi, pi]`.
pub fn atan2(y: f64, x: f64) -> f64 {
    if y == 0.0 {
        return if x >= 0.0 { 0.0 } else { PI };
    }
    if x == 0.0 {
        return if y > 0.0 { PI_2 } else { -PI_2 };
    }
    let ax = x.abs();
    let ay = y.abs();
    let mut a = if ay <= ax {
        atan_unit(ay / ax)
    } else {
        PI_2 - atan_unit(ax / ay)
    };
    if x < 0.0 {
        a = PI - a;
    }
    if y < 0.0 {
        -a
    } else {
        a
    }
}

/// Upstream `Util.within`: clamp `v` into `[lo, hi]`.
pub fn within(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

/// Upstream `Util.sigmoid(x, steepness, midpoint)`: `1 / (1 + e^(-steepness * (x - midpoint)))`.
pub fn sigmoid(x: f64, steepness: f64, midpoint: f64) -> f64 {
    1.0 / (1.0 + exp(-steepness * (x - midpoint)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference bit patterns from upstream's algorithm run in JavaScript.
    #[test]
    #[allow(clippy::approx_constant)] // 2.302585 is a test input, not ln(10)
    fn exp_matches_upstream() {
        for (x, bits) in [
            (-3.2, 0x3fa4dec899feb237u64),
            (-0.5, 0x3fe368b2fc6f838c),
            (0.0, 0x3ff0000000000000),
            (0.1, 0x3ff1aec7b35a00c7),
            (1.0, 0x4005bf0a8b0fa133),
            (2.302585, 0x4023ffffe0cb9b66),
            (7.5, 0x409c402b6eb1f75e),
            (50.0, 0x44719103e4080b42),
        ] {
            assert_eq!(exp(x).to_bits(), bits, "exp({x})");
        }
    }

    #[test]
    fn log_matches_upstream() {
        for (x, bits) in [
            (0.5, 0xbfe62e42fefa39efu64),
            (1.0, 0x0000000000000000),
            (2.5, 0x3fed5240f0e0e072),
            (10.0, 0x40026bb1bbb55514),
            (1234.5678, 0x401c7951d51791d7),
            (0.00001, 0xc027069e2aa2aa61),
            (300000.0, 0x4029391b79f84abc),
            (0.73, 0xbfd4243889324b58),
            (25000.0, 0x402440d5cac36323),
        ] {
            assert_eq!(log(x).to_bits(), bits, "log({x})");
        }
    }

    #[test]
    fn pow_matches_upstream() {
        for (x, y, bits) in [
            (25000.0, 0.73, 0x40995ea81b7aa4d8u64),
            (12345.0, 0.6, 0x4071d085dbacb7f3),
            (2.0, 10.0, 0x4090000000000000),
            (0.5, 3.3, 0x3fb9fdf8bcce984a),
            (300000.0, 0.6, 0x409e34ba69fc9149),
            (1.0, 0.6, 0x3ff0000000000000),
        ] {
            assert_eq!(pow(x, y).to_bits(), bits, "pow({x}, {y})");
        }
    }

    #[test]
    fn atan2_matches_upstream() {
        for (y, x, bits) in [
            (1.0, 1.0, 0x3fe921fb54442d18u64),
            (-1.0, 2.0, 0xbfddac6705566b9a),
            (3.0, -4.0, 0x4003fc176b7a8560),
            (-2.0, -2.0, 0xc002d97c7f3321d2),
            (0.0, -1.0, 0x400921fb54442d18),
        ] {
            assert_eq!(atan2(y, x).to_bits(), bits, "atan2({y}, {x})");
        }
    }

    #[test]
    fn close_to_std() {
        for i in 1..200 {
            let x = i as f64 * 37.3;
            assert!((log(x) - x.ln()).abs() < 1e-8 * x.ln().abs().max(1.0));
            let y = (i as f64 - 100.0) / 13.0;
            assert!((exp(y) - y.exp()).abs() <= 1e-8 * y.exp());
        }
    }
}
