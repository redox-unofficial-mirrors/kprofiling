# redox-kprofilling

Redox kernel profiller

## Usage

This command will extract the symbol table from the kernel binary.

(You need to specify the path for the `kernel` and `ksym`)

```sh
nm -CS /path/to/kernel > /path/to/ksym
```

The following command analyzes the profile data, matches addresses to the symbol table information, and formats the data (`cargo run`) then `inferno-collapse-perf` makes the data hierarchical and `inferno-flamegraph` makes the clickable SVG file.

```sh
cargo run -- /path/to/redoxfs/root/profiling.txt /path/to/ksym OPTIONS FREQ | inferno-collapse-perf | inferno-flamegraph > /path/to/flamegraph.svg
```

- Replace the `OPTIONS` part with a combination of `o`, `x`, and/or `i` (read the source code)
- Replace the `FREQ` part with the CPU frequency measured in GHz (assumes `constant_tsc`, check `lscpu` on Linux)

If you want more details about how to setup the profilling, read [this](https://doc.redox-os.org/book/performance.html#kernel) section.
