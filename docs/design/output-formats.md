# Output formats and provenance

## Scope

Hydrax v0.1 streams numerical results as UTF-8 CSV and can write a separate
UTF-8 JSON provenance record. Keeping the provenance in a sidecar preserves a
plain tabular stream for plotting and Unix pipelines while giving every saved
dataset an unambiguous, machine-readable description.

Use the two files as a pair:

```shell
cargo run -- demo --provenance waveform.provenance.json > waveform.csv
```

Hydrax does not infer a relationship from filenames. Workflows should move,
archive, and publish the CSV and its sidecar together.

## CSV schemas

All CSV files have one header row, no comments, comma delimiters, and decimal
floating-point values. SI units are encoded in column names. Complex spectra
are represented by separate real and imaginary columns.

| Schema identifier                 | Columns, in order                                                                              |
| --------------------------------- | ---------------------------------------------------------------------------------------------- |
| `hydrax.waveform.time.csv.v1`     | `time_s`, `h_plus`, `h_cross`                                                                  |
| `hydrax.waveform.spectrum.csv.v1` | `frequency_hz`, `h_plus_real_s`, `h_plus_imaginary_s`, `h_cross_real_s`, `h_cross_imaginary_s` |
| `hydrax.waveform.energy.csv.v1`   | `start_time_s`, `end_time_s`, `total_energy_j`, `average_power_w`                              |

Time-domain strains are dimensionless. Spectrum components have units of
seconds under Hydrax's continuous-Fourier-transform convention. The energy
schema currently contains one summary row.

The final integer in a schema identifier is its major version. Changing a
column's meaning, unit, order, or required presence increments that version.

## JSON provenance schema

The top-level `schema` value is `hydrax.provenance.v1`. Its fields are:

| Object     | Recorded information                                                                                                                                               |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `software` | Package name and version, repository, Git revision, and the state of the build source tree.                                                                        |
| `build`    | Rust compiler, compilation target and profile, enabled Cargo features, floating-point precision, and every resolved dependency name and version from `Cargo.lock`. |
| `run`      | Command name and the complete parsed runtime configuration, including explicit defaults and the provenance destination.                                            |
| `output`   | Media type and the exact CSV schema identifier selected by the command.                                                                                            |

`source_state` takes one of four values:

| Value      | Meaning                                                                                                                                              |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `clean`    | The workspace was a Git checkout matching `source_revision` exactly.                                                                                 |
| `modified` | The workspace was a Git checkout with uncommitted or untracked changes.                                                                              |
| `packaged` | The build used a published `.crate`; `source_revision` comes from its packaged VCS metadata and there was no working tree that could differ from it. |
| `unknown`  | Neither a Git checkout nor packaged metadata was available.                                                                                          |

When Hydrax is built outside a Git checkout, `source_revision` can likewise be
`unknown`; the package version and build information remain available. The
source state describes the tree at compilation time, not at execution time, and
it describes the whole workspace rather than any single crate.

Untracked changes include a Cargo output directory placed inside the repository
without being ignored, which is what `CARGO_TARGET_DIR` does when pointed at an
unignored path in the tree. A build whose provenance must read `clean` therefore
keeps its output directory outside the workspace, or ignored within it.

Consumers should treat `source_state` as an open vocabulary and accept an
unrecognized value rather than failing, in line with the field-compatibility
rule below.

Consumers must reject an unsupported provenance major version. Within a major
version, they should ignore fields they do not recognize so compatible
metadata can be added later.

## Reproducibility limits

The record identifies the program and inputs but does not promise bitwise
identity across compilers, targets, math libraries, or future parallel
execution. Scientific comparisons must use the tolerances and convergence
criteria documented for the relevant physics module. Future simulation
milestones will extend the record with grid and solver settings, EOS and
opacity checksums, and restart lineage.
