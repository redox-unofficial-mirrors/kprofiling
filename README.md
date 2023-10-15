# Usage

```bash

nm -CS /path/to/kernel >/path/to/ksym

# OPTIONS: a combination of o, x, and/or i (see source code)
# FREQ: CPU frequency in GHz (assumes constant_tsc, check lscpu on Linux)

cargo run -- /path/to/redoxfs/root/profiling.txt /path/to/ksym OPTIONS FREQ | inferno-collapse-perf | inferno-flamegraph >/path/to/flamegraph.svg

```
