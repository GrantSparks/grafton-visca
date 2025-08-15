---
allowed-tools: Bash
description: Intelligently tidy Rust source files by reorganizing imports and removing obsolete comments
argument-hint: <file-path>
---

# Intelligently Tidy Rust Source File(s)

Analyze and reorganize the Rust source file(s) at: **\$ARGUMENTS**

## Goal

Rewrite only import statements and comments so that the codebase is tidier **without changing behavior**.

* **Do**: regroup/merge/sort `use` items; delete clearly obsolete or noisy comments.
* **Do not**: change executable code, public API, visibility, attributes, semantics, or edition settings.

> **Idempotency:** Running this task repeatedly must produce no further changes.

---

## Invariants & Safety

* Preserve **all** doc comments (`///`, `//!`) and explanatory comments tied to correctness or safety, e.g. lines starting with **`SAFETY:`**, **`INVARIANT:`**, **`PANIC:`**, **`ERRORS:`**, **`PERF:`**, **`SECURITY:`**, **`NOTE:`**, **`WARNING:`**.
* Preserve license headers and file/module attributes (`#![...]`, `#[...]`), including `#[cfg(...)]`, `#[allow(...)]`, `#[deny(...)]`, `#[rustfmt::skip]`, `#[macro_use]`, `#[path = "..."]`, and `#[doc = ...]`.
* **Never** merge, move, or modify imports across differing attributes, visibility, or configuration (e.g., different `#[cfg(...)]` conditions).
* Do not alter globs (`*`) or change aliasing (`as`) names.
* Do not move or rewrite `pub use` re-exports; keep their order and grouping intact.
* Only process **top-level** `use` items. Leave function-/block-local imports unchanged.

---

## Import Organization Rules

Group and order imports into **three sections** separated by **one blank line** (keep any surrounding attributes attached to the corresponding section or item). Within each section, sort items **alphabetically, case-insensitive**, by full path, then sort nested brace contents alphabetically.

> **Section order (as specified here):**
>
> 1. **External crates**
> 2. **Standard library**
> 3. **Local modules**

> **Note:** This order differs from `rustfmt`’s default (Std → External → Crate). Honor the order above even if it diverges from project `rustfmt.toml`.

### Classification

