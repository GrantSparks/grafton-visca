#!/usr/bin/env python3
"""Validate changelog and commit-history integrity for a pull-request range."""

from __future__ import annotations

import difflib
import pathlib
import re
import subprocess
import sys


CHANGELOG = "CHANGELOG.md"
API_SNAPSHOT_GLOB = "api/2.0.0-rc.1/*.txt"
BODY_POLICY_BOUNDARY = ".github/change-record-body-policy-boundary"
HISTORY_POLICY_BOUNDARY = ".github/change-record-history-policy-boundary"
VERSION_COMPONENT = r"(?:0|[1-9][0-9]*)"
STRICT_DATED_RELEASE_HEADING = re.compile(
    rf"\A## \["
    rf"{VERSION_COMPONENT}\.{VERSION_COMPONENT}\.{VERSION_COMPONENT}"
    rf"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    rf"\] - \d{{4}}-\d{{2}}-\d{{2}}(?:\r?\n|\Z)"
)


class ValidationError(RuntimeError):
    """A malformed repository/range that cannot be validated safely."""


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["git", *args],
        check=False,
        capture_output=True,
        text=True,
    )
    if check and result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise ValidationError(f"git {' '.join(args)} failed: {detail}")
    return result


def git_file(revision: str, path: str, *, required: bool = True) -> str | None:
    # Do not use text mode here: its universal-newline conversion would make a
    # CRLF-to-LF rewrite of released history indistinguishable from the
    # original. Markdown must be UTF-8, and decoding its raw bytes preserves
    # the exact suffix that the history policy protects.
    result = subprocess.run(
        ["git", "show", f"{revision}:{path}"],
        check=False,
        capture_output=True,
    )
    if result.returncode == 0:
        try:
            return result.stdout.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValidationError(
                f"cannot read {path} at {revision} as UTF-8: {error}"
            ) from error
    if required:
        detail = (
            result.stderr.decode("utf-8", "replace").strip()
            or "path does not exist"
        )
        raise ValidationError(f"cannot read {path} at {revision}: {detail}")
    return None


def split_unreleased(text: str, revision: str) -> tuple[str, str, str, int]:
    heading = re.compile(r"(?m)^## \[Unreleased\][ \t]*(?:\r?\n|$)")
    matches = list(heading.finditer(text))
    if len(matches) != 1:
        raise ValidationError(
            f"{CHANGELOG} at {revision} must contain exactly one "
            "'## [Unreleased]' heading"
        )

    body_start = matches[0].end()
    next_heading = re.search(r"(?m)^## [^\r\n]+(?:\r?\n|$)", text[body_start:])
    body_end = (
        body_start + next_heading.start() if next_heading is not None else len(text)
    )

    return (
        text[:body_start],
        text[body_start:body_end],
        text[body_end:],
        body_start,
    )


def immutable_changelog(prefix: str, suffix: str) -> str:
    """Replace the mutable Unreleased body with a comparison sentinel."""

    return prefix + "\0UNRELEASED-BODY\0" + suffix


def lines_are_retained_in_order(previous: str, current: str) -> bool:
    """Require every prior changelog line to remain, allowing release additions."""

    current_lines = iter(current.splitlines(keepends=True))
    return all(
        any(candidate == line for candidate in current_lines)
        for line in previous.splitlines(keepends=True)
    )


def release_cut_body(
    base_prefix: str,
    base_unreleased: str,
    base_suffix: str,
    head_prefix: str,
    head_unreleased: str,
    head_suffix: str,
) -> tuple[str, int] | None:
    """Recognize one safe Unreleased-to-dated release transition.

    A release cut may add explanatory release text, but it cannot alter the
    existing changelog prefix or released suffix, drop or reorder prior notes,
    or introduce another top-level section before the retained suffix.
    """

    if base_prefix != head_prefix or head_unreleased.strip():
        return None

    heading = STRICT_DATED_RELEASE_HEADING.match(head_suffix)
    if heading is None:
        return None

    if base_suffix:
        if not head_suffix.endswith(base_suffix):
            return None
        inserted = head_suffix[: -len(base_suffix)]
    else:
        inserted = head_suffix

    release_body = inserted[heading.end() :]
    if re.search(r"(?m)^## [^\r\n]+", release_body):
        return None
    if not lines_are_retained_in_order(base_unreleased, release_body):
        return None

    release_body_offset = len(head_prefix) + len(head_unreleased) + heading.end()
    return release_body, release_body_offset


