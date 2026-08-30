# Accepted Phase 1 Baseline

This is the first accepted `main` quality-baseline evidence package. It was
captured from commit `b92755f9ec87c251516863a0136c28f811187ab5` by the green
CI run [33223804928](https://github.com/musutrade/Harness-Gate/actions/runs/33223804928)
on 2026-08-29.

The committed summary is [current.json](current.json) with the reviewable
Markdown companion [current.md](current.md). The CI artifact
`quality-baseline-33223804928` contains the raw per-sample reports. The same
run also published the coverage, cross-platform contract, and documentation
consistency artifacts.

Baseline series:

`x86_64-unknown-linux-gnu:rustc 1.98.0 (88d9e12ae 2026-08-18):1:1:cold-and-warm`

The five-sample Linux verification medians are 0.656s serial and 0.355s
parallel with a configured and observed peak of two workers. The test warm
median is 4.344s after a 32.387s cold sample. The Linux release-small binary is
7,953,272 bytes with SHA-256
`2cad70d1664f907125571106f07c1fd74950b0d4874a85328e0e2dc35b797af4`.

The pull-request `quality-baseline` matrix also captures the same benchmark and
binary-size evidence on macOS and Windows. Those artifacts are platform-local
series and are not compared numerically with this Linux canonical record.

Future baseline updates must be raised as reviewed pull requests by the
scheduled/manual refresh workflow. A local checkout containing unrelated
project audit rules may fail audit fixture tests; the clean-checkout CI run is
the authoritative baseline evidence for this package.
