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

    separator = re.compile(r"^:?-+:?$")
    unevidenced = re.compile(
        r"^(pending|blocked|fail(ed)?|tbd|todo)\b", re.IGNORECASE
    )
    # A stable release cannot turn an explicit statement that the observation
    # did not happen into provenance merely by marking the row Pass.  Keep
    # these denial phrases distinct from the narrower table-wide `unevidenced`
    # scan above: this predicate is used for the required provenance fields
    # and the two top-level release records, where a value has to be evidence
    # rather than merely non-pending text.
    placeholder = re.compile(
        r"^(?:"
        r"pending|blocked|fail(?:ed)?|tbd|todo|n/?a|none|unknown|xxx"
        r"|not[ \t/_\-\u2013\u2014]+(?:run|tested|performed|executed|verified|recorded|available|provided|applicable)"
        r"|no[ \t/_\-\u2013\u2014]+(?:evidence|test(?:ing)?|run|record(?:s)?|artifact(?:s)?|proof|verification|data|logs?|result(?:s)?)"
        r"|un(?:tested|verified|recorded|available)"
        r"|missing(?:[ \t/_\-\u2013\u2014]+(?:evidence|record(?:s)?|artifact(?:s)?|proof))?"
        r"|absent|unavailable"
        r")\b",
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
        + [f"CR-{number:02d}" for number in range(1, 8)]
        + [f"RT-{number:02d}" for number in range(1, 5)]
        + [f"MC-{number:02d}" for number in range(1, 6)]
    )
    required_id_set = set(required_ids)
    required_rows = {identifier: [] for identifier in required_ids}

    incomplete = []
    nonpassing = []
    ambiguous_headers = []
    required_table_structure_errors = []
    table_evidence_errors = []
    required_status_errors = []
    required_field_errors = []
    checked_required_table_headers = set()
    status_rows = 0
    def without_html_comments(document):
        """Remove comments and report every source line they occupied."""
        visible = []
        in_comment = False
        comment_lines = set()
        for line_number, source_line in enumerate(document.splitlines(keepends=True), 1):
            if source_line.endswith("\r\n"):
                body, line_ending = source_line[:-2], "\r\n"
            elif source_line.endswith(("\n", "\r")):
                body, line_ending = source_line[:-1], source_line[-1:]
            else:
                body, line_ending = source_line, ""

            pieces = []
            cursor = 0
            line_has_comment = False
            while cursor < len(body):
                if in_comment:
                    line_has_comment = True
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
                line_has_comment = True
                cursor = start + len("<!--")
                in_comment = True

            visible.append("".join(pieces) + line_ending)
            if line_has_comment:
                comment_lines.add(line_number)
        return "".join(visible), comment_lines

    def visible_document(document):
        """Project the checklist to visible lines before parsing tables/records.

        This is intentionally a bounded document-context pass, not a Markdown
        parser. HTML comments are removed first so fence markers inside a
        comment cannot affect the state machine. Fenced blocks, four-space
        indented code blocks, CommonMark/Pandoc raw blocks, and recognized
        line-starting block/raw HTML elements retain only line boundaries
        while hidden. Inline/custom elements and unpaired ordinary or void
        HTML elements remain visible so they cannot hide the rest of the
        checklist.
        """
        document, html_comment_lines = without_html_comments(document)
        visible = []
        fence_character = None
        fence_length = 0
        html_block_tag = None
        raw_block_terminator = None
        type_6_html_block = False
        indented_code = False
        opening = re.compile(r"^ {0,3}(?P<run>`{3,}|~{3,})(?P<suffix>.*)$")
        html_block_opening = re.compile(
            r"^ {0,3}<(?P<tag>[A-Za-z][A-Za-z0-9:-]*)\b", re.IGNORECASE
        )
        type_1_html_block_tags = {"pre", "script", "style", "textarea"}
        commonmark_type_6_html_tags = frozenset(
            (
                "address article aside base basefont blockquote body caption "
                "center col colgroup dd details dialog dir div dl dt fieldset "
                "figcaption figure footer form frame frameset h1 h2 h3 h4 h5 "
                "h6 head header hr html iframe legend li link main menu "
                "menuitem nav noframes ol optgroup option p param search "
                "section summary table tbody td tfoot th thead title tr track ul"
            ).split()
        )
        # The existing visibility contract also treats the two historical
        # raw/code containers below as paired raw contexts.
        paired_raw_html_tags = {"code", "xmp"}

        source_lines = document.splitlines(keepends=True)
        for source_index, source_line in enumerate(source_lines):
            if source_line.endswith("\r\n"):
                body, line_ending = source_line[:-2], "\r\n"
            elif source_line.endswith(("\n", "\r")):
                body, line_ending = source_line[:-1], source_line[-1:]
            else:
                body, line_ending = source_line, ""

            if raw_block_terminator is not None:
                if raw_block_terminator in body:
                    raw_block_terminator = None
                visible.append(line_ending)
                continue

            if html_block_tag is not None:
                if re.search(
                    rf"</{re.escape(html_block_tag)}\s*>", body, re.IGNORECASE
                ):
                    html_block_tag = None
                visible.append(line_ending)
                continue

            if type_6_html_block:
                if not body.strip():
                    type_6_html_block = False
                visible.append(line_ending)
                continue

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

            # Indented code blocks continue through blank lines. A subsequent
            # nonblank line with fewer than four spaces starts fresh parsing.
            if indented_code:
                if not body.strip() or re.match(r"^(?: {4}|\t)", body):
                    visible.append(line_ending)
                    continue
                indented_code = False

            if re.match(r"^(?: {4}|\t)", body):
                indented_code = True
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

            if re.match(r"^ {0,3}<\?", body):
                raw_block_terminator = "?>"
            elif re.match(r"^ {0,3}<!\[CDATA\[", body):
                raw_block_terminator = "]]>"
            elif re.match(r"^ {0,3}<![A-Z]", body):
                raw_block_terminator = ">"

            if raw_block_terminator is not None:
                if raw_block_terminator in body:
                    raw_block_terminator = None
                visible.append(line_ending)
                continue

            html_match = html_block_opening.match(body)
            if html_match is not None:
                tag = html_match.group("tag").casefold()
                closing = re.compile(rf"</{re.escape(tag)}\s*>", re.IGNORECASE)
                remaining = body[html_match.end() :]
                # CommonMark type-1 raw HTML blocks run through their
                # matching closing tag or EOF. Restrict this exception to
                # syntactically valid type-1 openers so `<textarea/>` and
                # ordinary unpaired elements do not hide the document.
                is_type_1_html_block = (
                    tag in type_1_html_block_tags
                    and (not remaining or remaining[0].isspace() or remaining[0] == ">")
                )
                if is_type_1_html_block:
                    if closing.search(remaining) is None:
                        html_block_tag = tag
                    visible.append(line_ending)
                    continue

                is_complete_block_html_opener = (
                    not remaining
                    or remaining[0].isspace()
                    or remaining.startswith(">")
                    or remaining.startswith("/>")
                )
                # CommonMark type-6 blocks end at the first blank line,
                # rather than at a matching end tag.
                if (
                    tag in commonmark_type_6_html_tags
                    and is_complete_block_html_opener
                ):
                    type_6_html_block = True
                    visible.append(line_ending)
                    continue

                # Only the recognized paired raw/code elements use this
                # closer-based projection. In particular, a paired `<span>`
                # or custom element remains visible so its Markdown content
                # is still parsed.
                is_paired_raw_html_element = (
                    tag in paired_raw_html_tags and is_complete_block_html_opener
                )
                if not is_paired_raw_html_element:
                    visible.append(source_line)
                    continue

                # A recognized paired raw/code opener is a hiding block only
                # when its matching closer exists on this line or later in
                # the document. This keeps the bounded projection from
                # treating an unclosed `<code>` or `<xmp>` as EOF-wide.
                remainder = "".join(source_lines[source_index:])
                if closing.search(remainder) is None:
                    visible.append(source_line)
                    continue
                if closing.search(remaining) is None:
                    html_block_tag = tag
                visible.append(line_ending)
                continue

            visible.append(source_line)
        return "".join(visible), html_comment_lines

    # Both table rows and top-level release records must come from the same
    # visible-document projection. Keeping line boundaries makes diagnostics
    # continue to point at the original checklist lines.
    checklist_text, html_comment_lines = visible_document(checklist.read_text())

    def markdown_table_cells(line):
        """Return normalized and raw cells for a nonempty pipe-table row."""
        stripped = line.strip()
        if "|" not in stripped:
            return None
        raw_cells = stripped.split("|")
        if stripped.startswith("|"):
            raw_cells = raw_cells[1:]
        if stripped.endswith("|"):
            raw_cells = raw_cells[:-1]
        if not raw_cells or not any(cell.strip() for cell in raw_cells):
            return None
        return [normalise(cell) for cell in raw_cells], raw_cells

    def is_table_delimiter(raw_cells, header_width, has_html_comment):
        return (
            not has_html_comment
            and len(raw_cells) == header_width
            and all(separator.fullmatch(cell.strip()) for cell in raw_cells)
        )

    checklist_lines = checklist_text.splitlines()
    atx_heading = re.compile(r"^ {0,3}#{1,6}(?:[ \t]|$)")
    line_index = 0
    while line_index < len(checklist_lines):
        header_source = checklist_lines[line_index]
        if (
            header_source != header_source.lstrip(" \t")
            or (
                line_index > 0
                and checklist_lines[line_index - 1].strip()
                and atx_heading.match(checklist_lines[line_index - 1]) is None
            )
        ):
            line_index += 1
            continue
        header_row = markdown_table_cells(checklist_lines[line_index])
        delimiter_row = (
            markdown_table_cells(checklist_lines[line_index + 1])
            if line_index + 1 < len(checklist_lines)
            else None
        )
        if (
            header_row is None
            or delimiter_row is None
            or not is_table_delimiter(
                delimiter_row[1],
                len(header_row[0]),
                line_index + 2 in html_comment_lines,
            )
        ):
            line_index += 1
            continue

        header_line = line_index + 1
        header_cells = header_row[0]
        folded = [cell.casefold() for cell in header_cells]

        # The checked-in matrix tables use `ID`, `Owner`, `Status`,
        # `Firmware / bench`, and `Evidence artifact / notes`. Remember their
        # positions per table; a future table may add other columns without
        # changing this contract.
        status_columns = [index for index, cell in enumerate(folded) if cell == "status"]
        id_columns = [index for index, cell in enumerate(folded) if cell == "id"]
        owner_columns = [index for index, cell in enumerate(folded) if cell == "owner"]
        firmware_bench_columns = [
            index
            for index, cell in enumerate(folded)
            if re.fullmatch(r"firmware\s*/\s*bench", cell)
        ]
        evidence_columns = [
            index
            for index, cell in enumerate(folded)
            if "evidence" in cell or "notes" in cell
        ]
        required_evidence_columns = [
            index
            for index, cell in enumerate(folded)
            if re.fullmatch(r"evidence\s+artifact\s*/\s*notes", cell)
        ]
        # `Evidence` can legitimately occur in a data value (for example,
        # "packet evidence"), so an evidence-like cell alone is not enough to
        # identify a header. A recognized matrix/release header has at least
        # `ID` or `Status`; those markers also let us reject duplicate labels.
        if status_columns or id_columns:
            if len(status_columns) > 1:
                ambiguous_headers.append(
                    f"line {header_line}: {len(status_columns)} Status columns"
                )
            if len(evidence_columns) > 1:
                ambiguous_headers.append(
                    f"line {header_line}: {len(evidence_columns)} Evidence-like columns"
                )
            if len(id_columns) > 1:
                ambiguous_headers.append(
                    f"line {header_line}: {len(id_columns)} ID columns"
                )
            if len(owner_columns) > 1:
                ambiguous_headers.append(
                    f"line {header_line}: {len(owner_columns)} Owner columns"
                )
            if len(firmware_bench_columns) > 1:
                ambiguous_headers.append(
                    f"line {header_line}: {len(firmware_bench_columns)} Firmware / bench columns"
                )

        status_column = status_columns[0] if len(status_columns) == 1 else None
        id_column = id_columns[0] if len(id_columns) == 1 else None
        owner_column = owner_columns[0] if len(owner_columns) == 1 else None
        firmware_bench_column = (
            firmware_bench_columns[0] if len(firmware_bench_columns) == 1 else None
        )
        evidence_column = (
            evidence_columns[0] if len(evidence_columns) == 1 else None
        )
        required_evidence_column = (
            required_evidence_columns[0]
            if len(required_evidence_columns) == 1
            else None
        )

        data_index = line_index + 2
        while data_index < len(checklist_lines):
            table_row = markdown_table_cells(checklist_lines[data_index])
            if table_row is None or len(table_row[0]) != len(header_cells):
                break
            cells = table_row[0]
            line_number = data_index + 1

            for cell in cells:
                projected = visible_markdown_text(cell)
                # The table-wide rule is intentionally narrower than the
                # evidence/sign-off predicates: arbitrary checklist cells may
                # use punctuation such as an em dash for a serial `Default`
                # value. Only visible pending/non-passing markers are release
                # blockers.
                if projected and unevidenced.match(projected):
                    incomplete.append(f"line {line_number}: {cell}")

            if status_column is not None:
                status = cells[status_column]
                status_rows += 1
                if status != "Pass":
                    nonpassing.append(f"line {line_number}: {status or '<empty>'}")

            if status_column is not None and evidence_column is not None:
                status = cells[status_column]
                evidence = cells[evidence_column]
                if status == "Pass" and not recorded_evidence(evidence):
                    table_evidence_errors.append(
                        f"line {line_number}: {evidence or '<empty>'}"
                    )

            if id_column is not None:
                identifier = cells[id_column]
                row = {
                    "line": line_number,
                    "identifier": identifier,
                    "status": cells[status_column] if status_column is not None else None,
                    "owner": cells[owner_column] if owner_column is not None else None,
                    "firmware_bench": (
                        cells[firmware_bench_column]
                        if firmware_bench_column is not None
                        else None
                    ),
                    "evidence": (
                        cells[required_evidence_column]
                        if required_evidence_column is not None
                        else None
                    ),
                }
                if identifier in required_id_set:
                    if header_line not in checked_required_table_headers:
                        checked_required_table_headers.add(header_line)
                        missing_columns = [
                            label
                            for label, column in (
                                ("ID", id_column),
                                ("Owner", owner_column),
                                ("Status", status_column),
                                ("Firmware / bench", firmware_bench_column),
                                (
                                    "Evidence artifact / notes",
                                    required_evidence_column,
                                ),
                            )
                            if column is None
                        ]
                        if missing_columns:
                            required_table_structure_errors.append(
                                f"line {header_line}: missing {', '.join(missing_columns)}"
                            )
                    required_rows[identifier].append(row)

            data_index += 1

        line_index = data_index

    if ambiguous_headers:
        sample = "; ".join(ambiguous_headers[:5])
        suffix = "" if len(ambiguous_headers) <= 5 else f"; ... ({len(ambiguous_headers)} total)"
        raise SystemExit(
            "stable 2.0+ publication rejects ambiguous checklist table headers: "
            f"{sample}{suffix}"
        )

    # Required hardware rows belong to the five checked-in matrix tables. A
    # smaller ID/Status/Evidence table can list every ID while omitting the
    # owner and exact firmware/bench provenance that the checklist requires,
    # so its header is not sufficient stable-release evidence.
    if required_table_structure_errors:
        sample = "; ".join(required_table_structure_errors[:5])
        suffix = (
            ""
            if len(required_table_structure_errors) <= 5
            else f"; ... ({len(required_table_structure_errors)} total)"
        )
        raise SystemExit(
            "stable 2.0+ publication requires every table containing a current "
            "hardware checklist ID to include ID, Owner, Status, Firmware / bench, "
            "and Evidence artifact / notes columns; incomplete required-row tables: "
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

    # A `Pass` claim needs the same operator, firmware/bench, and evidence
    # provenance as the checked-in matrix. This verifies that the values are
    # recorded, without claiming to authenticate the physical-camera
    # observation itself.
    for identifier in required_ids:
        row = required_rows[identifier][0]
        if row["status"] != "Pass":
            continue
        for label, field in (
            ("Owner", "owner"),
            ("Firmware / bench", "firmware_bench"),
            ("Evidence artifact / notes", "evidence"),
        ):
            if not recorded_evidence(row[field]):
                value = row[field] or "<empty>"
                required_field_errors.append(
                    f"{identifier} (line {row['line']}: {label} is {value})"
                )
    if required_field_errors:
        sample = "; ".join(required_field_errors[:5])
        suffix = (
            ""
            if len(required_field_errors) <= 5
            else f"; ... ({len(required_field_errors)} total)"
        )
        raise SystemExit(
            "stable 2.0+ publication requires every Pass row in a required-ID "
            "table to record non-placeholder Owner, Firmware / bench, and "
            f"Evidence artifact / notes values: {sample}{suffix}"
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
