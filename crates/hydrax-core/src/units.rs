//! SI constants and geometrized-unit conversions.
//!
//! Hydrax stores, reports, and validates every physical quantity in SI units.
//! Relativistic formulations are far more convenient in geometrized units,
//! which set G = c = 1 and so measure mass, time, and energy as lengths. This
//! module is the single source of truth for both the SI constants and the
//! conversions between the two systems, so no call site has to spell out a
//! conversion factor.
//!
//! A geometrized quantity is expressed here in metres raised to whatever power
//! the quantity requires:
//!
//! | Quantity     | SI unit  | Geometrized unit | Conversion  |
//! | ------------ | -------- | ---------------- | ----------- |
//! | length       | m        | m                | identity    |
//! | mass         | kg       | m                | G m / c²    |
//! | time         | s        | m                | c t         |
//! | energy       | J        | m                | G E / c⁴    |
//! | mass density | kg m⁻³   | m⁻²              | G ρ / c²    |
//! | pressure     | Pa       | m⁻²              | G p / c⁴    |
//! | speed        | m s⁻¹    | dimensionless    | v / c       |
//! | luminosity   | W        | dimensionless    | G P / c⁵    |
//!
//! Every conversion comes as a pair: `*_to_geometrized_*` maps SI into the
//! geometrized system and `*_from_geometrized_*` maps back.

/// Newtonian gravitational constant in m³ kg⁻¹ s⁻².
///
/// CODATA 2018 recommended value.
pub const GRAVITATIONAL_CONSTANT_M3_KG_S2: f64 = 6.674_30e-11;

/// Speed of light in vacuum in m s⁻¹.
///
/// Exact by the SI definition of the metre.
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// Nominal solar mass parameter GM in m³ s⁻².
///
/// IAU 2015 Resolution B3 nominal value. The product is measured to nine
/// significant figures while the gravitational constant alone is known to five,
/// so Hydrax carries the product and derives the mass from it rather than the
/// other way round.
pub const SOLAR_MASS_PARAMETER_M3_S2: f64 = 1.327_124_4e20;

/// Solar mass in kilograms.
///
/// Derived from [`SOLAR_MASS_PARAMETER_M3_S2`] and
/// [`GRAVITATIONAL_CONSTANT_M3_KG_S2`], and therefore no more precise than the
/// gravitational constant.
pub const SOLAR_MASS_KG: f64 = SOLAR_MASS_PARAMETER_M3_S2 / GRAVITATIONAL_CONSTANT_M3_KG_S2;

/// One parsec in metres, using the IAU 2015 nominal conversion.
pub const PARSEC_M: f64 = 3.085_677_581_491_367e16;

/// One kiloparsec in metres.
pub const KILOPARSEC_M: f64 = 1000.0 * PARSEC_M;

/// Squared speed of light in m² s⁻².
const SPEED_OF_LIGHT_SQUARED: f64 = SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S;

/// Fourth power of the speed of light in m⁴ s⁻⁴.
const SPEED_OF_LIGHT_FOURTH: f64 = SPEED_OF_LIGHT_SQUARED * SPEED_OF_LIGHT_SQUARED;

/// Fifth power of the speed of light in m⁵ s⁻⁵.
const SPEED_OF_LIGHT_FIFTH: f64 = SPEED_OF_LIGHT_FOURTH * SPEED_OF_LIGHT_M_S;

/// Convert a distance in parsecs to metres.
#[must_use]
pub const fn parsecs_to_m(distance_pc: f64) -> f64 {
    distance_pc * PARSEC_M
}

/// Convert a distance in metres to parsecs.
#[must_use]
pub const fn m_to_parsecs(distance_m: f64) -> f64 {
    distance_m / PARSEC_M
}

