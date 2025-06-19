# Transport Refactor Workflow Guide

This document outlines the GitHub-based workflow for managing the transport layer refactor.

## Initial Setup

1. **Run the label setup script**:
   ```bash
   gh auth login
   bash .github/scripts/setup-labels.sh
   ```

2. **Create milestone and issues**:
   ```bash
   bash .github/scripts/setup-milestone-and-issues.sh
   ```

## Development Workflow

### 1. Starting a Task

1. Find your task issue (e.g., "Task A: Define new Transport traits")
2. Assign yourself to the issue
3. Create a feature branch:
   ```bash
   git checkout -b refactor/task-a-new-traits
   ```

### 2. Branch Naming Convention

Use descriptive branch names following this pattern:
- `refactor/task-[letter]-[description]`
- Examples:
  - `refactor/task-a-new-traits`
  - `refactor/task-b-remove-resiliency`
  - `refactor/task-c-session-update`

### 3. Commit Guidelines

Follow conventional commits:
- `feat:` - New features
- `refactor:` - Code refactoring
- `fix:` - Bug fixes
- `docs:` - Documentation only
- `test:` - Test additions/changes
- `chore:` - Build process or auxiliary tool changes

Examples:
```bash
git commit -m "refactor: remove old Transport trait hierarchy"
git commit -m "feat: add minimal Transport trait with exchange method"
git commit -m "docs: update README with new transport architecture"
```

### 4. Pull Request Process

1. Push your feature branch:
   ```bash
   git push -u origin refactor/task-a-new-traits
   ```

2. Create PR using the template:
   ```bash
   gh pr create --fill
   ```

3. Link to the task issue in the PR description
4. Ensure all CI checks pass
5. Request review if needed

### 5. Task Completion

1. After PR is merged, the task issue will auto-close (if you used "Closes #X")
2. Delete your feature branch:
   ```bash
   git branch -d refactor/task-a-new-traits
   git push origin --delete refactor/task-a-new-traits
   ```

## Progress Tracking

### Using the Milestone
- View overall progress: https://github.com/GrantSparks/grafton-visca/milestones
- See all refactor issues: Click on the "v0.x.0 Transport Refactor" milestone

### Using Labels
- View all refactor tasks: Filter by `transport-epic` label
- View specific task: Filter by `task-a`, `task-b`, etc.
- View by priority: Filter by `priority:high`, `priority:medium`, etc.

### Project Board (Optional)
You can create a project board for visual tracking:
1. Go to Projects tab
2. Create new project "Transport Refactor"
3. Add automation for moving cards based on issue/PR status

## Testing Strategy

Before marking a task complete:
1. Run full test suite:
   ```bash
   cargo test --all-features
   ```

2. Check specific feature combinations:
   ```bash
   cargo test --no-default-features
   cargo test --features async
   cargo test --features tokio
   ```

3. Run lints:
   ```bash
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

4. Build documentation:
   ```bash
   cargo doc --no-deps --all-features
   ```

## Communication

### Issue Comments
- Use issue comments for design discussions
- Tag people with @username for input
- Post status updates on blockers

### PR Reviews
- Be specific in review comments
- Suggest code changes using GitHub's suggestion feature
- Approve with confidence once criteria are met

## Rollback Plan

If issues arise after merging:
1. Create hotfix branch from main
2. Revert problematic changes
3. Create PR with clear explanation
4. Consider creating a "lessons learned" issue

## Release Process

Once all tasks are complete:
1. Create release branch: `release/v0.x.0`
2. Update version in Cargo.toml
3. Update CHANGELOG.md
4. Create PR to main
5. After merge, create GitHub release
6. Publish to crates.io

## Tips for Success

1. **Work incrementally** - Small, focused PRs are easier to review
2. **Communicate blockers** - Don't hesitate to ask for help
3. **Test thoroughly** - This is a breaking change, quality matters
4. **Document as you go** - Update docs in the same PR as code changes
5. **Stay aligned** - Refer back to the design goals regularly

## Quick Reference

- **Main tracking issue**: Look for "[EPIC] Transport Layer Refactor"
- **Milestone**: "v0.x.0 Transport Refactor"
- **Key labels**: `transport-epic`, `task-[a-i]`, `refactor`
- **CI requirements**: All tests pass, clippy clean, formatted

---

Remember: This is a significant architectural change. Take time to get it right, and don't hesitate to discuss design decisions in the issues!