def validated_release_cut(
    history_start: str,
    head: str,
    history_immutable: str,
    head_prefix: str,
    head_suffix: str,
) -> tuple[str, str, int] | None:
    """Find one release cut whose parent and released suffix remain intact.

    The history policy boundary may predate the commit that actually cuts the
    release. Every edit to Unreleased before that cut remains ordinary mutable
    work, so the cut is verified against its direct parent instead. The
    policy-boundary immutable view still protects every pre-existing release.
    """

    commits = git(
        "rev-list", "--reverse", f"{history_start}..{head}", "--", CHANGELOG
    ).stdout.splitlines()
    cuts: list[tuple[str, str, int]] = []

    for commit in commits:
        parents = git("show", "-s", "--format=%P", commit).stdout.split()
        if len(parents) != 1:
            continue

        parent = parents[0]
        parent_changelog = git_file(parent, CHANGELOG)
        cut_changelog = git_file(commit, CHANGELOG)
        assert parent_changelog is not None
        assert cut_changelog is not None

        parent_prefix, parent_unreleased, parent_suffix, _ = split_unreleased(
            parent_changelog, parent
        )
        cut_prefix, cut_unreleased, cut_suffix, _ = split_unreleased(
            cut_changelog, commit
        )
        cut = release_cut_body(
            parent_prefix,
            parent_unreleased,
            parent_suffix,
            cut_prefix,
            cut_unreleased,
            cut_suffix,
        )
        if cut is None:
            continue

        if immutable_changelog(parent_prefix, parent_suffix) != history_immutable:
            continue
        if head_prefix != cut_prefix or head_suffix != cut_suffix:
            continue

        release_body, release_body_offset = cut
        cuts.append((cut_changelog, release_body, release_body_offset))

    if len(cuts) != 1:
        return None
    return cuts[0]


def changed_paths(start: str, end: str, pathspec: str) -> list[str]:
    result = git(
        "diff",
        "--name-only",
        "--diff-filter=ACDMRTUXB",
        start,
        end,
        "--",
        pathspec,
    )
    return [line for line in result.stdout.splitlines() if line]


def path_changed(start: str, end: str, path: str) -> bool:
    result = git("diff", "--quiet", start, end, "--", path, check=False)
    if result.returncode not in (0, 1):
        detail = result.stderr.strip() or result.stdout.strip()
        raise ValidationError(f"cannot compare {path}: {detail}")
    return result.returncode == 1


def is_ancestor(ancestor: str, descendant: str) -> bool:
    result = git("merge-base", "--is-ancestor", ancestor, descendant, check=False)
    if result.returncode not in (0, 1):
        detail = result.stderr.strip() or result.stdout.strip()
        raise ValidationError(
            f"cannot test ancestry of {ancestor} and {descendant}: {detail}"
        )
    return result.returncode == 0


def policy_boundary(
    marker: str, revision: str, marker_path: str, policy: str
) -> str:
    boundary = marker.strip()
    if re.fullmatch(r"[0-9a-f]{40}", boundary) is None:
        raise ValidationError(
            f"{marker_path} at {revision} must contain one full lowercase commit ID"
        )

    object_check = git("cat-file", "-e", f"{boundary}^{{commit}}", check=False)
    if object_check.returncode != 0:
        raise ValidationError(
            f"{policy} policy boundary {boundary} is unavailable; use a full-history checkout"
        )

    return boundary


def policy_start(merge_base: str, head: str, marker_path: str, policy: str) -> str:
    base_marker = git_file(merge_base, marker_path, required=False)
    head_marker = git_file(head, marker_path, required=False)

    # A marker is a reviewed, immutable policy boundary once it is present in
    # the merge base. Reading a replacement from HEAD would let a PR advance the
    # marker past its own noncompliant commits and remove them from validation.
    if base_marker is not None:
        boundary = policy_boundary(base_marker, merge_base, marker_path, policy)
        if head_marker is None:
            raise ValidationError(
                f"{marker_path} at {head} must retain the merge-base marker "
                f"{boundary}; it is missing"
            )
        head_boundary = policy_boundary(head_marker, head, marker_path, policy)
        if head_boundary != boundary:
            raise ValidationError(
                f"{marker_path} at {head} must retain the merge-base marker "
                f"{boundary}; found {head_boundary}"
            )
    elif head_marker is None:
        return merge_base
    else:
        # A branch whose base predates this policy may establish its reviewed
        # boundary once. Future ranges must retain the merge-base marker above.
        boundary = policy_boundary(head_marker, head, marker_path, policy)

    # A bootstrap boundary inside this range starts enforcement at itself. A
    # pre-existing boundary is behind the merge base, so normal PR validation
    # continues to start at the merge base.
    if is_ancestor(merge_base, boundary) and is_ancestor(boundary, head):
        return boundary
    return merge_base


