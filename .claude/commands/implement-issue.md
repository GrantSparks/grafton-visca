---
allowed-tools: Bash(gh issue view:*), Bash(gh issue comment:*), Edit, ReadFile, Bash(git add:*), Bash(git commit:*), Bash(git push:*), Bash(cargo test), Bash(cargo build), Bash(cargo check), Bash(cargo clippy), Bash(cargo fmt), Bash(grep:*), Bash(find:*), Bash(rg:*)
description: Implement the next logical step for a GitHub issue
---
Read GitHub issue #$ARGUMENTS and implement the next logical step based on the issue description and all comments.

## Initial Analysis Phase
Use `gh issue view $ARGUMENTS` to read the issue then ` --comments` for all comments. Use all available tools to:
- Search for relevant code patterns mentioned in the issue
- Read key files to understand code structure
- Understand how components interact
- Read all the comments, particularly all the way to the end, to capture the latest discussions and decisions

Analyze:
- What has already been implemented
- Decisions made in the discussion
- Remaining tasks and any blockers
- Priority and dependencies

## Implementation Guidelines
- **Code Understanding**: Use `ReadFile` to examine relevant files and understand code relationships
- **Precise Editing**: Use `Edit` for accurate code modifications
- **Code Quality**: Prioritize correctness, completeness, and maintainability
- **Breaking Changes**: This is a breaking change for a fresh API, so backward compatibility is not a concern
- **Testing**: Use `Bash` commands to run tests and verify changes

## Implementation Process
1. **Discovery**:
   - Use `ReadFile` to examine relevant functions, structs, traits, and modules
   - Use `Bash` commands like `grep`, `find`, or `rg` to locate code patterns
   - Trace dependencies by reading related files and checking Cargo.toml
   - Additional Tools
      - rust-analyzer (IDE code intelligence)
      - cargo-expand (macro expansion inspection)
      - rustup components: rust-analyzer, llvm-tools, docs, src, std

2. **Implement Thoroughly**:
   - Use `Edit` for modifying existing files
   - Use `Create` for new files or `Edit` when needed
   - Run tests with `Bash(cargo test:*)` to verify changes
   - Use `Bash(cargo check)` for quick compilation checks during development
   - Run `Bash(cargo clippy)` to catch common mistakes and improve code quality

3. **Verify Implementation**:
   - Execute tests and check for failures
   - Search for any TODOs or FIXMEs in modified code
   - Ensure all edge cases are handled
   - Run `Bash(cargo fmt)` to ensure consistent formatting

## GitHub Comment Update
After implementation, create a detailed comment:

1. Draft comment using `Edit` to create `/tmp/gh-comment-$ARGUMENTS.md` including:
   - **Progress Update**: What was implemented with technical details
   - **Key Changes**: Files modified/created and important code changes
   - **Testing**: Test results and any new test cases added
   - **Next Steps**: Remaining tasks as checkboxes
   - **Notes**: Important considerations, potential issues, dependencies

2. Use 'gh issue comment $ARGUMENTS --body-file /tmp/gh-comment-$ARGUMENTS.md' to post the comment

Remember: Take time to understand the codebase structure before implementing. Use available tools effectively to navigate and modify code precisely.
