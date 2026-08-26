# Convergence and orders of accuracy

Every number Hydrax produces from a sampled source is a discretization of an
exact expression. This page records which expression each discrete operator
approximates, the order of accuracy it is designed to have, the order it was
measured to have, and — as importantly — which operators have no convergence
coverage yet.

## Why convergence is checked separately from correctness

Hydrax has two other kinds of numerical test, and neither one can establish an
order of accuracy.

The **acceptance tests** assert exact properties: a spherically symmetric source
radiates nothing, an isotropic trace does not change a waveform, strain scales
as the inverse of distance, the polarizations mix by twice an observer rotation
about the line of sight. Those hold at every resolution, so they say nothing
about the rate at which anything converges.

The **differential tests** assert that each compiled kernel reproduces a
safe-Rust twin. Two implementations of the same wrong stencil agree perfectly,
so agreement between them bounds neither one's accuracy. That is exactly the
gap convergence testing fills: each operator is compared against a closed-form
answer it never sees, on a sequence of refined grids.

## How an observed order is read

On grids whose spacing is halved in turn, an error scaling as \(h^p\) falls by
\(2^p\) per refinement, so \(\log_2(E_\text{coarse}/E_\text{fine})\) estimates \(p\).
Each test computes that estimate for every successive pair in a four-grid
sequence and requires all of them to lie within `0.15` of the designed order.

The band is two-sided on purpose. A lower bound alone — "the error fell by more
than a factor of fifteen" — passes a measurement that has stopped tracking the
discretization at all: an error compared against the wrong point, a norm taken
over a window that shrinks with the grid, an error that has already reached
round-off. Every one of those makes the ratio _larger_, not smaller. Requiring
the observed order to sit in a narrow band around the designed value is what
makes these tests able to fail.

## Where the refinement sequences stop, and why

A centered second-derivative stencil divides by \(\Delta t^2\) and a third-
derivative stencil by \(\Delta t^3\), so their round-off _grows_ under refinement
while their truncation error shrinks. Every sequence here stays in the range
where truncation dominates round-off by several orders of magnitude. Refining
much further would measure the floating-point format rather than the scheme,
and the observed order would fall away from its designed value instead of
approaching it. A sequence extended without checking that is a convergence test
that reports failure for the arithmetic's reasons rather than the code's.

## Time-domain operators

Measured in `crates/hydrax/tests/convergence.rs`, through the `hydrax` facade,
so the accuracy measured is the accuracy a caller gets.

The source is \(Q = A\cos(\omega t)\,\mathrm{diag}(1,-1,0)\) at 800 Hz, observed
off both the pole and the equator so that neither polarization vanishes.

| Quantity                                         | Discretizes                                 | Designed | Observed      |
| ------------------------------------------------ | ------------------------------------------- | -------- | ------------- |
| Waveform, `FiniteDifference::ThreePoint`         | \(h^{TT}_{ij} = 2G\ddot Q^{TT}_{ij}/c^4 D\) | 2        | 1.997 → 2.000 |
| Waveform, `FiniteDifference::FivePoint`          | the same                                    | 4        | 3.991 → 3.999 |
| Luminosity, `ThirdDerivativeStencil::FivePoint`  | \(P = G\,\dddot Q_{ij}\dddot Q_{ij}/5c^5\)  | 2        | 1.977 → 1.999 |
| Luminosity, `ThirdDerivativeStencil::SevenPoint` | the same                                    | 4        | 3.981 → 3.998 |
| Radiated energy, `FivePoint`                     | \(\int P\,\mathrm{d}t\)                     | 2        | 2.000 → 2.000 |
| Radiated energy, `SevenPoint`                    | the same                                    | 2        | 2.046 → 2.003 |

**The anchors.** The analytic polarizations,
\(h_+ = -(GA\omega^2/c^4D)(1+\cos^2 i)\cos 2\phi\cos\omega t\) and
\(h_\times = 2(GA\omega^2/c^4D)\cos i\,\sin 2\phi\cos\omega t\), are projected onto
the transverse basis by hand. So are the luminosity
\(P = 2GA^2\omega^6\sin^2(\omega t)/5c^5\) and its integral
\(\int_0^T P\,\mathrm{d}t \propto T/2 - \sin(2\omega T)/4\omega\). Hydrax computes
none of the four; they are references, not restatements of what the code does.

**Why the energy is second order with a fourth-order stencil.** The energy is a
trapezoidal integral of the luminosity samples, and the trapezoid rule is
second-order accurate. With the five-point stencil that order matches the power
it integrates. With the seven-point stencil it does not, and the quadrature —
not the derivative — is what limits the answer. The test asserts second order
for _both_, which is the statement that the quadrature is the bottleneck: an
implementation that gained fourth-order energy from a fourth-order stencil would
have had to change the quadrature, and the test would notice.

