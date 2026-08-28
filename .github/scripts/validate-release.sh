#!/usr/bin/env bash
set -euo pipefail

release_tag="${1:-${GITHUB_REF_NAME:-}}"

# Build metadata is refused outright rather than stripped and re-gated.
# `2.0.0+meta` has exactly the same semver precedence as `2.0.0`, so accepting
# both would let one release be published under two different tag strings, and
# every downstream comparison -- manifest versions, the changelog heading, the
# crates.io version, and the stable-release evidence gate below -- would then
# have to agree on the same normalisation to stay honest. Refusing keeps one
# canonical tag per release and removes that whole class of bypass.
if [[ "${release_tag}" == *+* ]]; then
    echo "release tag must not carry semver build metadata (received: ${release_tag}); +metadata has the same release precedence as the base version, so tag the canonical vX.Y.Z instead" >&2
    exit 1
fi

# Anchored on purpose: no surrounding whitespace, no uppercase `V`, no
# `refs/tags/` prefix, and no leading zeroes in the numeric identifiers.
if [[ ! "${release_tag}" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]]; then
    echo "release tag must have the form vX.Y.Z or vX.Y.Z-prerelease (received: ${release_tag:-<empty>})" >&2
    exit 1
fi

release_version="${release_tag#v}"

python3 - "${release_version}" <<'PY'
import json
import html
import pathlib
import re
import os
import subprocess
import sys
import tomllib

expected = sys.argv[1]
root = pathlib.Path.cwd()

# The version is re-parsed here instead of trusting the shell shape check
# above: this predicate decides whether the hardware-evidence gate runs at all,
# so it has to fail closed on anything it does not fully understand.
version_match = re.fullmatch(
    r"(?P<major>0|[1-9][0-9]*)"
    r"\.(?P<minor>0|[1-9][0-9]*)"
    r"\.(?P<patch>0|[1-9][0-9]*)"
    r"(?:-(?P<prerelease>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?P<build>\+.*)?",
    expected,
)
if version_match is None:
    raise SystemExit(f"tag version {expected} is not a plain semantic version")
if version_match.group("build") is not None:
    raise SystemExit(
        f"tag version {expected} carries semver build metadata; build metadata "
        "does not change release precedence, so it is not an accepted release "
        "identity"
    )

# Every stable release from 2.0.0 onward carries the hardware claim, so the
# evidence gate keys off "major >= 2 and not a pre-release" rather than the
# literal major version 2. Candidate tags (-rc.N and friends) stay exempt:
# they are publishable while their hardware rows are still marked pending.
requires_hardware_evidence = (
    int(version_match.group("major")) >= 2
    and version_match.group("prerelease") is None
)

metadata = subprocess.run(
    # This is intentionally lockfile-independent.  grafton-visca is a library
    # workspace and does not track Cargo.lock; the publication workflow creates
    # an ignored lockfile later, immediately before locked packaging.
    ["cargo", "metadata", "--format-version", "1", "--no-deps"],
    check=True,
    capture_output=True,
    text=True,
).stdout

packages = {package["name"]: package for package in json.loads(metadata)["packages"]}
for name in ("grafton-visca", "grafton-visca-macros"):
    actual = packages[name]["version"]
    if actual != expected:
        raise SystemExit(f"{name} version {actual} does not match tag version {expected}")

manifest = tomllib.loads((root / "Cargo.toml").read_text())
macro_dependency = manifest["dependencies"]["grafton-visca-macros"]["version"]
if macro_dependency != f"={expected}":
    raise SystemExit(
        "grafton-visca-macros dependency version "
        f"{macro_dependency} is not pinned to the tag version ={expected}"
    )

changelog = (root / "CHANGELOG.md").read_text()
heading = re.compile(
    rf"^## \[{re.escape(expected)}\] - \d{{4}}-\d{{2}}-\d{{2}}$", re.MULTILINE
)
if not heading.search(changelog):
    raise SystemExit(
        f"CHANGELOG.md must contain a dated '## [{expected}] - YYYY-MM-DD' heading"
    )

