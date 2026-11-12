//! Vanilla's `MarsagliaPolarGaussian`: normal deviates generated in pairs.

use cerium_math::mth;

/// The cached second deviate of the Marsaglia polar method.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Gaussian {
    next_next: f64,
    has_next_next: bool,
}

impl Gaussian {
    /// Forgets the cached deviate.
    pub(crate) fn reset(&mut self) {
        self.has_next_next = false;
    }

    /// The next normal deviate, drawing uniform doubles from `next_double` as needed.
    pub(crate) fn next(&mut self, mut next_double: impl FnMut() -> f64) -> f64 {
        if self.has_next_next {
            self.has_next_next = false;
            return self.next_next;
        }
        loop {
            let x = 2.0 * next_double() - 1.0;
            let y = 2.0 * next_double() - 1.0;
            let radius_squared = mth::square_f64(x) + mth::square_f64(y);
            if radius_squared < 1.0 && radius_squared != 0.0 {
                let multiplier = (-2.0 * libm::log(radius_squared) / radius_squared).sqrt();
                self.next_next = y * multiplier;
                self.has_next_next = true;
                return x * multiplier;
            }
        }
    }
}
