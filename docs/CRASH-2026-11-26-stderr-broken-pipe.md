# Tidal daemon broken pipe crash investigation

On 26 November 2026, `tidal-daemon` on Rimegale aborted while logging an IPC `get_status` request. The core contains the Rust panic `failed printing to stderr: Broken pipe (os error 32)`. The immediate failure is established; the likely trigger was an Omarchy shell restart closing the daemon's stderr pipe.

## Incident details

| Field | Recorded value |
| --- | --- |
| Host | Rimegale, Arch Linux with Omarchy |
| Plugin | `jaj.tidal`, installed manifest version `1.0.78` |
| Process and command line | `/home/jaj/.config/omarchy/plugins/jaj.tidal/bin/tidal-daemon` |
| PID | `183755` |
| Crashing thread | `188252` |
| Signal | `SIGABRT` (6), `SI_TKILL` |
| Core timestamp | 2026-11-26 21:05:22 CET |
| Completed coredump journal entry | 2026-11-26 21:05:23 CET |

## Evidence and mechanism

GDB inspection of the core recovered this panic message:

```text
failed printing to stderr: Broken pipe (os error 32)
```

The same thread's memory contains the pending log line:

```text
[1795723522] IPC command: get_status
```

The recorded stack passes through libc `raise` and `abort` into the daemon. The panic's location metadata identifies Rust's `library/std/src/io/stdio.rs`, line 1209, column 9. The daemon executable is stripped and has no build ID; full function names could not be recovered. Arch debuginfod downloads were attempted but unavailable from the diagnostic environment. These limitations do not obscure the recovered panic message.

The installed source and project checkout have the relevant logging pattern in [`backend/src/log.rs`](../backend/src/log.rs):

```rust
eprintln!("{line}");
if let Some(log) = LOG.get() {
    // Append to the private daemon log.
}
```

`eprintln!` panics when writing to stderr fails. Because this call precedes the file append, that log entry never reaches the private log. The logger also uses `eprintln!` for file-write and lock errors, exposing those paths to the same failure.

The release profile in [`backend/Cargo.toml`](../backend/Cargo.toml) sets `panic = "abort"`. A logging panic therefore aborts the whole daemon, including when it originates in an IPC worker thread.

Other core threads were waiting in sleep or D-Bus polling. No evidence implicated audio decoding or an active track operation in this crash.

## Timeline and likely trigger

All times below are CET on 26 November 2026.

| Time | Observation |
| --- | --- |
| 21:02:04 | Omarchy shell exited with status 137 and began relaunching. |
| 21:02:15–16 | Another `tidal-daemon`, PID `2150983`, aborted; its recorded stack has the same daemon offsets as the later crash. |
| 21:05:13 | Omarchy shell again exited with status 137 and began relaunching. |
| 21:05:15 | Replacement shell launched. |
| 21:05:22 | PID `183755` aborted while logging `get_status`. |
| 21:05:23 | systemd-coredump completed recording the crash. |
| 21:05:27 | Shell reported Tidal's MPRIS peer disconnecting and its IPC socket closing. |

The most likely sequence is that the old daemon survived the shell restart, lost the reader of its stderr pipe, and then received a request from the replacement shell. Logging that request encountered `EPIPE` and panicked. The core proves the broken stderr pipe and logging panic; the journal supports, but does not independently prove, the pipe ownership and closure sequence.

The reason for the shell's status-137 exits remains unresolved. Status 137 alone does not establish an OOM kill. No OOM or kernel "Killed process" entries were found around this incident. At diagnosis time, the system had approximately 1.4 GiB available memory and 8.1 GiB free swap; those readings are not measurements from the instant of the crash.

## Recurrence and impact

`coredumpctl list tidal-daemon` showed ten aborts between 23 October and 26 November, including six with retained cores from 17 November onward. Both crashes on 26 November have matching recorded stacks; the older crashes were not individually diagnosed and should not all be assumed to share this cause.

The executable's modification time was 20 November at 07:07:38 CET. No package transaction near the crash was found in the inspected pacman log. The evidence points to a runtime logging failure rather than a package replacement at the crash timestamp.

The crash interrupts the daemon's IPC and MPRIS availability. No saved-data loss was established: the session file at `~/.local/state/tidal/session.json` and UI settings at `~/.local/state/omarchy/tidal/ui_settings.json` remained present. Their presence is not a full integrity audit. No recovery or trash restoration was needed for any identified missing file.

## Recommended correction and verification

Make daemon logging tolerate stderr failures. Replace fallible-print macros in daemon logging paths with explicit writes whose errors are handled without another fallible-print macro. Keep private file logging independent of stderr success. Audit other daemon-mode `println!` and `eprintln!` calls for the same risk.

A focused regression check should close the read end of a subprocess's stderr pipe, trigger logging, and verify that the daemon remains alive and appends to its private log. A separate integration check should verify shell restart and daemon reconnection behavior. Run the repository's required verification gate before claiming an implementation milestone complete.

The failure is likely to recur whenever the daemon logs after losing its stderr reader. The robust correction belongs in the Tidal plugin's daemon. The evidence does not establish an Omarchy core defect; any report should first go to the `omarchy-tidal-music-player` project. The shell's unexplained exits may deserve a separate investigation. No upstream issue was filed.

## Diagnostic scope and cleanup

The investigation used `coredumpctl info`, crash history, journal entries, package logs, filesystem metadata, source inspection, and all-thread GDB backtraces. It did not modify application code or configuration, restart services, or mute notifications.

Extracted cores and temporary debuginfod caches were created under fresh temporary directories and removed after inspection. The original systemd-managed core was retained. No core memory, credentials, or session contents are included in this report.