/// Convert a distance in kiloparsecs to metres.
///
/// ```
/// use hydrax_core::{KILOPARSEC_M, kiloparsecs_to_m};
///
/// assert!((kiloparsecs_to_m(10.0) - 10.0 * KILOPARSEC_M).abs() < f64::EPSILON);
/// ```
#[must_use]
pub const fn kiloparsecs_to_m(distance_kpc: f64) -> f64 {
    distance_kpc * KILOPARSEC_M
}

/// Convert a distance in metres to kiloparsecs.
#[must_use]
pub const fn m_to_kiloparsecs(distance_m: f64) -> f64 {
    distance_m / KILOPARSEC_M
}

/// Convert a mass in kilograms to its geometrized length G m / c² in metres.
///
/// The geometrized solar mass is half the Sun's Schwarzschild radius:
///
/// ```
/// use hydrax_core::{SOLAR_MASS_KG, mass_to_geometrized_m};
///
/// let length_m = mass_to_geometrized_m(SOLAR_MASS_KG);
/// assert!((length_m - 1476.625).abs() < 0.001);
/// ```
#[must_use]
pub const fn mass_to_geometrized_m(mass_kg: f64) -> f64 {
    mass_kg * GRAVITATIONAL_CONSTANT_M3_KG_S2 / SPEED_OF_LIGHT_SQUARED
}

/// Convert a geometrized mass in metres to kilograms.
#[must_use]
pub const fn mass_from_geometrized_m(mass_m: f64) -> f64 {
    mass_m * SPEED_OF_LIGHT_SQUARED / GRAVITATIONAL_CONSTANT_M3_KG_S2
}

/// Convert a time in seconds to its geometrized length c t in metres.
#[must_use]
pub const fn time_to_geometrized_m(time_s: f64) -> f64 {
    time_s * SPEED_OF_LIGHT_M_S
}

/// Convert a geometrized time in metres to seconds.
#[must_use]
pub const fn time_from_geometrized_m(time_m: f64) -> f64 {
    time_m / SPEED_OF_LIGHT_M_S
}

/// Convert an energy in joules to its geometrized length G E / c⁴ in metres.
#[must_use]
pub const fn energy_to_geometrized_m(energy_j: f64) -> f64 {
    energy_j * GRAVITATIONAL_CONSTANT_M3_KG_S2 / SPEED_OF_LIGHT_FOURTH
}

/// Convert a geometrized energy in metres to joules.
#[must_use]
pub const fn energy_from_geometrized_m(energy_m: f64) -> f64 {
    energy_m * SPEED_OF_LIGHT_FOURTH / GRAVITATIONAL_CONSTANT_M3_KG_S2
}

/// Convert a mass density in kg m⁻³ to its geometrized value G ρ / c² in m⁻².
#[must_use]
pub const fn density_to_geometrized_per_m2(density_kg_m3: f64) -> f64 {
    density_kg_m3 * GRAVITATIONAL_CONSTANT_M3_KG_S2 / SPEED_OF_LIGHT_SQUARED
}

/// Convert a geometrized mass density in m⁻² to kg m⁻³.
#[must_use]
pub const fn density_from_geometrized_per_m2(density_per_m2: f64) -> f64 {
    density_per_m2 * SPEED_OF_LIGHT_SQUARED / GRAVITATIONAL_CONSTANT_M3_KG_S2
}

/// Convert a pressure in pascals to its geometrized value G p / c⁴ in m⁻².
#[must_use]
pub const fn pressure_to_geometrized_per_m2(pressure_pa: f64) -> f64 {
    pressure_pa * GRAVITATIONAL_CONSTANT_M3_KG_S2 / SPEED_OF_LIGHT_FOURTH
}

/// Convert a geometrized pressure in m⁻² to pascals.
#[must_use]
pub const fn pressure_from_geometrized_per_m2(pressure_per_m2: f64) -> f64 {
    pressure_per_m2 * SPEED_OF_LIGHT_FOURTH / GRAVITATIONAL_CONSTANT_M3_KG_S2
}

