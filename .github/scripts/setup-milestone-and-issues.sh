#!/bin/bash
# Script to create GitHub milestone and issues for the transport refactor
# Run with: gh auth login && bash .github/scripts/setup-milestone-and-issues.sh

echo "Setting up milestone and issues for transport refactor..."

# Create milestone
echo "Creating milestone..."
MILESTONE_NUMBER=$(gh api repos/:owner/:repo/milestones \
  --method POST \
  --field title="v0.x.0 Transport Refactor" \
  --field description="Complete overhaul of transport layer to create a lean, idiomatic, and composable API" \
  --field state="open" \
  --jq '.number')

echo "Milestone created with number: $MILESTONE_NUMBER"

# Create the main tracking issue
echo "Creating main tracking issue..."
TRACKING_ISSUE=$(gh issue create \
  --title "[EPIC] Transport Layer Refactor - Lean, Idiomatic, Composable API" \
  --body-file .github/ISSUE_TEMPLATE/transport-refactor-tracking.md \
  --label "epic,refactor,breaking-change,architecture" \
  --milestone "$MILESTONE_NUMBER" \
  --assignee "@me")

TRACKING_NUMBER=$(echo "$TRACKING_ISSUE" | grep -oE '[0-9]+$')
echo "Main tracking issue created: #$TRACKING_NUMBER"

# Create individual task issues
echo "Creating individual task issues..."

# Task A
gh issue create \
  --title "[Task A] Define new Transport traits" \
  --body "## Task Description
Define the new minimal Transport traits in \`src/transport/mod.rs\`.

## Acceptance Criteria
- [ ] Create minimal \`Transport\` trait for sync
- [ ] Create \`AsyncTransport\` trait behind feature flag
- [ ] Remove old \`Transport\`, \`BlockingTransport\`, \`UnifiedTransport\` traits
- [ ] Add blanket impls for \`Read + Write\` / \`AsyncRead + AsyncWrite\`

## Implementation Notes
- Focus on raw byte exchange only
- No retry or reconnection logic
- Keep the traits minimal and composable

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-a" \
  --milestone "$MILESTONE_NUMBER"

# Task B
gh issue create \
  --title "[Task B] Delete resiliency code" \
  --body "## Task Description
Remove all embedded resiliency and reconnection logic from the crate.

## Acceptance Criteria
- [ ] Remove \`src/transport/resilient.rs\`
- [ ] Remove \`src/reconnecting_transport.rs\`
- [ ] Clean up references in \`lib.rs\`
- [ ] Remove from \`Cargo.toml\`
- [ ] Update tests and docs

## Implementation Notes
- This will remove ~1,500 lines of code
- Users will need to implement their own retry logic
- Consider adding an example retry wrapper

## Dependencies
- Depends on: Task A

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-b" \
  --milestone "$MILESTONE_NUMBER"

# Task C
gh issue create \
  --title "[Task C] Refactor Session layer" \
  --body "## Task Description
Refactor \`Session\` (\`src/session.rs\`) to work with new transport traits and handle protocol concerns.

## Acceptance Criteria
- [ ] Accept \`&mut dyn Transport\` / \`&mut dyn AsyncTransport\` parameters
- [ ] Manage VISCA socket IDs (0-7) internally
- [ ] Handle packet assembly/disassembly
- [ ] Update error types as needed

## Implementation Notes
- Socket IDs should no longer be transport concerns
- Session is responsible for VISCA protocol details
- Keep transport interface minimal

## Dependencies
- Depends on: Task A, Task B

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-c" \
  --milestone "$MILESTONE_NUMBER"

# Task D
gh issue create \
  --title "[Task D] Refactor Camera client" \
  --body "## Task Description
Update high-level client (\`Camera\`, etc.) to work with generic transport traits.

## Acceptance Criteria
- [ ] Make Camera generic over \`T: Transport\`
- [ ] Add async variant under feature flag
- [ ] Update all dependent types
- [ ] Ensure examples still work

## Implementation Notes
- This affects the main public API
- Need to maintain both sync and async variants
- Consider ergonomics for common use cases

## Dependencies
- Depends on: Task C

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-d" \
  --milestone "$MILESTONE_NUMBER"

# Task E
gh issue create \
  --title "[Task E] Extract network implementations to examples" \
  --body "## Task Description
Move concrete network implementations out of the library into examples.

## Acceptance Criteria
- [ ] Move TCP impl to \`examples/tcp_transport.rs\`
- [ ] Move UDP impl to \`examples/udp_transport.rs\`
- [ ] Create serial port example
- [ ] Ensure examples compile with \`--examples\`
- [ ] Remove from library exports

## Implementation Notes
- These become reference implementations
- Users can copy and customize as needed
- Keep examples well-documented

## Dependencies
- Depends on: Task D

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-e" \
  --milestone "$MILESTONE_NUMBER"

# Task F
gh issue create \
  --title "[Task F] Update documentation" \
  --body "## Task Description
Update all documentation to reflect the new architecture.

## Acceptance Criteria
- [ ] Update README with new architecture
- [ ] Document transport trait and rationale
- [ ] Add retry wrapper example
- [ ] Remove stale resiliency references
- [ ] Update all rustdoc comments

## Implementation Notes
- Explain the design philosophy
- Provide migration guide from old API
- Show common patterns (retry, logging, etc.)

## Dependencies
- Depends on: Task E

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-f,documentation" \
  --milestone "$MILESTONE_NUMBER"

# Task G
gh issue create \
  --title "[Task G] Update tests for new architecture" \
  --body "## Task Description
Adapt all tests to work with the new transport architecture.

## Acceptance Criteria
- [ ] Update unit tests for new API
- [ ] Add mock transport implementing \`Read\`/\`Write\`
- [ ] Test all feature combinations
- [ ] Ensure CI passes all checks
- [ ] Add transport trait tests

## Implementation Notes
- Need good mocking strategy for transports
- Test both sync and async paths
- Ensure examples are tested

## Dependencies
- Depends on: Task F

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-g" \
  --milestone "$MILESTONE_NUMBER"

# Task H
gh issue create \
  --title "[Task H] Audit and update Cargo features" \
  --body "## Task Description
Review and update Cargo features for the new architecture.

## Acceptance Criteria
- [ ] Keep \`async\`, \`tokio\`, \`serialport\` as opt-in
- [ ] Remove resilient/reconnect features
- [ ] Update feature documentation
- [ ] Verify feature combinations work

## Implementation Notes
- Simplify feature matrix
- Document what each feature enables
- Consider feature dependencies

## Dependencies
- Depends on: Task G

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-h" \
  --milestone "$MILESTONE_NUMBER"

# Task I
gh issue create \
  --title "[Task I] Release preparation" \
  --body "## Task Description
Prepare for the breaking release of the new architecture.

## Acceptance Criteria
- [ ] Update CHANGELOG.md with all changes
- [ ] Bump version to 0.x.0 (breaking)
- [ ] Create migration guide
- [ ] Tag release
- [ ] Update crates.io metadata

## Implementation Notes
- This is a major breaking change
- Provide clear migration path
- Consider blog post or announcement

## Dependencies
- Depends on: All other tasks

## Related Epic
Part of #$TRACKING_NUMBER" \
  --label "refactor,transport-epic,task-i" \
  --milestone "$MILESTONE_NUMBER"

echo "All issues created successfully!"
echo "View the milestone at: https://github.com/:owner/:repo/milestone/$MILESTONE_NUMBER"
echo "View the tracking issue at: $TRACKING_ISSUE"