use std::ops::{Add, Div, Mul, Sub};

/// A real symmetric Cartesian rank-two tensor.
///
/// Components are ordered as `xx`, `yy`, `zz`, `xy`, `xz`, and `yz`. A mass
/// quadrupole represented by this type has SI units of kg m².
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SymmetricTensor3 {
    components: [f64; 6],
}

impl SymmetricTensor3 {
    /// Construct a symmetric tensor from its six independent components.
    #[must_use]
    pub const fn new(xx: f64, yy: f64, zz: f64, xy: f64, xz: f64, yz: f64) -> Self {
        Self {
            components: [xx, yy, zz, xy, xz, yz],
        }
    }

    /// Construct an isotropic tensor with equal diagonal components.
    #[must_use]
    pub const fn isotropic(diagonal: f64) -> Self {
        Self::new(diagonal, diagonal, diagonal, 0.0, 0.0, 0.0)
    }

    /// Return the six independent components in canonical order.
    #[must_use]
    pub const fn components(self) -> [f64; 6] {
        self.components
    }

    /// Return the tensor trace.
    #[must_use]
    pub fn trace(self) -> f64 {
        self.components[0] + self.components[1] + self.components[2]
    }

    /// Return the symmetric trace-free part of the tensor.
    #[must_use]
    pub fn trace_free(self) -> Self {
        self - Self::isotropic(self.trace() / 3.0)
    }

    /// Return whether every component is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.components.iter().all(|value| value.is_finite())
    }

    /// Contract the tensor with two Cartesian vectors, `leftᵢ Tᵢⱼ rightⱼ`.
    #[must_use]
    pub fn contract(self, left: [f64; 3], right: [f64; 3]) -> f64 {
        let [xx, yy, zz, xy, xz, yz] = self.components;
        left[0] * (xx * right[0] + xy * right[1] + xz * right[2])
            + left[1] * (xy * right[0] + yy * right[1] + yz * right[2])
            + left[2] * (xz * right[0] + yz * right[1] + zz * right[2])
    }
}

impl Add for SymmetricTensor3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut components = self.components;
        for (component, rhs_component) in components.iter_mut().zip(rhs.components) {
            *component += rhs_component;
        }
        Self { components }
    }
}

impl Sub for SymmetricTensor3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let mut components = self.components;
        for (component, rhs_component) in components.iter_mut().zip(rhs.components) {
            *component -= rhs_component;
        }
        Self { components }
    }
}

impl Mul<f64> for SymmetricTensor3 {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        let mut components = self.components;
        for component in &mut components {
            *component *= rhs;
        }
        Self { components }
    }
}

impl Div<f64> for SymmetricTensor3 {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        self * (1.0 / rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::SymmetricTensor3;

    #[test]
    fn trace_free_projection_removes_trace() {
        let tensor = SymmetricTensor3::new(4.0, 2.0, -3.0, 1.0, 2.0, 3.0);
        assert!(tensor.trace_free().trace().abs() < 1.0e-14);
    }

    #[test]
    fn contraction_includes_symmetric_off_diagonal_terms() {
        let tensor = SymmetricTensor3::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        let value = tensor.contract([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
        assert!((value - 382.0).abs() < f64::EPSILON);
    }
}