# Stable publication from 2.0.0 onward requires physical-camera evidence in
# addition to the software gates above.
if requires_hardware_evidence:
    checklist = root / (
        os.environ.get("HARDWARE_CHECKLIST") or "docs/hardware_release_checklist.md"
    )
    if not checklist.is_file():
        raise SystemExit(f"missing hardware release checklist: {checklist}")

    # Remove only whitespace and complete, surrounding Markdown emphasis. Do
    # not strip emphasis characters from inside a value: `P_T-01` and
    # `P_a_ss` must not turn into valid `PT-01`/`Pass` values.
    def normalise(cell):
        value = cell.strip()
        wrappers = (("**", "**"), ("__", "__"), ("*", "*"), ("_", "_"), ("`", "`"))
        changed = True
        while changed:
            changed = False
            for opening, closing in wrappers:
                if value.startswith(opening) and value.endswith(closing):
                    inner = value[len(opening) : -len(closing)].strip()
                    if inner != value:
                        value = inner
                        changed = True
                        break
        return value

    separator = re.compile(r"^:?-{2,}:?$")
    unevidenced = re.compile(
        r"^(pending|blocked|fail(ed)?|tbd|todo)\b", re.IGNORECASE
    )
    placeholder = re.compile(
        r"^(pending|blocked|fail(ed)?|tbd|todo|n/?a|none|unknown|xxx)\b",
        re.IGNORECASE,
    )

    def visible_markdown_text(value):
        """Project a checklist value to the text a Markdown reader sees."""
        text = normalise(value)
        text = html.unescape(text)

        # A zero-width character is not visible in rendered Markdown and must
        # not split a placeholder word.  Keep this to the common format and
        # joiner characters rather than attempting Unicode confusable folding.
        text = re.sub(r"[\u200b\u200c\u200d\ufeff]", "", text)

        # Strip the small HTML surface that can wrap or prefix a value.  This
        # is deliberately a visible-text projection, not an HTML parser:
        # comments are discarded and ordinary tags are removed while their
        # text content remains.  Unescape first so `&lt;span&gt;Pending...`
        # follows the same path as literal tags.
        text = re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
        # Keep the tag matcher deliberately bounded, but make quoted
        # attributes atomic with respect to `>`: a wrapper such as
        # `<span title="x > y">Pending</span>` must project to `Pending`.
        # The required whitespace after the tag name also keeps autolinks such
        # as `<https://example.test/evidence>` out of this HTML-ish surface.
        html_tag = re.compile(
            r"</?[A-Za-z][A-Za-z0-9]*(?:[ \t]+(?:[^'\"> \t\n]|'[^'\n]*'|\"[^\"\n]*\")*)*[ \t]*/?>"
        )
        text = html_tag.sub("", text)

        # Markdown backslash escapes are presentation syntax.  Unescape only
        # punctuation that Markdown treats specially; ordinary backslashes in
        # paths and prose remain intact.
        markdown_escape = re.compile(r"\\([\\`*_{}\[\]()#+.!>~|:<>-])")
        text = markdown_escape.sub(r"\1", text)

        # Remove only presentation prefixes. Repeating this bounded number
        # of times handles nested quotes/lists without becoming a Markdown
        # parser, and leaves paths and URLs untouched.
        for _ in range(8):
            previous = text
            text = re.sub(r"^\s{0,3}(?:>\s*)+", "", text)
            text = re.sub(r"^\s{0,3}#{1,6}[ \t]*", "", text)
            text = re.sub(
                r"^\s{0,3}(?:[-+*]|[0-9]{1,9}[.)])[ \t]+", "", text
            )
            text = re.sub(r"^\s{0,3}\[[ xX]\][ \t]+", "", text)
            if text == previous:
                break

        inline_link = re.compile(
            r"!?\[([^\]]*)\]\((?:[^()\n]|\([^()\n]*\))*\)"
        )
        reference_link = re.compile(r"!?\[([^\]]*)\]\[[^\]]*\]")
        malformed_link = re.compile(r"!?\[([^\]]*)\]\([^\n]*$")
        bracket_label = re.compile(r"!?\[([^\]]*)\]")
        unmatched_label = re.compile(r"^!?\[([^\]\n]*)$")
        for _ in range(8):
            previous = text
            text = inline_link.sub(r"\1", text)
            text = reference_link.sub(r"\1", text)
            text = malformed_link.sub(r"\1", text)
            text = bracket_label.sub(r"\1", text)
            text = unmatched_label.sub(r"\1", text)
            if text == previous:
                break

        # Complete outer underscore wrappers are handled by `normalise`.
        # Handle a wrapper split inside a word as well, while preserving
        # ordinary internal underscores such as `P_a_ss`.
        underscore_wrapper = re.compile(r"^(_{1,3})(.*?)(?:\1)(.*)$")
        for _ in range(4):
            match = underscore_wrapper.match(text)
            if match is None:
                break
            text = match.group(2) + match.group(3)

        # Asterisks, code ticks, and strike markers are presentation syntax
        # even when malformed or split through a visible word.
        text = re.sub(r"[*~`]", "", text)
        return re.sub(r"\s+", " ", text).strip()

    def is_placeholder(value):
        if value is None:
            return True
        candidate = visible_markdown_text(value)
        if not candidate or candidate in {"...", "…"}:
            return True
        if re.fullmatch(r"[-–—]+", candidate):
            return True
        return placeholder.match(candidate) is not None

    def recorded_evidence(value):
        return not is_placeholder(value)

    # These are the row identifiers in the checked-in 2.0 hardware matrix.
    # Generate the ranges in one place so adding a row is an intentional
    # validator change rather than a subtly incomplete hand-maintained list.
    required_ids = (
        [f"PT-{number:02d}" for number in range(1, 24)]
        + [f"FW-{number:02d}" for number in range(1, 10)]
        + [f"CR-{number:02d}" for number in range(1, 7)]
        + [f"RT-{number:02d}" for number in range(1, 5)]
        + [f"MC-{number:02d}" for number in range(1, 6)]
    )
    required_id_set = set(required_ids)
    required_rows = {identifier: [] for identifier in required_ids}

    incomplete = []
    nonpassing = []
    ambiguous_headers = []
    table_evidence_errors = []
    required_status_errors = []
    required_evidence_errors = []
    status_column = None
    id_column = None
    evidence_column = None
    table_started = False
    status_rows = 0
    def without_html_comments(document):
        """Remove HTML comments while retaining every source line boundary."""
        visible = []
        in_comment = False
        for source_line in document.splitlines(keepends=True):
            if source_line.endswith("\r\n"):
                body, line_ending = source_line[:-2], "\r\n"
            elif source_line.endswith(("\n", "\r")):
                body, line_ending = source_line[:-1], source_line[-1:]
            else:
                body, line_ending = source_line, ""

            pieces = []
            cursor = 0
            while cursor < len(body):
                if in_comment:
                    end = body.find("-->", cursor)
                    if end < 0:
                        cursor = len(body)
                        break
                    cursor = end + len("-->")
                    in_comment = False
                    continue

                start = body.find("<!--", cursor)
                if start < 0:
                    pieces.append(body[cursor:])
                    cursor = len(body)
                    break
                pieces.append(body[cursor:start])
                cursor = start + len("<!--")
                in_comment = True

            visible.append("".join(pieces) + line_ending)
        return "".join(visible)

    def visible_document(document):
        """Project the checklist to visible lines before parsing tables/records.

        This is intentionally a bounded document-context pass, not a Markdown
        parser. HTML comments are removed first so fence markers inside a
        comment cannot affect the state machine. Fenced blocks use the common
        0--3-space, three-or-more backtick/tilde form and retain only line
        boundaries while hidden.
        """
        document = without_html_comments(document)
        visible = []
        fence_character = None
        fence_length = 0
        opening = re.compile(r"^ {0,3}(?P<run>`{3,}|~{3,})(?P<suffix>.*)$")

        for source_line in document.splitlines(keepends=True):
            if source_line.endswith("\r\n"):
                body, line_ending = source_line[:-2], "\r\n"
            elif source_line.endswith(("\n", "\r")):
                body, line_ending = source_line[:-1], source_line[-1:]
            else:
                body, line_ending = source_line, ""

            if fence_character is not None:
                prefix = re.match(
                    rf"^ {{0,3}}(?P<run>{re.escape(fence_character)}+)(?P<rest>.*)$",
                    body,
                )
                if (
                    prefix is not None
                    and len(prefix.group("run")) >= fence_length
                    and not prefix.group("rest").strip()
                ):
                    fence_character = None
                    fence_length = 0
                visible.append(line_ending)
                continue

            match = opening.match(body)
            if match is not None:
                marker = match.group("run")
                # CommonMark disallows backticks in a backtick fence's info
                # string. Applying that small constraint avoids hiding a
                # visible line that merely starts with an invalid fence.
                if marker[0] == "`" and "`" in match.group("suffix"):
                    visible.append(source_line)
                    continue
                fence_character = marker[0]
                fence_length = len(marker)
                visible.append(line_ending)
                continue

            visible.append(source_line)
        return "".join(visible)

    # Both table rows and top-level release records must come from the same
    # visible-document projection. Keeping line boundaries makes diagnostics
    # continue to point at the original checklist lines.
    checklist_text = visible_document(checklist.read_text())
    for line_number, line in enumerate(checklist_text.splitlines(), 1):
        if "|" not in line:
            status_column = None
            id_column = None
            evidence_column = None
            table_started = False
            continue

        first_row = not table_started
        table_started = True
        cells = [normalise(cell) for cell in line.split("|")]
        folded = [cell.casefold() for cell in cells]

        # The checked-in matrix tables use `ID`, `Status`, and an
        # `Evidence...`/`...notes` column. Remember their positions per table;
        # a future table may add columns without changing the validator.
        status_columns = [index for index, cell in enumerate(folded) if cell == "status"]
        id_columns = [index for index, cell in enumerate(folded) if cell == "id"]
        evidence_columns = [
            index
            for index, cell in enumerate(folded)
            if "evidence" in cell or "notes" in cell
        ]
        # `Evidence` can legitimately occur in a data value (for example,
        # "packet evidence"), so an evidence-like cell alone is not enough to
        # identify a header. A recognized table header has at least `ID` or
        # `Status`; those markers also let us reject duplicate evidence labels.
        if first_row and (status_columns or id_columns):
            if len(status_columns) > 1:
                ambiguous_headers.append(
                    f"line {line_number}: {len(status_columns)} Status columns"
                )
            if len(evidence_columns) > 1:
                ambiguous_headers.append(
                    f"line {line_number}: {len(evidence_columns)} Evidence-like columns"
                )
            if len(id_columns) > 1:
                ambiguous_headers.append(
                    f"line {line_number}: {len(id_columns)} ID columns"
                )
            status_column = status_columns[0] if len(status_columns) == 1 else None
            id_column = id_columns[0] if len(id_columns) == 1 else None
            evidence_column = (
                evidence_columns[0] if len(evidence_columns) == 1 else None
            )
            continue

        separator_row = (
            any(separator.match(cell) for cell in cells)
            and all(not cell or separator.match(cell) for cell in cells)
        )
        if separator_row:
            continue

        for cell in cells:
            projected = visible_markdown_text(cell)
            # The table-wide rule is intentionally narrower than the
            # evidence/sign-off predicates: arbitrary checklist cells may use
            # punctuation such as an em dash for a serial `Default` value.
            # Only visible pending/non-passing markers are release blockers.
            # Boundary cells from Markdown's leading/trailing pipes are empty
            # structural cells, so do not report those as placeholders.
            if projected and unevidenced.match(projected):
                incomplete.append(f"line {line_number}: {cell}")

        if status_column is not None and status_column < len(cells):
            status = cells[status_column]
            status_rows += 1
            if status != "Pass":
                nonpassing.append(f"line {line_number}: {status or '<empty>'}")

        if status_column is not None and evidence_column is not None:
            evidence = (
                cells[evidence_column] if evidence_column < len(cells) else None
            )
            if not recorded_evidence(evidence):
                table_evidence_errors.append(
                    f"line {line_number}: {evidence or '<empty>'}"
                )

        if id_column is None or id_column >= len(cells):
            continue
        identifier = cells[id_column]
        if identifier not in required_id_set:
            # Future rows are allowed, but only the current required IDs are
            # subject to exact-cardinality and per-row evidence checks below.
            continue

        status = (
            cells[status_column]
            if status_column is not None and status_column < len(cells)
            else None
        )
        evidence = (
            cells[evidence_column]
            if evidence_column is not None and evidence_column < len(cells)
            else None
        )
        required_rows[identifier].append(
            {
                "line": line_number,
                "status": status,
                "evidence": evidence,
            }
        )

    if ambiguous_headers:
        sample = "; ".join(ambiguous_headers[:5])
        suffix = "" if len(ambiguous_headers) <= 5 else f"; ... ({len(ambiguous_headers)} total)"
        raise SystemExit(
            "stable 2.0+ publication rejects ambiguous checklist table headers: "
            f"{sample}{suffix}"
        )

    if table_evidence_errors:
        sample = "; ".join(table_evidence_errors[:5])
        suffix = (
            ""
            if len(table_evidence_errors) <= 5
            else f"; ... ({len(table_evidence_errors)} total)"
        )
        message = (
            "stable 2.0+ publication requires every data row under a "
            "Status+Evidence table to have recorded non-placeholder "
            f"evidence/notes: {sample}{suffix}"
        )
        if incomplete:
            message += f"; incomplete rows: {'; '.join(incomplete[:5])}"
        raise SystemExit(message)

    if incomplete:
        sample = "; ".join(incomplete[:5])
        suffix = "" if len(incomplete) <= 5 else f"; ... ({len(incomplete)} total)"
        raise SystemExit(
            "stable 2.0+ publication requires every hardware/checklist row to "
            f"have evidence; incomplete rows: {sample}{suffix}"
        )
    if nonpassing:
        sample = "; ".join(nonpassing[:5])
        suffix = "" if len(nonpassing) <= 5 else f"; ... ({len(nonpassing)} total)"
        raise SystemExit(
            "stable 2.0+ publication requires every hardware/checklist status "
            f"to be Pass; non-Pass status cells: {sample}{suffix}"
        )
    # A checklist with no recognisable `Status` table proves nothing, so an
    # emptied or restructured file must fail rather than pass by omission.
    if status_rows == 0:
        raise SystemExit(
            "stable 2.0+ publication requires hardware evidence rows; "
            f"{checklist} has no table row under a 'Status' column"
        )

    missing_ids = [
        identifier for identifier in required_ids if not required_rows[identifier]
    ]
    duplicate_ids = [
        f"{identifier} ({len(required_rows[identifier])} rows)"
        for identifier in required_ids
        if len(required_rows[identifier]) > 1
    ]
    if missing_ids:
        sample = ", ".join(missing_ids[:12])
        suffix = "" if len(missing_ids) <= 12 else f", ... ({len(missing_ids)} total)"
        raise SystemExit(
            "stable 2.0+ publication requires every current hardware checklist "
            f"ID exactly once; missing required IDs: {sample}{suffix}"
        )
    if duplicate_ids:
        sample = "; ".join(duplicate_ids[:12])
        suffix = "" if len(duplicate_ids) <= 12 else f"; ... ({len(duplicate_ids)} total)"
        raise SystemExit(
            "stable 2.0+ publication requires every current hardware checklist "
            f"ID exactly once; duplicate required IDs: {sample}{suffix}"
        )

    for identifier in required_ids:
        row = required_rows[identifier][0]
        status = row["status"]
        if status is None or status != "Pass":
            required_status_errors.append(
                f"{identifier} (line {row['line']}: {status or '<empty>'})"
            )
        if not recorded_evidence(row["evidence"]):
            evidence = row["evidence"] or "<empty>"
            required_evidence_errors.append(
                f"{identifier} (line {row['line']}: {evidence})"
            )

    if required_status_errors:
        sample = "; ".join(required_status_errors[:5])
        suffix = (
            ""
            if len(required_status_errors) <= 5
            else f"; ... ({len(required_status_errors)} total)"
        )
        raise SystemExit(
            "stable 2.0+ publication requires every required hardware checklist "
            f"row to have Status exactly Pass; invalid required rows: {sample}{suffix}"
        )
    if required_evidence_errors:
        sample = "; ".join(required_evidence_errors[:5])
        suffix = (
            ""
            if len(required_evidence_errors) <= 5
            else f"; ... ({len(required_evidence_errors)} total)"
        )
        raise SystemExit(
            "stable 2.0+ publication requires every required hardware checklist "
            f"row to have recorded non-placeholder evidence/notes: {sample}{suffix}"
        )

    # The five release-signoff gates are deliberately covered by the existing
    # table-wide Status/evidence scan below. The checked-in contract gives
    # those gates prose labels rather than stable IDs, and RELEASING.md
    # requires every Status row plus the two top-level records, not exact gate
    # label cardinality; imposing a second label schema would reject valid
    # future additions without strengthening the stated release invariant.
    def recorded(label):
        pattern = re.compile(rf"^{re.escape(label)}:[ \t]*(.*)$", re.MULTILINE)
        for match in pattern.finditer(checklist_text):
            value = normalise(match.group(1))
            if len(value) >= 2 and not is_placeholder(value):
                return value
        return None

    if recorded("Final sign-off") is None or recorded("Evidence index") is None:
        raise SystemExit(
            "stable 2.0+ publication requires non-placeholder 'Final sign-off:' "
            f"and 'Evidence index:' records in {checklist}"
        )
PY

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    echo "version=${release_version}" >> "${GITHUB_OUTPUT}"
fi

echo "release metadata is consistent for ${release_tag}"
