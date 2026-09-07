//! Rational timestamp representation for frame-exact and sample-exact calculations.
//!
//! Stores timestamps as `num / den` (integer numerator and denominator)
//! to prevent floating-point drift during editing, trimming, and playback.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeRational {
    pub num: i64,
    pub den: u32,
}

impl TimeRational {
    pub const ZERO: Self = Self { num: 0, den: 1 };

    pub fn new(num: i64, den: u32) -> Self {
        assert!(den > 0, "Denominator cannot be zero");
        let mut r = Self { num, den };
        r.reduce();
        r
    }

    pub fn from_seconds(seconds: f64, timebase: u32) -> Self {
        let num = (seconds * (timebase as f64)).round() as i64;
        Self::new(num, timebase)
    }

    pub fn to_seconds(self) -> f64 {
        (self.num as f64) / (self.den as f64)
    }

    pub fn reduce(&mut self) {
        if self.num == 0 {
            self.den = 1;
            return;
        }
        let gcd = gcd(self.num.unsigned_abs(), self.den as u64) as u32;
        if gcd > 1 {
            self.num /= gcd as i64;
            self.den /= gcd;
        }
    }

    pub fn rescale(self, new_den: u32) -> Self {
        if self.den == new_den {
            return self;
        }
        let new_num = ((self.num as i128) * (new_den as i128) / (self.den as i128)) as i64;
        Self {
            num: new_num,
            den: new_den,
        }
    }
    pub fn is_zero(self) -> bool {
        self.num == 0
    }

    pub fn abs(self) -> Self {
        Self {
            num: self.num.abs(),
            den: self.den,
        }
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

impl Add for TimeRational {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let common_den =
            (self.den as u64) * (rhs.den as u64) / gcd(self.den as u64, rhs.den as u64);
        let s_num = (self.num as i128) * ((common_den / self.den as u64) as i128);
        let r_num = (rhs.num as i128) * ((common_den / rhs.den as u64) as i128);
        Self::new((s_num + r_num) as i64, common_den as u32)
    }
}

impl Sub for TimeRational {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let common_den =
            (self.den as u64) * (rhs.den as u64) / gcd(self.den as u64, rhs.den as u64);
        let s_num = (self.num as i128) * ((common_den / self.den as u64) as i128);
        let r_num = (rhs.num as i128) * ((common_den / rhs.den as u64) as i128);
        Self::new((s_num - r_num) as i64, common_den as u32)
    }
}

impl PartialEq for TimeRational {
    fn eq(&self, other: &Self) -> bool {
        (self.num as i128) * (other.den as i128) == (other.num as i128) * (self.den as i128)
    }
}

impl Eq for TimeRational {}

impl PartialOrd for TimeRational {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeRational {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let left = (self.num as i128) * (other.den as i128);
        let right = (other.num as i128) * (self.den as i128);
        left.cmp(&right)
    }
}

impl fmt::Display for TimeRational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}s ({}/{})", self.to_seconds(), self.num, self.den)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_rational_arithmetic() {
        let t1 = TimeRational::new(1, 2); // 0.5s
        let t2 = TimeRational::new(1, 4); // 0.25s
        let sum = t1 + t2;
        assert_eq!(sum.num, 3);
        assert_eq!(sum.den, 4);
        assert_eq!(sum.to_seconds(), 0.75);

        let diff = t1 - t2;
        assert_eq!(diff.num, 1);
        assert_eq!(diff.den, 4);
        assert_eq!(diff.to_seconds(), 0.25);
    }

    #[test]
    fn test_time_rational_from_seconds() {
        let t = TimeRational::from_seconds(1.5, 1000);
        assert_eq!(t.to_seconds(), 1.5);
    }

    #[test]
    fn test_time_rational_exact_ordering_and_equality() {
        let t_2s = TimeRational { num: 2, den: 1 };
        let t_1_5s = TimeRational { num: 3, den: 2 };
        assert!(t_2s > t_1_5s);
        assert!(t_1_5s < t_2s);

        let t_half_a = TimeRational { num: 1, den: 2 };
        let t_half_b = TimeRational { num: 2, den: 4 };
        assert_eq!(t_half_a, t_half_b);
        assert_eq!(t_half_a.cmp(&t_half_b), std::cmp::Ordering::Equal);
    }
}
