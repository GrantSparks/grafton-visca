#!/bin/sh
# Pre-commit hook to remove AI assistant co-author lines from commit messages
# This ensures commits don't contain automated attribution lines

set -e  # Exit on error

COMMIT_MSG_FILE="$1"

# Validate input
if [ -z "$COMMIT_MSG_FILE" ]; then
    echo "Error: No commit message file provided" >&2
    exit 1
fi

if [ ! -f "$COMMIT_MSG_FILE" ]; then
    echo "Error: Commit message file does not exist: $COMMIT_MSG_FILE" >&2
    exit 1
fi

if [ ! -r "$COMMIT_MSG_FILE" ]; then
    echo "Error: Cannot read commit message file: $COMMIT_MSG_FILE" >&2
    exit 1
fi

# Create secure temporary file
TMP_FILE=$(mktemp "${TMPDIR:-/tmp}/commit-msg.XXXXXX") || {
    echo "Error: Failed to create temporary file" >&2
    exit 1
}

# Ensure cleanup on exit
trap 'rm -f "$TMP_FILE"' EXIT INT TERM

# Process the commit message
# 1. Remove lines with AI co-authorship (case-insensitive, various formats)
# 2. Remove lines with bot/generated markers
# 3. Clean up resulting blank lines
sed -E \
    -e '/^[[:space:]]*$/!b' \
    -e ':a' \
    -e '/^[[:space:]]*$/N' \
    -e '//ba' \
    "$COMMIT_MSG_FILE" | \
sed -E \
    -e '/^[[:space:]]*co-authored-by:[[:space:]]*claude/Id' \
    -e '/^[[:space:]]*co-authored-by:[[:space:]]*.*\[bot\]/Id' \
    -e '/^[[:space:]]*co-authored-by:[[:space:]]*AI[[:space:]]*$/Id' \
    -e '/^[[:space:]]*🤖[[:space:]]*generated/Id' \
    -e '/^[[:space:]]*generated[[:space:]]+by[[:space:]]+claude/Id' \
    -e '/^[[:space:]]*signed-off-by:[[:space:]]*claude/Id' \
    | \
awk '
    # Remove leading blank lines and collapse multiple blank lines to one
    BEGIN { blank_count = 0; content_started = 0 }
    /^[[:space:]]*$/ {
        if (content_started) blank_count++
        next
    }
    {
        content_started = 1
        if (blank_count > 0) {
            print ""
            blank_count = 0
        }
        print
    }
' > "$TMP_FILE"

# Check if we have any content left
if [ ! -s "$TMP_FILE" ]; then
    # If file is empty after processing, restore original
    # (we don't want to accidentally delete entire commit messages)
    echo "Warning: Commit message became empty after processing, keeping original" >&2
else
    # Check if content actually changed
    if ! cmp -s "$COMMIT_MSG_FILE" "$TMP_FILE"; then
        # Replace original with processed version
        cp "$TMP_FILE" "$COMMIT_MSG_FILE" || {
            echo "Error: Failed to update commit message file" >&2
            exit 1
        }
    fi
fi

exit 0
