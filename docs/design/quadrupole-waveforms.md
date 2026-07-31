# Quadrupole waveform design

## Scope

The first Hydrax waveform model uses the leading-order slow-motion, weak-field
mass-quadrupole approximation. It accepts a uniformly sampled Cartesian source
quadrupole and returns the two dimensionless polarizations seen by a distant
observer.

This model does not evolve a fluid or spacetime. It is a verified extraction
foundation for later axisymmetric and three-dimensional source simulations.

## Source convention

The symmetric trace-free mass quadrupole is conventionally

\[
Q_{ij} = \int \rho
\left(x_i x_j - \frac{1}{3}\delta_{ij}r^2\right)\,d^3x .
\]

Hydrax accepts all six independent components in kg m². The input does not have
to be trace-free: an isotropic trace is removed automatically by contraction
with the transverse-traceless polarization tensors.

The leading-order wave field at distance \(D\) is

\[
h_{ij}^{TT} = \frac{2G}{c^4D}\ddot Q_{ij}^{TT}.
\]

The approximation requires internal source velocities small compared with the
speed of light, weak self-gravity in the extraction formula, and an observer in
the radiation zone. Later relativistic milestones will require independent
wave-extraction methods.

## Observer and polarization convention

Inclination \(\theta\) is measured from the positive z-axis. Azimuth \(\phi\)
is measured from the positive x-axis toward the positive y-axis. Hydrax uses

\[
\begin{aligned}
\mathbf p &= (\cos\theta\cos\phi,\;\cos\theta\sin\phi,\;-\sin\theta),\\
\mathbf q &= (-\sin\phi,\;\cos\phi,\;0),
\end{aligned}
\]

and evaluates

\[
\begin{aligned}
h_+ &= \frac{G}{c^4D}
\left(p_i p_j-q_i q_j\right)\ddot Q_{ij},\\
h_\times &= \frac{2G}{c^4D}p_i q_j\ddot Q_{ij}.
\end{aligned}
\]

Changing the handedness or rotating \(\mathbf p\) and \(\mathbf q\) changes the
polarization convention but not the physical tensor field. The convention must
therefore accompany exported data.

## Numerical differentiation

Hydrax initially provides centered three-point and five-point approximations:

\[
f''_n = \frac{f_{n-1}-2f_n+f_{n+1}}{\Delta t^2} + O(\Delta t^2),
\]

\[
f''_n = \frac{-f_{n-2}+16f_{n-1}-30f_n+16f_{n+1}-f_{n+2}}
{12\Delta t^2} + O(\Delta t^4).
\]

Hydrax discards one or two samples at each boundary instead of silently using a
lower-order one-sided formula. Returned waveform times are shifted to the first
valid centered sample. They remain source retarded times; the constant
propagation delay \(D/c\) is not added.

Differentiation amplifies high-frequency noise. Simulation-derived quadrupoles
will therefore need convergence studies and, eventually, a stress-formula
implementation that avoids taking two numerical time derivatives.

## Frequency-domain convention

For \(N\) samples separated by \(\Delta t\), Hydrax approximates the continuous
Fourier transform

\[
\widetilde h(f_k) = \Delta t\sum_{n=0}^{N-1} w_n h_n
e^{-2\pi i f_k(t_0+n\Delta t)},
\qquad f_k=\frac{k}{N\Delta t}.
\]

The output contains bins \(0\leq k\leq\lfloor N/2\rfloor\). These values are the
nonnegative half of the two-sided transform: Hydrax does not double interior
bins. Since strain is dimensionless, each complex Fourier amplitude has units
of seconds. The absolute start time \(t_0\) is retained as a spectral phase
factor.

Hydrax provides rectangular, symmetric Hann, and symmetric Tukey windows. The
transform is the Fourier transform of the windowed data and is not divided by
the window's coherent gain. That gain is reported separately so amplitude
corrections can be made explicitly when appropriate. A Tukey taper fraction of
zero is rectangular, while one is Hann.

## Verification requirements

The time-domain implementation is accepted only if it demonstrates:

- zero strain for an arbitrary spherical pulsation;
- analytic amplitude and phase for a sinusoidal trace-free source;
- inverse-distance scaling;
- invariance under addition of an isotropic trace;
- the advertised finite-difference convergence order; and
- rejection of non-finite samples and invalid observer distances.
- exact discrete-transform normalization for DC and bin-centered sinusoids;
- preservation of magnitude and rotation of phase under a time-origin shift;
- agreement of Tukey's limiting cases with rectangular and Hann windows; and
- rejection of invalid window parameters.

Radiated-energy estimates, file formats, and provenance records are subsequent
parts of the first milestone.
