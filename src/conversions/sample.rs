use dasp_sample::{FromSample, Sample as DaspSample, ToSample};
use std::marker::PhantomData;

/// Converts the samples data type to `O`.
#[derive(Clone, Debug)]
pub struct DataConverter<I, O> {
    input: I,
    marker: PhantomData<O>,
}

impl<I, O> DataConverter<I, O> {
    /// Builds a new converter.
    #[inline]
    pub fn new(input: I) -> DataConverter<I, O> {
        DataConverter {
            input,
            marker: PhantomData,
        }
    }

    /// Destroys this iterator and returns the underlying iterator.
    #[inline]
    pub fn into_inner(self) -> I {
        self.input
    }

    /// get mutable access to the iterator
    #[inline]
    pub fn inner_mut(&mut self) -> &mut I {
        &mut self.input
    }
}

impl<I, O> Iterator for DataConverter<I, O>
where
    I: Iterator,
    I::Item: Sample,
    O: FromSample<I::Item> + Sample,
{
    type Item = O;

    #[inline]
    fn next(&mut self) -> Option<O> {
        self.input.next().map(|s| DaspSample::from_sample(s))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<I, O> ExactSizeIterator for DataConverter<I, O>
where
    I: ExactSizeIterator,
    I::Item: Sample,
    O: FromSample<I::Item> + Sample,
{
}

/// Represents a value of a single sample.
///
/// This trait is implemented by default on three types: `i16`, `u16` and `f32`.
///
/// - For `i16`, silence corresponds to the value `0`. The minimum and maximum amplitudes are
///   represented by `i16::min_value()` and `i16::max_value()` respectively.
/// - For `u16`, silence corresponds to the value `u16::max_value() / 2`. The minimum and maximum
///   amplitudes are represented by `0` and `u16::max_value()` respectively.
/// - For `f32`, silence corresponds to the value `0.0`. The minimum and maximum amplitudes are
///   represented by `-1.0` and `1.0` respectively.
///
/// You can implement this trait on your own type as well if you wish so.
///
pub trait Sample: DaspSample + ToSample<f32> {
    /// The value corresponding to the absence of sound.
    const ZERO_VALUE: Self = DaspSample::EQUILIBRIUM;

    /// Linear interpolation between two samples.
    ///
    /// The result should be equivalent to
    /// `first * (1 - numerator / denominator) + second * numerator / denominator`.
    ///
    /// To avoid numeric overflows pick smaller numerator.
    fn lerp(first: Self, second: Self, numerator: u32, denominator: u32) -> Self;

    /// Multiplies the value of this sample by the given amount.
    #[inline]
    fn amplify(self, value: f32) -> Self {
        self.mul_amp(value.to_sample())
    }

    /// Converts the sample to a normalized `f32` value.
    #[inline]
    fn to_f32(self) -> f32 {
        self.to_sample()
    }

    /// Adds the amplitude of another sample to this one.
    #[inline]
    fn saturating_add(self, other: Self) -> Self {
        self.add_amp(other.to_signed_sample())
    }

    /// Returns true if the sample is the zero value.
    #[inline]
    fn is_zero(self) -> bool {
        self == Self::ZERO_VALUE
    }

    /// Returns the value corresponding to the absence of sound.
    #[deprecated(note = "Please use `Self::ZERO_VALUE` instead")]
    #[inline]
    fn zero_value() -> Self {
        Self::ZERO_VALUE
    }
}

impl Sample for u16 {
    #[inline]
    fn lerp(first: u16, second: u16, numerator: u32, denominator: u32) -> u16 {
        let a = first as i32;
        let b = second as i32;
        let n = numerator as i32;
        let d = denominator as i32;
        (a + (b - a) * n / d) as u16
    }
}

impl Sample for i16 {
    #[inline]
    fn lerp(first: i16, second: i16, numerator: u32, denominator: u32) -> i16 {
        (first as i32 + (second as i32 - first as i32) * numerator as i32 / denominator as i32)
            as i16
    }
}

impl Sample for f32 {
    #[inline]
    fn lerp(first: f32, second: f32, numerator: u32, denominator: u32) -> f32 {
        first + (second - first) * numerator as f32 / denominator as f32
    }

    #[inline]
    fn to_f32(self) -> f32 {
        self
    }

    #[inline]
    fn is_zero(self) -> bool {
        2.0 * (self - Self::ZERO_VALUE).abs()
            <= f32::EPSILON * (self.abs() + Self::ZERO_VALUE.abs())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use num_rational::Ratio;
    use quickcheck::{quickcheck, TestResult};

    #[test]
    fn lerp_u16_constraints() {
        let a = 12u16;
        let b = 31u16;
        assert_eq!(Sample::lerp(a, b, 0, 1), a);
        assert_eq!(Sample::lerp(a, b, 1, 1), b);

        assert_eq!(Sample::lerp(0, u16::MAX, 0, 1), 0);
        assert_eq!(Sample::lerp(0, u16::MAX, 1, 1), u16::MAX);
        // Zeroes
        assert_eq!(Sample::lerp(0u16, 0, 0, 1), 0);
        assert_eq!(Sample::lerp(0u16, 0, 1, 1), 0);
        // Downward changes
        assert_eq!(Sample::lerp(1u16, 0, 0, 1), 1);
        assert_eq!(Sample::lerp(1u16, 0, 1, 1), 0);
    }

    #[test]
    fn lerp_i16_constraints() {
        let a = 12i16;
        let b = 31i16;
        assert_eq!(Sample::lerp(a, b, 0, 1), a);
        assert_eq!(Sample::lerp(a, b, 1, 1), b);

        assert_eq!(Sample::lerp(0, i16::MAX, 0, 1), 0);
        assert_eq!(Sample::lerp(0, i16::MAX, 1, 1), i16::MAX);
        assert_eq!(Sample::lerp(0, i16::MIN, 1, 1), i16::MIN);
        // Zeroes
        assert_eq!(Sample::lerp(0u16, 0, 0, 1), 0);
        assert_eq!(Sample::lerp(0u16, 0, 1, 1), 0);
        // Downward changes
        assert_eq!(Sample::lerp(a, i16::MIN, 0, 1), a);
        assert_eq!(Sample::lerp(a, i16::MIN, 1, 1), i16::MIN);
    }

    quickcheck! {
        fn lerp_u16_random(first: u16, second: u16, numerator: u16, denominator: u16) -> TestResult {
            if denominator == 0 { return TestResult::discard(); }

            let (numerator, denominator) = Ratio::new(numerator, denominator).into_raw();
            if numerator > 5000 { return TestResult::discard(); }

            let a = first as f64;
            let b = second as f64;
            let c = numerator as f64 / denominator as f64;
            if c < 0.0 || c > 1.0 { return TestResult::discard(); };
            let reference = a * (1.0 - c) + b * c;
            let x = Sample::lerp(first, second, numerator as u32, denominator as u32) as f64;
            TestResult::from_bool((x - reference).abs() < 1.0)
        }
    }
}
