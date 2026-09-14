# Repository execution observer

This Linux-only Rust helper is development/CI infrastructure. It is not a plugin
runtime dependency or a release payload component. Build with the current stable
Rust toolchain, then set `HARNESS_EXEC_AUDIT` to the absolute helper executable.

```sh
CARGO_TARGET_DIR="$PWD/target/exec-audit" cargo +1.98.1 build \
  --manifest-path tools/quality/exec-audit/Cargo.toml --release --locked
export HARNESS_EXEC_AUDIT="$PWD/target/exec-audit/release/harness-gate-exec-audit"
python3 tools/quality/exec-audit/test_acceptance.py
"$HARNESS_EXEC_AUDIT" new-events.jsonl -- cargo build --release
```

## Why syscall text was replaced

PR #261 CI runs 34787135013 and 34789368051 failed on `???(` fragments at
thread exit. `strace -q` changed the suffix from detached to unfinished without
fixing register-read races. The retained failing traces remain invalid evidence.
The Rust `audit-probe race` fixture reproduces the same class of failure: 100
processes each start six threads and exit their group. In the operator's Ubuntu
24.04 run, strace 6.8 produced 118 unknown fragments on this fixture; the new
observer completed on the same fixture without any unknown record.

The observer uses Linux PTRACE_O_TRACEEXEC, TRACEFORK, TRACEVFORK and TRACECLONE.
At PTRACE_EVENT_EXEC, the new image has been installed but has not returned to
user space. It reads `/proc/PID/exe` and the complete NUL-separated cmdline at
that stop. It does not request syscall-entry/exit stops or read registers of
exiting threads. Nonleader exec and execveat are covered by real tests.
See the [Linux ptrace API](https://man7.org/linux/man-pages/man2/ptrace.2.html),
particularly PTRACE_EVENT_EXEC and execve under ptrace.

The JSONL contract explicitly covers **successful program executions**, including
interpreters and descendants. Failed executable lookups are not executions and
are not represented. This is a changed observation format, not a claim that old
strace evidence has been repaired. Legacy text parsing still rejects unknown,
detached, truncated and dangling exec records.

A complete stream has a versioned start record, one record per successful exec,
and a completion record after all traced children terminate. Read failures,
non-UTF8/oversized arguments, unexpected events and missing completion fail. Logs
are created exclusively; existing evidence is never overwritten. EXITKILL kills
tracees if the observer exits unexpectedly. Normal target exit status is preserved; signal termination is reported as
128 plus the signal number. Interruption tests kill the downloading target while
keeping the observer alive. Killing the observer itself leaves an incomplete
stream which is rejected, and EXITKILL terminates its tracees.

This is not an adversarial sandbox or an environment/library-load attestation.
It needs permitted ptrace and procfs access. It does not certify code inside an
already running interpreter, adversarial CLONE_UNTRACED use or arbitrary process
job-control semantics. Those were not established by the prior strace audit.
The helper build itself is outside the observed candidate build; its source and
lockfile are reviewed and its real acceptance is a required CI step. Raw event
files retain the existing `.execve` filenames, with a self-identifying schema.