/// Convert a speed in m s⁻¹ to its dimensionless geometrized value v / c.
#[must_use]
pub const fn speed_to_geometrized(speed_m_s: f64) -> f64 {
    speed_m_s / SPEED_OF_LIGHT_M_S
}

/// Convert a dimensionless geometrized speed to m s⁻¹.
#[must_use]
pub const fn speed_from_geometrized(speed: f64) -> f64 {
    speed * SPEED_OF_LIGHT_M_S
}

/// Convert a luminosity in watts to its dimensionless geometrized value
/// G P / c⁵.
#[must_use]
pub const fn power_to_geometrized(power_w: f64) -> f64 {
    power_w * GRAVITATIONAL_CONSTANT_M3_KG_S2 / SPEED_OF_LIGHT_FIFTH
}

/// Convert a dimensionless geometrized luminosity to watts.
#[must_use]
pub const fn power_from_geometrized(power: f64) -> f64 {
    power * SPEED_OF_LIGHT_FIFTH / GRAVITATIONAL_CONSTANT_M3_KG_S2
}

#[cfg(test)]
mod tests {
    use super::{
        GRAVITATIONAL_CONSTANT_M3_KG_S2, KILOPARSEC_M, PARSEC_M, SOLAR_MASS_KG,
        SOLAR_MASS_PARAMETER_M3_S2, SPEED_OF_LIGHT_M_S, density_from_geometrized_per_m2,
        density_to_geometrized_per_m2, energy_from_geometrized_m, energy_to_geometrized_m,
        kiloparsecs_to_m, m_to_kiloparsecs, m_to_parsecs, mass_from_geometrized_m,
        mass_to_geometrized_m, parsecs_to_m, power_from_geometrized, power_to_geometrized,
        pressure_from_geometrized_per_m2, pressure_to_geometrized_per_m2, speed_from_geometrized,
        speed_to_geometrized, time_from_geometrized_m, time_to_geometrized_m,
    };

    /// Assert that two nonzero values agree to a relative tolerance.
    fn assert_close(actual: f64, expected: f64, relative_tolerance: f64) {
        let difference = (actual - expected).abs();
        assert!(
            difference <= relative_tolerance * expected.abs(),
            "expected {expected}, got {actual}, relative difference \
             {relative}, tolerance {relative_tolerance}",
            relative = difference / expected.abs()
        );
    }

    /// A few ulps of headroom for a conversion and its inverse.
    const ROUND_TRIP_TOLERANCE: f64 = 1.0e-15;

    #[test]
    fn constants_agree_with_published_values() {
        // Published values Hydrax does not compute: the IAU 2015 nominal solar
        // mass and the geometrized solar mass and Schwarzschild radius quoted
        // throughout the relativity literature as 1.4766 km and 2.953 km.
        assert_close(SOLAR_MASS_KG, 1.988_41e30, 1.0e-6);
        assert_close(mass_to_geometrized_m(SOLAR_MASS_KG), 1476.6, 1.0e-4);
        assert_close(2.0 * mass_to_geometrized_m(SOLAR_MASS_KG), 2953.2, 1.0e-4);
        // The kiloparsec is a thousand parsecs and nothing else.
        assert_close(KILOPARSEC_M, 1000.0 * PARSEC_M, 0.0);
        // The mass parameter is the product Hydrax carries, not a third value.
        assert_close(
            SOLAR_MASS_KG * GRAVITATIONAL_CONSTANT_M3_KG_S2,
            SOLAR_MASS_PARAMETER_M3_S2,
            ROUND_TRIP_TOLERANCE,
        );
    }

