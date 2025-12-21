---
allowed-tools: Bash(gh issue view:*), Bash(gh issue comment:*), Bash(gh issue close:*), Edit, ReadFile, Bash(git add:*), Bash(git commit:*), Bash(git push:*), Bash(git status:*), Bash(git diff:*), Bash(cargo test*), Bash(cargo build*), Bash(cargo check*), Bash(cargo clippy*), Bash(cargo fmt*), Bash(grep:*), Bash(find:*), Bash(rg:*)
description: Validate that all requirements for a GitHub issue have been fully implemented
---
Read GitHub issue #$ARGUMENTS and validate that every requirement, including extended scope mentioned in comments, has been fully and completely implemented.

## Validation Process Overview
Use the github mcp service to read the issue and all comments. Do not trust claims made in the issue or comments - independently verify everything. Assess whether the implementation meets the highest quality standards and identify potential improvements.

## Phase 1: Requirements Extraction
1. **Read Complete Context**:
   - Use the `Bash(git remote -v)` command to fetch the repo details.
   - Use the github mcp service to get full issue history
   - Extract all requirements from the original issue
   - Identify any scope expansions or clarifications in comments
   - Note any edge cases or special considerations mentioned
   - Read all the comments, particularly all the way to the end, to capture the latest discussions and decisions

2. **Define Full Scope**:
   - Determine what should properly be in scope given the initial task
   - Don't accept limited scope claims at face value
   - Consider what a thorough implementation would include
   - Identify any missing functionality that should reasonably be included

## Phase 2: Implementation Verification
1. **Code Inspection**:
   - Use `ReadFile` to examine all relevant files mentioned in comments
   - Use `Bash(grep:*)`, `Bash(find:*)`, or `Bash(rg:*)` to search for:
     - Implementation of each requirement
     - Error handling for edge cases
     - Proper input validation
     - Complete test coverage
   - Verify code quality, naming conventions, and documentation
   - Look for any remaining TODO comments relating to the issue
   - Additional Tools
      - rust-analyzer (IDE code intelligence)
      - cargo-expand (macro expansion inspection)
      - rustup components: rust-analyzer, llvm-tools, docs, src, std

2. **Functionality Testing**:
   - Run all relevant tests with `Bash(cargo test:*)`
   - Check for test coverage of all requirements
   - Look for missing test cases for edge conditions
   - Verify integration between components
   - Use `Bash(cargo check)` for quick compilation verification

3. **Quality Assessment**:
   - Run `Bash(cargo clippy)` to check for common mistakes and improvements
   - Check `Bash(cargo fmt -- --check)` for proper formatting
   - Look for opportunities to use more idiomatic Rust patterns
   - Verify proper error handling with Result/Option types
   - Check for proper lifetime annotations where needed
   - Assess unsafe code usage if any

## Phase 3: Improvement Analysis
1. **Identify Gaps**:
   - List any unimplemented requirements
   - Note missing edge case handling
   - Identify incomplete or weak test coverage
   - Find opportunities for better code organization

2. **Quality Improvements**:
   - How could the implementation be more idiomatic Rust?
   - Are there better patterns for error handling?
   - Could the code be more performant or memory efficient?
   - Are there opportunities to reduce allocations?
   - Could traits be better utilized for abstraction?

## Phase 4: Decision and Action
Based on your validation:

### If Implementation is Incomplete or Substandard:
1. Draft detailed feedback in `/tmp/gh-validation-$ARGUMENTS.md` including:
   - **Validation Results**: What was checked and findings
   - **Missing Requirements**: Checklist of unimplemented items
   - **Quality Issues**: Specific problems found (clippy warnings, etc.)
   - **Recommended Improvements**: Concrete suggestions with examples
   - **Next Steps**: Clear action items for completion

2. Post it as a comment to github.

### If Implementation is Complete and High Quality:
1. Final verification:
   - Run `git status` to see uncommitted changes
   - Use `git diff` to review any pending modifications
   - Run `cargo test` to ensure all tests pass.  Test with each feature enabled
   - Run `cargo build --release` to verify release build works.  Also compile examples

2. Commit and push.

3. Either Post a comment and close the issue if you are already on the main branch, or in the case where you are on a feature branch create a PR instead of closing the issue directly.

## Validation Standards
- **Completeness**: Every requirement explicitly stated or reasonably implied is implemented
- **Correctness**: Implementation works correctly for all valid inputs
- **Robustness**: Proper handling of edge cases and invalid inputs
- **Quality**: Code is clean, maintainable, and follows Rust best practices
- **Testing**: Comprehensive test coverage exists for all functionality
- **Documentation**: Code has proper doc comments and examples where appropriate

Remember: Be thorough and critical. The goal is to ensure the highest quality implementation, not just to check boxes. If the implementation could be better, say so with specific recommendations.