* **Standard library:** paths starting with `std::`, `core::`, `alloc::`, or `proc_macro::`.
* **Local modules:** paths starting with `crate::`, `self::`, or `super::`, **and** absolute paths that match the current crate name (if determinable from `Cargo.toml`).
* **External crates:** everything else (including dependency crate names from `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, and workspace dependencies).

### Merging Rules

* Merge imports that share the exact same leading path and attribute/visibility set.

  * Example:
    `use std::io::Read;` + `use std::io::Write;` → `use std::io::{Read, Write};`
* Use nested grouping where helpful:
  `use std::{collections::HashMap, io::{Read, Write}};`
* Keep `pub use` lines separate; **do not** merge them with private `use` lines.
* **Do not** merge across:

  * Different `#[cfg(...)]`/attributes or different visibility (`pub` vs private).
  * Different aliasing (`as`) targets that would collide.
  * Macro imports that rely on `#[macro_use]` or edition-specific import behavior.
  * Globs (`*`)—never expand or fold them.

### Formatting

* Exactly **one** blank line **between import sections** and **between the last import section and the next non-import item**.
* Remove duplicate imports; keep a single canonical entry after merging.
* Preserve inline/trailing comments **on import lines** by placing them on the merged line or directly above it.

---

## Intelligent Comment Cleanup

Delete comments that add no enduring value and are clearly obsolete, while keeping documentation and rationale.

### Remove

1. **Completed TODOs/FIXMEs**

   * If the exact requested action is evidently implemented within the nearby code (e.g., “TODO: Add error handling” and the code now returns `Result` or uses `?`/`map_err`).
2. **Debugging artifacts**

   * Commented-out `println!`, `dbg!`, `eprintln!`, `log::...`, `tracing::...`, and markers like `// debug`, `// test`, `// temp print`.
3. **Redundant narration**

   * Obvious explanations like `// increment counter` above `counter += 1;`, `// return result` above `return ...;`, `// constructor` above `fn new()`.
4. **Unhelpful development remnants**

   * Bare `// HACK`, `// TEMP`, `// FIXME` without actionable context; stale migration notes; `// OLD`/`// DEPRECATED` on commented code.
5. **Commented-out code** without a preservation reason.

   * Heuristics: lines beginning with `//` or within `/* ... */` that look like Rust syntax (`fn `, `struct `, `enum `, `impl `, `let `, `use `, `mod `, `match `, `if `, `pub `, `async `, `await`, etc.).

### Keep

* All doc comments (`///`, `//!`) and any comments that explain invariants, safety, concurrency, lifetime guarantees, algorithmic complexity, security considerations, or public API rationale.
* Tooling directives embedded in comments (e.g., `cbindgen:`, `tarpaulin:`, `coverage:`, `rust-analyzer:`) and “generated code” guards (`// @generated`, `// DO NOT EDIT`).
* Comments tied to attributes or configuration, or that are the only record of non-obvious behavior.

> **Conservatism rule:** If unsure whether a comment is valuable, **keep it**.  Explanations of large code blocks, complex logic, or unusual patterns are often helpful for future maintainers.

---

## Procedure (Agent Steps)

1. **Scope detection**

   * If a `Cargo.toml` is present, read:

     * `package.name` (to recognize local absolute paths).
     * All dependency sections (to recognize external crates).
2. **Collect import blocks**

   * Consider only top-level `use` items not inside macros or blocks.
   * Keep `pub use` re-exports in place; do not reorder relative to other code beyond intra-section sorting.
3. **Classify, merge, and sort** according to the rules above.
4. **Rewrite imports**

   * Emit three sections in the specified order, with one blank line between sections.
   * Preserve attached attributes and meaningful comments.
5. **Clean comments**

   * Apply removal heuristics above, being conservative around non-obvious comments.
6. **Whitespace tidy**

   * Collapse multiple blank lines (max one) between logical sections; do not touch intentional spacing in doc comments.
7. **Idempotency check**

   * Ensure a second run would produce no changes.
8. **(If project root available)** Quick safety check

   * If `Cargo.toml` exists: `cargo check -q` (do not attempt to “fix” compile errors; only report if your changes would have introduced them).
9. **Report**

   * For each file, summarize: number of imports merged/deduplicated, comments removed, and any preserved “safety/important” comments encountered.

---

## Examples

**Before**

```rust
// OLD: for legacy support
// use old_crate::parse;
use std::io::Write;
use std::io::Read;
// TEMP
use serde::Serialize; // debug
use serde::Deserialize;

fn process(data: &str) {
    // TODO: Add validation
    if data.is_empty() {
        return; // return result
    }
    // Process the data
    // println!("processing: {}", data);
    println!("{}", data); // output result
}
```

**After**

```rust
use serde::{Deserialize, Serialize};

use std::io::{Read, Write};

fn process(data: &str) {
    if data.is_empty() {
        return;
    }
    println!("{}", data);
}
```

**Attributes & cfg‑gated imports (preserve, don’t cross-merge)**

```rust
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::util::{self, Result};
```

**Aliases & nested grouping**

```rust
use std::{collections::HashMap, io::{self, Read, Write}};
use anyhow::{Context as AnyhowContext, Result};
```

**Re-exports (do not move/merge)**

```rust
pub use crate::api::Client;        // keep position
pub use crate::config::Config;     // keep position

use std::fmt::{self, Display};
```

---

## Inline Formatting

When formatting strings, prefer named/inline formatting to positional formatting for clarity and maintainability.

For our codebase, please use named/inline formatting in strings:

✅ Do: format!("Config loaded: {config_name}")
❌ Avoid: format!("Config loaded: {}", config_name)

This applies to *all* formatting macros (format!, println!, eprintln!, log macros, etc.)

---

## Non‑Goals

* No refactors, no renaming, no code movement beyond import blocks (other than formatting strings).
* Don’t expand or collapse glob imports.
* Don’t change visibility (`pub`), edition, features, or attributes.
* Don’t modify strings, literals, or macro invocations.

---

## Minimal Bash Hints (optional, when available)

* Detect workspace context:

  * `test -f Cargo.toml && cargo metadata --no-deps -q || true`
* Safety check:

  * `cargo check -q || true` (report only)
* Consider using `rustfmt` **only** to normalize formatting **within sections** (not group order). The specified section order takes precedence.

---

## Acceptance Criteria (per file)

* Imports exist in exactly three sections (External → Standard → Local) with single blank lines between sections.
* Items within each section are alphabetically sorted; nested brace contents are alphabetically sorted.
* Duplicate and mergeable imports are merged without crossing attributes/visibility/cfg.
* `pub use` lines are preserved and not merged with private uses.
* Obvious debug/temporary/commented-out code and completed TODO/FIXME comments are removed.
* All doc/safety/invariant/security/tooling comments remain intact.
* A second run yields zero diffs.