    #[test]
    fn distance_conversions_invert_each_other() {
        for distance_kpc in [1.0e-6, 1.0, 8.5, 10.0, 1.0e6] {
            assert_close(
                m_to_kiloparsecs(kiloparsecs_to_m(distance_kpc)),
                distance_kpc,
                ROUND_TRIP_TOLERANCE,
            );
            assert_close(
                m_to_parsecs(parsecs_to_m(distance_kpc)),
                distance_kpc,
                ROUND_TRIP_TOLERANCE,
            );
            // A kiloparsec conversion is a parsec conversion scaled by 1000.
            assert_close(
                kiloparsecs_to_m(distance_kpc),
                1000.0 * parsecs_to_m(distance_kpc),
                ROUND_TRIP_TOLERANCE,
            );
        }
    }

    #[test]
    fn geometrized_conversions_invert_each_other() {
        // Astrophysical magnitudes: a solar mass, a millisecond, a supernova
        // energy, nuclear density, the pressure at that density, a tenth of the
        // speed of light, and a gravitational-wave luminosity.
        for value in [1.0e-30, 1.0e-3, 1.0, 1.0e3, 1.0e30, 1.0e45] {
            assert_close(
                mass_from_geometrized_m(mass_to_geometrized_m(value)),
                value,
                ROUND_TRIP_TOLERANCE,
            );
            assert_close(
                time_from_geometrized_m(time_to_geometrized_m(value)),
                value,
                ROUND_TRIP_TOLERANCE,
            );
            assert_close(
                energy_from_geometrized_m(energy_to_geometrized_m(value)),
                value,
                ROUND_TRIP_TOLERANCE,
            );
            assert_close(
                density_from_geometrized_per_m2(density_to_geometrized_per_m2(value)),
                value,
                ROUND_TRIP_TOLERANCE,
            );
            assert_close(
                pressure_from_geometrized_per_m2(pressure_to_geometrized_per_m2(value)),
                value,
                ROUND_TRIP_TOLERANCE,
            );
            assert_close(
                speed_from_geometrized(speed_to_geometrized(value)),
                value,
                ROUND_TRIP_TOLERANCE,
            );
            assert_close(
                power_from_geometrized(power_to_geometrized(value)),
                value,
                ROUND_TRIP_TOLERANCE,
            );
        }
    }

    #[test]
    fn light_travels_one_geometrized_metre_per_metre() {
        assert_close(speed_to_geometrized(SPEED_OF_LIGHT_M_S), 1.0, 0.0);
        // One second of light travel is one light-second of length.
        assert_close(
            time_to_geometrized_m(1.0),
            SPEED_OF_LIGHT_M_S,
            ROUND_TRIP_TOLERANCE,
        );
    }

    #[test]
    fn geometrized_density_is_geometrized_mass_per_volume() {
        // Each conversion carries its own power of c, so deriving a geometrized
        // density from a geometrized mass and an unconverted volume fails
        // unless the two powers agree.
        let volume_m3 = 1.0e12;
        for mass_kg in [1.0e20, SOLAR_MASS_KG, 1.0e35] {
            assert_close(
                density_to_geometrized_per_m2(mass_kg / volume_m3),
                mass_to_geometrized_m(mass_kg) / volume_m3,
                ROUND_TRIP_TOLERANCE,
            );
        }
    }

    #[test]
    fn geometrized_pressure_is_geometrized_energy_per_volume() {
        let volume_m3 = 1.0e12;
        for pressure_pa in [1.0e20, 1.0e30, 1.0e35] {
            assert_close(
                pressure_to_geometrized_per_m2(pressure_pa),
                energy_to_geometrized_m(pressure_pa * volume_m3) / volume_m3,
                ROUND_TRIP_TOLERANCE,
            );
        }
    }

    #[test]
    fn geometrized_luminosity_is_geometrized_energy_per_geometrized_time() {
        let time_s = 0.02;
        for power_w in [1.0e30, 1.0e40, 1.0e46] {
            assert_close(
                power_to_geometrized(power_w),
                energy_to_geometrized_m(power_w * time_s) / time_to_geometrized_m(time_s),
                ROUND_TRIP_TOLERANCE,
            );
        }
    }
}