The two error terms are why the seven-point energy sequence starts three
refinements finer than the rest. Its error is \(a\,\Delta t^2 + b\,\Delta t^4\)
with the fourth-order term still the larger of the two on the coarser grids, so
a sequence started there reads an order sliding from about 3.1 down toward 2
while the crossover happens, rather than the flat 2 the trapezoid rule promises.
Fitting the two coefficients to the measured errors predicts observed orders of
2.046, 2.012 and 2.003 on the shifted sequence; those are the values measured,
which is an independent check that the two-term model is the right reading of
the sequence rather than a convenient one.

**The integration window is held fixed.** Centered stencils discard their
boundary radius, so the retained interval would otherwise move by
\(r\,\Delta t\) under refinement and an integral over it would converge at the
rate at which its own limits stopped moving. The sources are extended by exactly
that radius at each end, and the tests assert that the resulting diagnostic
starts at \(t = 0\). The radius is measured through the public API rather than
assumed, so a stencil whose width changed could not silently unpad them.

## Kernel operators

Measured in `crates/hydrax-kernels-sys/tests/convergence.rs`. Every measurement
runs twice, once against the compiled C kernel and once against its safe-Rust
twin, so a convergence claim covers both implementations rather than leaning on
the differential tests to carry it from one to the other.

| Quantity                    | Discretizes                       | Designed | Observed      |
| --------------------------- | --------------------------------- | -------- | ------------- |
| `spherical_flux_divergence` | \(r^{-2}\partial_r(r^2 F)\)       | 2        | 2.008 → 2.002 |
| `minmod_slopes`             | \(\partial_x f\) on monotone data | 1        | 0.995 → 0.999 |

**The anchor** is \(\nabla\cdot F = 2F/r + \partial_r F\) for a purely radial
field, differentiated by hand for a flux whose three conserved components differ
in amplitude, wavenumber and phase, so a kernel that crossed the component
stride would produce a visibly wrong answer rather than a self-consistent one.

**Why the divergence is second order rather than exact.** A cell's interface
fluxes give its _volume-averaged_ divergence exactly, by the divergence theorem.
Reading that average as a value at the cell midpoint is the whole of the
approximation, and it costs exactly two orders. The exact half of that statement
is tested on its own: summing the right-hand side over the domain weighted by
cell volume must leave only the flux through the two outer boundaries, since
every interior interface appears once with each sign. That is an identity rather
than a limit, it holds on a one-cell grid as well as a six-hundred-cell one, and
it is checked to round-off.

**Why first order is the right answer for the limiter.** On smooth monotone
convex data both one-sided differences share a sign, so minmod returns the
smaller of them — the backward difference, which approximates the derivative to
within \(h f''/2\). First order there is not a shortfall: it is the accuracy that
makes a MUSCL reconstruction second-order in space. Second-order convergence
would mean the limiter was not limiting. At a smooth extremum minmod correctly
returns zero instead; measuring an order there would measure the clipping, so
that case is covered by the unit tests rather than here.

## What is not covered

Stated explicitly, because a convergence page that lists only its successes
reads as though everything has been checked.

- **`WaveformSpectrum`.** The discrete transform of a windowed finite record
  approaches the continuous transform at a rate set by leakage and aliasing
  rather than by a fixed power of the sample interval, so there is no single
  order to assert. The spectrum is verified instead against cases where the
  discrete answer is exact: a constant signal's DC integral, an exact-bin
  cosine's two-sided amplitude, and the phase rotation an absolute start time
  imposes.
- **`hll_flux`.** Its accuracy is a property of the reconstruction and flux
  function it is used with, and Hydrax has neither yet — there is no equation of
  state and no hydrodynamic flux in v0.1. Measuring an order for the composed
  scheme (reconstruct with minmod, solve with HLL, take the spherical
  divergence) is the natural next convergence test, and belongs with the
  milestone that introduces the scheme rather than ahead of it.
- **Spatial resolution of a hydrodynamic solution.** There is no evolution to
  refine yet. When there is, the check that matters is a self-convergence study
  of the evolved solution, not of the operators in isolation.

## Adding a convergence test

1. Write down the exact expression the operator approximates and its designed
   order. If the designed order is not known before the measurement, the test
   will end up asserting whatever the code happens to do.
2. Derive the reference by hand, or take it from a published result. A reference
   computed with another part of Hydrax measures self-consistency, not accuracy.
3. Choose a refinement sequence whose leading truncation term stays well above
   both the next term in its series and the accumulated round-off across the
   whole sequence.
4. Hold fixed everything that is not the grid — in particular the interval the
   error norm is taken over.
5. Assert a _band_ around the designed order, not a lower bound.
6. Confirm the test fails on a plausible defect before trusting it. Perturbing a
   stencil coefficient, replacing the quadrature with a lower-order rule, or
   comparing at the wrong point are the checks that were run against these.