def breaking_bullet_errors(changelog: str, body: str, offset: int) -> list[str]:
    errors: list[str] = []
    starts = list(re.finditer(r"(?m)^- [^\n]*\*\*BREAKING\*\*", body))
    boundary_pattern = re.compile(r"(?m)^(?:- |#{2,6} )")

    for start in starts:
        following = boundary_pattern.search(body, start.end())
        end = following.start() if following is not None else len(body)
        bullet = body[start.start() : end]
        if re.search(r"\(#[1-9][0-9]*\)", bullet) is None:
            line = changelog.count("\n", 0, offset + start.start()) + 1
            errors.append(
                f"{CHANGELOG}:{line}: `**BREAKING**` bullet is missing an issue "
                "reference in the form `(#NNN)`"
            )
    return errors


def commit_body_errors(start: str, head: str) -> list[str]:
    errors: list[str] = []
    commits = git("rev-list", "--reverse", f"{start}..{head}").stdout.splitlines()
    for commit in commits:
        touched = git(
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-m",
            commit,
            "--",
            "src/",
        ).stdout.splitlines()
        if not touched:
            continue
        body = git("show", "-s", "--format=%b", commit).stdout
        if not body.strip():
            subject = git("show", "-s", "--format=%s", commit).stdout.strip()
            errors.append(
                f"commit {commit[:12]} ({subject}) touches src/ but has an empty commit body"
            )
    return errors


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(
            "usage: validate-change-record.py <base-revision> [head-revision]",
            file=sys.stderr,
        )
        return 2

    base = argv[1]
    head = argv[2] if len(argv) == 3 else "HEAD"

    try:
        merge_base = git("merge-base", base, head).stdout.strip()
        if not merge_base:
            raise ValidationError(f"{base} and {head} have no merge base")

        history_start = policy_start(
            merge_base, head, HISTORY_POLICY_BOUNDARY, "changelog-history"
        )
        base_changelog = git_file(history_start, CHANGELOG)
        head_changelog = git_file(head, CHANGELOG)
        assert base_changelog is not None
        assert head_changelog is not None

        base_prefix, base_unreleased, base_suffix, _ = split_unreleased(
            base_changelog, history_start
        )
        head_prefix, unreleased, head_suffix, unreleased_offset = split_unreleased(
            head_changelog, head
        )
        base_immutable = immutable_changelog(base_prefix, base_suffix)
        head_immutable = immutable_changelog(head_prefix, head_suffix)
        release_cut = validated_release_cut(
            history_start,
            head,
            base_immutable,
            head_prefix,
            head_suffix,
        )

        errors: list[str] = []
        if base_immutable != head_immutable and release_cut is None:
            diff = list(
                difflib.unified_diff(
                    base_immutable.splitlines(),
                    head_immutable.splitlines(),
                    fromfile=f"{CHANGELOG}@{history_start[:12]} outside Unreleased",
                    tofile=f"{CHANGELOG}@{head[:12]} outside Unreleased",
                    lineterm="",
                )
            )
            preview = "\n".join(diff[:12])
            errors.append(
                f"{CHANGELOG} content outside [Unreleased] is immutable; restore released "
                f"history and add a superseding Unreleased entry\n{preview}"
            )

        snapshots = changed_paths(merge_base, head, API_SNAPSHOT_GLOB)
        if snapshots and not path_changed(merge_base, head, CHANGELOG):
            errors.append(
                "public API snapshot changes require a CHANGELOG.md hunk in the same "
                f"change ({', '.join(snapshots)})"
            )

        errors.extend(
            breaking_bullet_errors(head_changelog, unreleased, unreleased_offset)
        )
        if release_cut is not None:
            release_changelog, release_body, release_body_offset = release_cut
            errors.extend(
                breaking_bullet_errors(
                    release_changelog, release_body, release_body_offset
                )
            )
        errors.extend(
            commit_body_errors(
                policy_start(merge_base, head, BODY_POLICY_BOUNDARY, "commit-body"),
                head,
            )
        )
    except ValidationError as error:
        print(f"change-record validation error: {error}", file=sys.stderr)
        return 2

    if errors:
        for error in errors:
            print(f"change-record violation: {error}", file=sys.stderr)
        return 1

    print(
        "change-record validation passed: released history is immutable outside "
        "validated release cuts, "
        "API snapshots are recorded, breaking bullets are referenced, and src/ "
        "commits have bodies"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
