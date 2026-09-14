# Standalone native collector

Retains `rust-native-production-mir-block/1` and the existing native driver,
normalizer, re-export verification and Core policy bridge. The Rust launcher
embeds our precompiled driver, adapter sources, schemas and license notices.
It checks those bytes before running external isolated Python (`-I -S -B`).

See the [delivery and installation contract](../../../docs/quality/native-external-toolchain.md).
No compiler, interpreter, linker or LLVM distribution is included.

Build with external prerequisites already installed:

```sh
RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$PWD/target/native-driver" \
  cargo +1.97.1 build --manifest-path tools/quality/rust-native-driver/Cargo.toml --release --locked
python3 tools/quality/rust-native-plugin/build.py \
  --driver target/native-driver/release/harness-gate-rust-native-driver \
  --rustc "$(rustup which --toolchain 1.97.1 rustc)" \
  --output target/native-package
```

Build-only components: `rustc-dev`, `rust-docs` (standard-library notices),
GNU `strip`; fetch locked Cargo dependencies before offline packaging. Runtime
components: the matching compiler/sysroot, `llvm-tools-preview`, Python 3.12+
and the project's native build dependencies. The builder authenticates crate
sources/notices against Cargo.lock and writes a CycloneDX SBOM using Core's tool.

Run mandatory acceptance (missing inputs or skipped tests fail):

```sh
NATIVE_PLUGIN_BINARY="$PWD/target/native-package/harness-gate-rust-collector-linux-amd64" \
NATIVE_DRIVER="$PWD/target/native-driver/release/harness-gate-rust-native-driver" \
NATIVE_DRIVER_SYSROOT="$(rustc +1.97.1 --print sysroot)" \
HARNESS_GATE_NATIVE_POLICY_BINARY="$(command -v harness-gate)" \
  python3 tools/quality/rust-native-plugin/accept.py
```

Use a fresh package output. `Cargo.toml` declares the independently released
version; the std-only launcher is compiled by `build.py`, not Cargo. The driver
has its own unchanged Cargo manifest/lockfile. An arbitrary `--driver` supplied
to a local build is not release provenance: the hosted release always builds it
from the selected commit before packaging and testing those exact bytes.
