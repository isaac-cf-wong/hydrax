# Run configuration files

## Scope

A Hydrax run is described by a set of named settings. They can be given as
command-line flags, or collected in a TOML file and passed with `--config`:

```shell
cargo run -- demo --config galactic.toml > waveform.csv
```

A file makes a run reviewable, diffable, and repeatable, and it keeps a long
list of settings out of a shell history. TOML is the format because the
workspace already builds with it, it is line-oriented enough to review in a
diff, and it needs no schema of its own to be readable.

## Where a setting comes from

Each setting is resolved independently, from the first of these that supplies
it:

1. the command-line flag,
2. the configuration file named by `--config`,
3. the default documented in `--help`.

A flag therefore overrides one setting of a configuration without restating the
rest, and a configuration file need only carry the settings that differ from
the defaults. Because resolution is per setting, an omitted flag is not the
same as a flag given its default value: only the former falls through.

## Keys

The keys are the setting names, which are also the flag names with underscores
in place of dashes and the field names recorded in a provenance sidecar.

| Key               | Unit or values                      | Default      |
| ----------------- | ----------------------------------- | ------------ |
| `duration_s`      | seconds                             | 0.02         |
| `sample_rate_hz`  | hertz                               | 16384        |
| `frequency_hz`    | hertz                               | 800          |
| `amplitude_kg_m2` | kg m²                               | 1e38         |
| `distance_kpc`    | kiloparsecs                         | 10           |
| `inclination_deg` | degrees from the positive z-axis    | 0            |
| `azimuth_deg`     | degrees from the positive x-axis    | 0            |
| `difference`      | `three-point`, `five-point`         | `five-point` |
| `output`          | `time`, `spectrum`, `energy`        | `time`       |
| `window`          | `rectangular`, `hann`, `tukey`      | `hann`       |
| `tukey_alpha`     | taper fraction in [0, 1]            | 0.5          |
| `provenance`      | path of the JSON provenance sidecar | not written  |

An example:

```toml
duration_s = 0.5
sample_rate_hz = 16384.0
frequency_hz = 650.0
amplitude_kg_m2 = 1e38
distance_kpc = 8.5
output = "spectrum"
window = "tukey"
tukey_alpha = 0.25
provenance = "galactic.provenance.json"
```

An unrecognized key is an error naming the key, not a silently ignored setting:
a misspelled key would otherwise leave a run at a default the caller believed
they had changed. The same holds for a value of the wrong type or a malformed
file, and the reported error carries the path of the file and the position of
the offending text.

Settings are validated after resolution, so a value from a file is rejected on
the same terms as a value from a flag.

## Relation to the provenance record

The `run.configuration` object of a
[provenance sidecar](output-formats.md) is the fully resolved configuration:
every setting with its explicit value, whether it came from a flag, a file, or
a default. Its keys are exactly the keys accepted in a configuration file, so a
recorded run can be replayed by handing its recorded configuration back as a
file.

The path of the configuration file is deliberately not recorded. A path
identifies a file that can change or disappear, while the resolved settings it
produced are recorded in full and cannot.

## Verification requirements

The configuration layer is accepted only if it demonstrates:

- resolution of every setting from a configuration file, and of every setting
  from the command line;
- a command-line flag overriding every setting a configuration file supplies,
  with the settings it does not name still taken from the file;
- the documented defaults in `--help` agreeing with the defaults the resolution
  actually applies;
- rejection of an unknown key by name, of a value of the wrong type, of a
  malformed file, and of a missing file, each naming the file;
- validation of a configured setting on the same terms as a flag; and
- a recorded configuration reloading as a configuration file and resolving to
  the run it was recorded from.
