# Diagnostics

Diagnostics are structured values with a project-owned `VESTRA-*` code.

| Field | Meaning |
| --- | --- |
| `severity` | `fatal` or `warning`. |
| `category` | `usage`, `project`, `semantic`, `asset`, `media`, `backend`, `render`, `output`, `cancellation`, or `internal`. |
| `code` | Stable-looking machine identifier in the `VESTRA-` namespace. Consumers should match documented codes, not message text. |
| `message` | Human-readable explanation. |
| `pointer` | Optional JSON pointer to the related field. |
| `related_id` | Optional project object identifier. |
| `hint` | Optional remediation context. |

Rust exposes `Diagnostic` through validation reports and SDK errors. Python
exposes immutable `Diagnostic` values through reports and exception
attributes. CLI human output formats them for people. CLI JSON results and
reports serialize diagnostics, errors, and warnings where the command has
them.

Tracing/logging is operational observability. `RenderEvent` progress describes
operation state. Diagnostics and errors are structured, actionable failures;
JSON progress is not a diagnostics stream and does not carry warnings.

Codes are part of the current diagnostic contract, but the project does not
promise that every code is an eternal catalog. Treat codes as identifiers and
messages as display text. The current namespace is `VESTRA-`; older project
namespaces are not current behavior.
