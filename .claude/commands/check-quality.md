---
allowed-tools: Bash(gh issue view:*), Bash(gh issue comment:*), Bash(gh issue close:*), Edit, ReadFile, Bash(git add:*), Bash(git commit:*), Bash(git push:*), Bash(git status:*), Bash(git diff:*), Bash(cargo test*), Bash(cargo build*), Bash(cargo check*), Bash(cargo clippy*), Bash(cargo fmt*), Bash(grep:*), Bash(find:*), Bash(rg:*)
description: Validate that all requirements for a GitHub issue have been fully implemented
---
Run all code quality checks and tests to ensure that the code meets the project's standards.

## Quality Checks
1. Review the github workflow @.github/workflows/ci.yml to understand the quality checks that are run.
2. Execute all of the checks defined in the workflow, for all features tested by the workflow.
3. Fix everything, thats why we are asking you to do this. Ensure that all checks pass without either errors or warnings.
4. Resolve issues with the highest code quality approach rather than the quickest solution.  We are not interested in expedient solutions, but rather in the best practices and standards of the project.