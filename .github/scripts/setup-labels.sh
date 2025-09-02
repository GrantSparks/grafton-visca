#!/bin/bash
# Script to create GitHub labels for the transport refactor project
# Run with: gh auth login && bash .github/scripts/setup-labels.sh

echo "Setting up GitHub labels for grafton-visca..."

# Epic and task labels
gh label create "epic" --description "Large feature or refactor spanning multiple issues" --color "7057ff"
gh label create "transport-epic" --description "Related to transport layer refactor" --color "5319e7"
gh label create "task-a" --description "Define new traits" --color "c5def5"
gh label create "task-b" --description "Delete resiliency code" --color "c5def5"
gh label create "task-c" --description "Refactor Session" --color "c5def5"
gh label create "task-d" --description "Refactor Camera client" --color "c5def5"
gh label create "task-e" --description "Extract network implementations" --color "c5def5"
gh label create "task-f" --description "Update documentation" --color "c5def5"
gh label create "task-g" --description "Testing" --color "c5def5"
gh label create "task-h" --description "Feature audit" --color "c5def5"
gh label create "task-i" --description "Release preparation" --color "c5def5"

# Type labels
gh label create "refactor" --description "Code refactoring" --color "e99695"
gh label create "breaking-change" --description "Introduces breaking API changes" --color "d73a4a"
gh label create "architecture" --description "Architectural changes" --color "0052cc"

# Standard labels
gh label create "bug" --description "Something isn't working" --color "d73a4a"
gh label create "enhancement" --description "New feature or request" --color "a2eeef"
gh label create "documentation" --description "Improvements or additions to documentation" --color "0075ca"
gh label create "good first issue" --description "Good for newcomers" --color "7057ff"
gh label create "help wanted" --description "Extra attention is needed" --color "008672"

# Priority labels
gh label create "priority:high" --description "High priority" --color "d73a4a"
gh label create "priority:medium" --description "Medium priority" --color "fbca04"
gh label create "priority:low" --description "Low priority" --color "c5def5"

# Status labels
gh label create "blocked" --description "Blocked by another issue or external factor" --color "e11d21"
gh label create "in-progress" --description "Work in progress" --color "fbca04"
gh label create "needs-review" --description "Needs code review" --color "0e8a16"
gh label create "ready" --description "Ready to be worked on" --color "0e8a16"

echo "Labels created successfully!"
