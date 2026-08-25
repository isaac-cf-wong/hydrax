# Units and numerical conventions

## Scope

Every value that crosses the Hydrax public surface is an SI value: metres,
kilograms, seconds, and the units built from them. Column names, error
messages, configuration keys, and struct fields carry the unit in their name so
that a number never travels without it.

Relativistic formulations are far more convenient in _geometrized_ units, which
set `G = c = 1`. The later milestones — relativistic collapse, spacetime
evolution, and their equations of state — will use them internally. Hydrax
therefore keeps the two systems apart deliberately: SI at the boundary,
geometrized inside, with one conversion layer between them.

## Constants

The constants live once, in `hydrax-core`, and every other crate reads them
from there. A constant that follows from another is derived rather than
restated, so the two can never disagree.

| Constant                          | Value                          | Source                                           |
| --------------------------------- | ------------------------------ | ------------------------------------------------ |
| `GRAVITATIONAL_CONSTANT_M3_KG_S2` | 6.674 30 × 10⁻¹¹ m³ kg⁻¹ s⁻²   | CODATA 2018 recommended value                    |
| `SPEED_OF_LIGHT_M_S`              | 299 792 458 m s⁻¹              | Exact, by the SI definition of the metre         |
| `SOLAR_MASS_PARAMETER_M3_S2`      | 1.327 124 4 × 10²⁰ m³ s⁻²      | IAU 2015 Resolution B3 nominal value             |
| `SOLAR_MASS_KG`                   | 1.988 41 × 10³⁰ kg             | Derived: mass parameter / gravitational constant |
| `PARSEC_M`                        | 3.085 677 581 491 367 × 10¹⁶ m | IAU 2015 nominal conversion                      |
| `KILOPARSEC_M`                    | 10³ parsecs                    | Derived: one thousand `PARSEC_M`                 |

The solar mass parameter is stored rather than the solar mass because the
product `GM` is measured to nine significant figures while the gravitational
constant alone is known to five. A derived `SOLAR_MASS_KG` is therefore no more
precise than `G`, and everything that needs a solar mass in a relativistic
formula uses the product and never divides by `G` at all.

## Geometrized units

Setting `G = c = 1` leaves one dimension, and Hydrax measures it in metres. A
quantity's geometrized value is its SI value multiplied by whichever power of
`G` and `c` makes the result a power of a length:

| Quantity     | SI unit | Geometrized unit | Conversion |
| ------------ | ------- | ---------------- | ---------- |
| length       | m       | m                | identity   |
| mass         | kg      | m                | G m / c²   |
| time         | s       | m                | c t        |
| energy       | J       | m                | G E / c⁴   |
| mass density | kg m⁻³  | m⁻²              | G ρ / c²   |
| pressure     | Pa      | m⁻²              | G p / c⁴   |
| speed        | m s⁻¹   | dimensionless    | v / c      |
| luminosity   | W       | dimensionless    | G P / c⁵   |

Each conversion is a pair of functions in `hydrax-core`: `*_to_geometrized_*`
maps an SI value into the geometrized system and `*_from_geometrized_*` maps it
back. No conversion factor is written out at a call site, so a power of `c`
cannot be dropped in one place and kept in another.

Geometrized mass and energy carry the same unit as length, and geometrized
density and pressure the same unit as inverse area, because in this system they
are the same kind of quantity. That is the property the conversions are checked
against: a geometrized density equals a geometrized mass divided by an
unconverted volume, a geometrized pressure equals a geometrized energy divided
by an unconverted volume, and a geometrized luminosity equals a geometrized
energy divided by a geometrized time. Each identity fails unless the powers of
`c` in two separate conversions agree.

Two familiar values anchor the mass conversion. The geometrized solar mass
`GM/c²` is 1.4766 km and the Sun's Schwarzschild radius `2GM/c²` is 2.9533 km;
both are quoted throughout the relativity literature and neither is computed by
Hydrax.

## Distances

Source distances are quoted in parsecs or kiloparsecs, since a Galactic
core-collapse supernova sits at a few to a few tens of kiloparsecs. The
conversion to metres belongs to `hydrax-core` alongside the constant it uses;
no caller multiplies by a thousand of its own.

## Verification requirements

The conversion layer is accepted only if it demonstrates:

- agreement of every stored constant with its published value, and of every
  derived constant with the constant it is derived from;
- recovery of the original value by each conversion followed by its inverse,
  across the astrophysical range of magnitudes;
- the cross-quantity identities above, which fix the relative powers of `c` in
  separate conversions;
- the published geometrized solar mass and solar Schwarzschild radius; and
- inverse-distance scaling of a waveform placed at a distance given in
  kiloparsecs, checked against a strain computed from published constants
  rather than from Hydrax's own.

The waveform conventions that use these units are documented in
[quadrupole waveform design](quadrupole-waveforms.md).
