---
allowed-tools: Bash
description: Intelligently tidy Rust source files by reorganizing imports and removing obsolete comments
argument-hint: <file-path>
---

# Intelligently Tidy Rust Source File

Analyze and reorganize the Rust source file at: $ARGUMENTS

## Your Task

Edit the specified Rust file and tidy the code so that it:
1. Reorganizes and merges imports following Rust conventions
2. Removes obsolete comments while preserving valuable documentation
3. Maintains all functional code exactly as-is

## Import Organization Rules

1. **Group imports into three sections** with blank lines between:
   - External crates (alphabetically sorted)
   - Standard library imports (alphabetically sorted)  
   - Local module imports (alphabetically sorted)

2. **Merge imports from the same crate** into single statements:
   - `use std::io::Read;` and `use std::io::Write;` → `use std::io::{Read, Write};`
   - `use serde::Deserialize;` and `use serde::Serialize;` → `use serde::{Deserialize, Serialize};`
   - Sort items within braces alphabetically
   - Use nested grouping: `use std::{collections::HashMap, io::{Read, Write}};`

## Intelligent Comment Cleanup

### REMOVE these comments:

1. **Completed TODOs/FIXMEs**: 
   - Check if the described work is already implemented in the code
   - Example: "TODO: Add error handling" when error handling exists

2. **Debugging artifacts**:
   - Commented-out `println!()`, `dbg!()`, `eprintln!()`
   - Comments like `// debug`, `// test`, `// temp print`

3. **Redundant explanations**:
   - `// increment counter` above `counter += 1;`
   - `// return result` above `return result;`
   - `// constructor` above `fn new()`

4. **Development remnants without context**:
   - Bare `// HACK`, `// TEMP`, `// FIXME` without explanation
   - `// OLD`, `// DEPRECATED` on commented code
   - Version migration notes that no longer apply

5. **Commented-out code** without preservation reason

## Example Transformation

If the file contains:
```rust
use std::io::Write;
// OLD: for legacy support
// use old_crate::parse;
use std::io::Read;

fn process(data: &str) {
    // TODO: Add validation
    if data.is_empty() {
        return;
    }
    // Process the data
    println!("{}", data); // output result
}
```

Output:
```rust
use std::io::{Read, Write};

fn process(data: &str) {
    if data.is_empty() {
        return;
    }
    println!("{}", data);
}
```
