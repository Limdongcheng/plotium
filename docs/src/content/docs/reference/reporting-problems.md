---
title: Reporting problems
description: Find the diagnostic files to attach when reporting a Plotium crash or runtime problem.
---

## After a crash

When Plotium catches an internal error, it saves a plain-text crash report and
shows its full path. On the next launch, the recovery dialog or Operation history
shows that path again. Attach the report to a
[GitHub issue](https://github.com/nmrtist/plotium/issues); it contains the Plotium
version, platform, panic location, backtrace, and the tail of the session log.

Crash reports are stored in the `crashes` subdirectory of Plotium's application
data directory. Plotium keeps the 10 most recent reports:

- Windows: `%LOCALAPPDATA%\plotium\data\crashes`
- macOS: `~/Library/Application Support/plotium/crashes`
- Linux: `$XDG_DATA_HOME/plotium/crashes`, or `~/.local/share/plotium/crashes`

## Logs for other problems

Each launch creates a log in the adjacent `logs` subdirectory. Plotium keeps the
five most recent session logs. For a problem that does not crash the
application, attach the log from the affected session and describe what you
were doing. Include a sample project or input file only when it does not
contain sensitive data.

Plotium does not upload reports or logs automatically.
