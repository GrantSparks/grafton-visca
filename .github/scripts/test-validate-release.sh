#!/usr/bin/env bash
set -euo pipefail

# Regression checks for the release validator. The fixtures deliberately live
# in mktemp directories so these checks never modify the real hardware
# checklist or require a tracked Cargo.lock.
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
validator="${script_dir}/validate-release.sh"

make_fixture() {
    local destination="$1"
    local version="$2"
    local checklist="$3"

    mkdir -p "${destination}/grafton-visca-macros/src" \
        "${destination}/src" "${destination}/docs"
    python3 - "${destination}" "${version}" "${checklist}" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
version = sys.argv[2]
checklist = sys.argv[3]

(root / "Cargo.toml").write_text(f'''[workspace]
members = [".", "grafton-visca-macros"]
resolver = "2"

[workspace.package]
version = "{version}"
edition = "2021"

[package]
name = "grafton-visca"
version.workspace = true
edition.workspace = true

[dependencies]
grafton-visca-macros = {{ path = "grafton-visca-macros", version = "={version}" }}
''')
(root / "grafton-visca-macros/Cargo.toml").write_text(f'''[package]
name = "grafton-visca-macros"
version.workspace = true
edition.workspace = true

[lib]
proc-macro = true
''')
(root / "src/lib.rs").write_text("#![allow(dead_code)]\n")
(root / "grafton-visca-macros/src/lib.rs").write_text("\n")
(root / "CHANGELOG.md").write_text(
    f"# Changelog\n\n## [{version}] - 2026-08-26\n"
)
(root / "docs/hardware_release_checklist.md").write_text(checklist)
PY
}

expect_failure() {
    local expected_text="$1"
    shift
    local log_file
    log_file="$(mktemp)"
    local status=0
    "$@" >"${log_file}" 2>&1 || status=$?
    if [[ "${status}" -eq 0 ]]; then
        cat "${log_file}"
        rm -f "${log_file}"
        echo "expected validator failure containing: ${expected_text}" >&2
        return 1
    fi
    if ! grep -Fq "${expected_text}" "${log_file}"; then
        cat "${log_file}" >&2
        rm -f "${log_file}"
        echo "validator failure did not contain: ${expected_text}" >&2
        return 1
    fi
    rm -f "${log_file}"
}

run_validator() {
    local root="$1"
    local version="$2"
    (cd "${root}" && \
        HARDWARE_CHECKLIST=docs/hardware_release_checklist.md \
        bash "${validator}" "${version}")
}

# Generate matrix fixtures from the same compact ID ranges as the checked-in
# contract. Variants mutate one row so structural and evidence regressions do
# not require hand-copying all 48 required rows into every fixture.
make_checklist() {
    local variant="$1"
    python3 - "${variant}" <<'PY'
import sys

variant = sys.argv[1]
required_ids = (
    [f"PT-{number:02d}" for number in range(1, 24)]
    + [f"FW-{number:02d}" for number in range(1, 10)]
    + [f"CR-{number:02d}" for number in range(1, 8)]
    + [f"RT-{number:02d}" for number in range(1, 5)]
    + [f"MC-{number:02d}" for number in range(1, 6)]
)

def owner_for(identifier):
    if identifier.startswith("PT-"):
        return "Transport QA"
    if identifier.startswith("FW-"):
        return "Profile QA"
    if identifier.startswith("CR-"):
        return "Lifecycle QA"
    if identifier.startswith("RT-"):
        return "Runtime QA"
    return "Multi-camera QA"

def row(identifier, status="Pass", owner=None, firmware_bench=None, evidence=None):
    if owner is None:
        owner = owner_for(identifier)
    if firmware_bench is None:
        firmware_bench = "Camera firmware 1.2.3 / bench rack A"
    if evidence is None:
        evidence = f"docs/evidence/2.0.0/{identifier.lower()}.md#capture"
    if variant == "reordered-emphasis":
        return (
            f"| **{evidence}** | **{firmware_bench}** | **{status}** | "
            f"**{owner}** | **{identifier}** |"
        )
    if variant == "complete-em-dash":
        serial_ids = {"PT-03", "PT-06", "PT-09", "PT-12", "PT-15", "PT-18", "PT-21"}
        transport = "Blocking serial / Raw" if identifier in serial_ids else "TCP / Raw"
        default = "—" if identifier in serial_ids else "5678"
        return (
            f"| {identifier} | Fixture profile | {transport} | {default} | {owner} | "
            f"{status} | {firmware_bench} | {evidence} |"
        )
    if variant == "missing-owner-column":
        return f"| {identifier} | {status} | {firmware_bench} | {evidence} |"
    if variant == "missing-firmware-bench-column":
        return f"| {identifier} | {owner} | {status} | {evidence} |"
    if variant == "missing-evidence-column":
        return f"| {identifier} | {owner} | {status} | {firmware_bench} |"
    return f"| {identifier} | {owner} | {status} | {firmware_bench} | {evidence} |"

lines = [
    "# Hardware evidence",
    "",
    "| ID | Owner | Status | Firmware / bench | Evidence artifact / notes |",
    "| --- | --- | --- | --- | --- |",
]
if variant == "emphasised":
    lines[2] = "| ID | Owner | **Status** | Firmware / bench | Evidence artifact / notes |"
elif variant == "duplicate-status":
    lines[2] = "| ID | Owner | Status | Status | Firmware / bench | Evidence artifact / notes |"
    lines[3] = "| --- | --- | --- | --- | --- | --- |"
elif variant == "duplicate-evidence":
    lines[2] = "| ID | Owner | Status | Firmware / bench | Evidence | Evidence artifact / notes |"
    lines[3] = "| --- | --- | --- | --- | --- | --- |"
elif variant == "reordered-emphasis":
    lines[2] = "| Evidence artifact / notes | Firmware / bench | **Status** | Owner | **ID** |"
elif variant == "complete-em-dash":
    lines[2] = "| ID | Profile | Transport | Default | Owner | Status | Firmware / bench | Evidence artifact / notes |"
    lines[3] = "| --- | --- | --- | --- | --- | --- | --- | --- |"
elif variant == "missing-owner-column":
    lines[2] = "| ID | Status | Firmware / bench | Evidence artifact / notes |"
    lines[3] = "| --- | --- | --- | --- |"
elif variant == "missing-firmware-bench-column":
    lines[2] = "| ID | Owner | Status | Evidence artifact / notes |"
    lines[3] = "| --- | --- | --- | --- |"
elif variant == "missing-evidence-column":
    lines[2] = "| ID | Owner | Status | Firmware / bench |"
    lines[3] = "| --- | --- | --- | --- |"

for identifier in required_ids:
    if variant == "missing" and identifier == "PT-01":
        continue
    if variant == "missing-cr-07" and identifier == "CR-07":
        continue

    row_identifier = identifier
    status = "Pass"
    owner = None
    firmware_bench = None
    evidence = None
    if variant == "pending":
        status = "Pending (Not run)"
        firmware_bench = "Pending"
        evidence = "Pending — transcript and packet capture"
    if identifier == "PT-01":
        if variant == "renamed":
            row_identifier = "ARBITRARY-01"
        elif variant == "internal-id":
            row_identifier = "P_T-01"
        elif variant == "blank-owner":
            owner = ""
        elif variant == "placeholder-owner":
            owner = "N/A"
        elif variant == "blank-firmware-bench":
            firmware_bench = ""
        elif variant == "placeholder-firmware-bench":
            firmware_bench = "N/A"
        elif variant == "blank-evidence":
            evidence = ""
        elif variant == "placeholder-evidence":
            evidence = "N/A"
        elif variant == "lowercase":
            status = "pass"
        elif variant == "internal-status":
            status = "P_a_ss"
        elif variant == "internal-evidence":
            evidence = "P_a_ss"
        elif variant == "malformed-row-placeholder":
            evidence = "**Pending*"
        elif variant == "formatted-evidence":
            evidence = "**[capture](https://example.test/PT-01)**"
        elif variant == "dash-containing-evidence":
            evidence = "bench--capture"
        elif variant == "autolink-evidence":
            evidence = "<https://example.test/PT-01>"
        elif variant == "blocked":
            status, evidence = "Blocked", "bench issue"
        elif variant in {"nonpassing", "emphasised"}:
            status, evidence = (
                ("Review" if variant == "nonpassing" else "reviewed"),
                "docs/evidence/2.0.0/pt-01.md#capture",
            )
    lines.append(row(row_identifier, status, owner, firmware_bench, evidence))

if variant == "duplicate":
    lines.append(row("PT-01"))
elif variant == "future":
    lines.append(row("FUTURE-01", evidence="future evidence"))
elif variant in {"future-blank", "future-placeholder"}:
    evidence = "" if variant == "future-blank" else "Pending"
    lines.append(row("FUTURE-01", evidence=evidence))

lines.extend(["", "Final sign-off: Release owner 2026-08-26", "Evidence index: docs/evidence/2.0.0.md"])
if variant == "unsigned":
    lines = lines[:-2]
elif variant == "placeholder-signoff":
    lines[-2:] = ["Final sign-off: pending", "Evidence index: TBD"]
elif variant == "bold-placeholder-signoff":
    lines[-2:] = [
        "Final sign-off: **pending**",
        "Evidence index: docs/evidence/2.0.0.md",
    ]
elif variant == "inline-code-placeholder-evidence":
    lines[-2:] = [
        "Final sign-off: Release owner 2026-08-26",
        "Evidence index: `TBD`",
    ]
elif variant == "malformed-bold-placeholder-signoff":
    lines[-2:] = [
        "Final sign-off: **pending*",
        "Evidence index: docs/evidence/2.0.0.md",
    ]
elif variant == "malformed-bold-placeholder-evidence":
    lines[-2:] = [
        "Final sign-off: Release owner 2026-08-26",
        "Evidence index: **TBD*",
    ]
elif variant == "malformed-prefix-placeholder-signoff":
    lines[-2:] = [
        "Final sign-off: **pending",
        "Evidence index: docs/evidence/2.0.0.md",
    ]
elif variant == "malformed-trailing-placeholder-signoff":
    lines[-2:] = [
        "Final sign-off: **pending** extra",
        "Evidence index: docs/evidence/2.0.0.md",
    ]
elif variant == "formatted-evidence":
    lines[-2:] = [
        "Final sign-off: **Release owner 2026-08-26**",
        "Evidence index: [docs/evidence/2.0.0.md](https://example.test/evidence)",
    ]

print("\n".join(lines))
PY
}

make_corpus_checklist() {
    local field="$1"
    local disguise="$2"
    python3 - "${field}" "${disguise}" "$(make_checklist complete)" <<'PY'
import sys

field = sys.argv[1]
disguise = sys.argv[2]
checklist = sys.argv[3]

if field == "signoff":
    checklist = checklist.replace(
        "Final sign-off: Release owner 2026-08-26",
        f"Final sign-off: {disguise}",
        1,
    )
elif field == "evidence":
    checklist = checklist.replace(
        "Evidence index: docs/evidence/2.0.0.md",
        f"Evidence index: {disguise}",
        1,
    )
elif field == "row":
    checklist = checklist.replace(
        "| PT-01 | Transport QA | Pass | Camera firmware 1.2.3 / bench rack A | docs/evidence/2.0.0/pt-01.md#capture |",
        f"| PT-01 | Transport QA | Pass | Camera firmware 1.2.3 / bench rack A | {disguise} |",
        1,
    )
elif field == "table":
    # This value is intentionally outside the recognized ID/Status/Evidence
    # columns. The table-wide pending scan must still inspect it.
    checklist += (
        "\n\n| Gate | Owner | Result |\n"
        "| --- | --- | --- |\n"
        f"| Hardware note | Release QA | {disguise} |\n"
    )
else:
    raise SystemExit(f"unknown corpus field: {field}")

print(checklist)
PY
}

make_valid_control_checklist() {
    local field="$1"
    local control="$2"
    python3 - "${field}" "${control}" "$(make_checklist complete)" <<'PY'
import sys

field = sys.argv[1]
control = sys.argv[2]
checklist = sys.argv[3]

if field == "signoff":
    checklist = checklist.replace(
        "Final sign-off: Release owner 2026-08-26",
        f"Final sign-off: {control}",
        1,
    )
elif field == "evidence":
    checklist = checklist.replace(
        "Evidence index: docs/evidence/2.0.0.md",
        f"Evidence index: {control}",
        1,
    )
elif field == "row":
    checklist = checklist.replace(
        "| PT-01 | Transport QA | Pass | Camera firmware 1.2.3 / bench rack A | docs/evidence/2.0.0/pt-01.md#capture |",
        f"| PT-01 | Transport QA | Pass | Camera firmware 1.2.3 / bench rack A | {control} |",
        1,
    )
elif field == "table":
    # This value is intentionally outside the recognized ID/Status/Evidence
    # columns. It exercises the table-wide visible-text scan without making
    # punctuation in an unrelated checklist column a fake evidence failure.
    checklist += (
        "\n\n| Gate | Owner | Result |\n"
        "| --- | --- | --- |\n"
        f"| Hardware note | Release QA | {control} |\n"
    )
else:
    raise SystemExit(f"unknown control field: {field}")

print(checklist)
PY
}

# Build checklists that exercise the document-level visibility projection. The
# matrix and top-level records deliberately use the same complete fixture so a
# hidden table cannot accidentally be replaced by a smaller one-row control.
make_visibility_checklist() {
    local scenario="$1"
    python3 - "${scenario}" "$(make_checklist complete)" <<'PY'
import sys

scenario = sys.argv[1]
checklist = sys.argv[2]
matrix, record_tail = checklist.split("\n\nFinal sign-off:", 1)
records = "Final sign-off:" + record_tail
signoff, evidence = records.split("\n", 1)

def hidden(kind, content):
    if kind == "comment":
        return "<!-- hidden block\n" + content + "\n-->"
    if kind == "comment_unclosed":
        return "<!-- hidden block\n" + content
    if kind == "backtick":
        return "```markdown\n" + content + "\n```"
    if kind == "tilde":
        return "~~~markdown\n" + content + "\n~~~"
    if kind == "backtick-long":
        return "   ````markdown\n" + content + "\n   `````"
    if kind == "tilde-long":
        return "   ~~~~markdown\n" + content + "\n   ~~~~~"
    if kind == "backtick_unclosed":
        return "```markdown\n" + content
    raise SystemExit(f"unknown hidden kind: {kind}")

if scenario.startswith("records-"):
    scenario_parts = scenario.split("-")
    kind = "-".join(scenario_parts[1:-1])
    field = scenario_parts[-1]
    if field == "both":
        return_text = matrix + "\n\n" + hidden(kind, records)
    elif field == "signoff":
        return_text = matrix + "\n\n" + hidden(kind, signoff) + "\n" + evidence
    elif field == "evidence":
        return_text = matrix + "\n\n" + signoff + "\n" + hidden(kind, evidence)
    else:
        raise SystemExit(f"unknown record field: {field}")
elif scenario.startswith("matrix-"):
    _, kind = scenario.split("-", 1)
    return_text = hidden(kind, matrix) + "\n\n" + records
elif scenario.startswith("visible-"):
    scenario_parts = scenario.split("-")
    kind = "-".join(scenario_parts[1:-1])
    first = scenario_parts[-1]
    hidden_records = "Final sign-off: hidden\nEvidence index: hidden"
    if first == "signoff":
        return_text = (
            matrix + "\n\n" + signoff + "\n" + hidden(kind, hidden_records)
            + "\n" + evidence
        )
    elif first == "evidence":
        return_text = (
            matrix + "\n\n" + evidence + "\n" + hidden(kind, hidden_records)
            + "\n" + signoff
        )
    else:
        raise SystemExit(f"unknown visible-record order: {first}")
else:
    raise SystemExit(f"unknown visibility scenario: {scenario}")

print(return_text)
PY
}

# Keep the table-recognition checks separate from the visibility corpus above:
# the invalid variants contain a complete-looking matrix, but only `split` is
# rendered as visible Markdown tables.
make_table_context_checklist() {
    local scenario="$1"
    python3 - "${scenario}" "$(make_checklist complete)" <<'PY'
import sys

scenario = sys.argv[1]
checklist = sys.argv[2]
matrix, record_tail = checklist.split("\n\nFinal sign-off:", 1)
records = "Final sign-off:" + record_tail
lines = matrix.splitlines()

if scenario == "no-delimiter":
    # The complete fixture has its header and delimiter at fixed positions.
    del lines[3]
elif scenario == "mismatched-delimiter":
    lines[3] = "| --- | --- | --- | --- |"
elif scenario == "bold-delimiter":
    lines[3] = "| **---** | **---** | **---** | **---** | **---** |"
elif scenario == "code-delimiter":
    lines[3] = "| `---` | `---` | `---` | `---` | `---` |"
elif scenario == "comment-delimiter":
    lines[3] = "| <!--x-->--- | <!--x-->--- | <!--x-->--- | <!--x-->--- | <!--x-->--- |"
elif scenario == "indented":
    lines = ["    " + line if line else line for line in lines]
elif scenario == "pre":
    lines = ["<pre>", *lines, "</pre>"]
elif scenario == "code":
    lines = ["<code>", *lines, "</code>"]
elif scenario == "script":
    lines = ["<script>", *lines, "</script>"]
elif scenario == "style":
    lines = ["<style>", *lines, "</style>"]
elif scenario in {"textarea", "xmp"}:
    lines = [f"<{scenario}>", *lines, f"</{scenario}>"]
elif scenario == "div":
    # A type-6 block remains raw only until its first blank line.
    lines = ["<div>", *lines[2:], "</div>"]
elif scenario in {"unclosed-script", "unclosed-textarea"}:
    tag = scenario.removeprefix("unclosed-")
    lines = [f"<{tag}>", *lines]
elif scenario == "processing-instruction":
    lines = ["<?fixture", *lines, "?>"]
elif scenario == "cdata":
    lines = ["<![CDATA[", *lines, "]]>"]
elif scenario in {"unclosed-processing-instruction", "unclosed-cdata"}:
    opener = "<?fixture" if scenario.endswith("processing-instruction") else "<![CDATA["
    lines = [opener, *lines]
elif scenario == "multiline-pre":
    lines = ["<pre", 'class="fixture">', *lines, "</pre>"]
elif scenario == "paragraph-no-blank":
    lines = lines[:2] + ["Matrix narrative immediately before the header."] + lines[2:]
elif scenario == "list-continuation":
    lines = lines[:2] + ["- Matrix continuation:"] + [
        "  " + line if line else line for line in lines[2:]
    ]
elif scenario == "closed-html-boundary":
    lines = lines[:2] + ["<div>Closed ordinary HTML context.</div>", ""] + lines[2:]
elif scenario == "heading-boundary":
    del lines[1]
elif scenario in {"span-boundary", "custom-element-boundary"}:
    tag = "span" if scenario == "span-boundary" else "custom-element"
    lines = [f"<{tag}>", "", *lines, "", f"</{tag}>"]
elif scenario in {"div-boundary", "details-boundary"}:
    tag = "div" if scenario == "div-boundary" else "details"
    lines = [f"<{tag}>", "", *lines, "", f"</{tag}>"]
elif scenario == "split":
    header, delimiter = lines[2:4]
    rows = lines[4:]
    split_at = len(rows) // 2
    lines = (
        lines[:4]
        + rows[:split_at]
        + ["", "Matrix continuation.", "", header, delimiter]
        + rows[split_at:]
    )
else:
    raise SystemExit(f"unknown table-context scenario: {scenario}")

print("\n".join(lines) + "\n\n" + records)
PY
}

root_temp="$(mktemp -d)"
trap 'rm -rf "${root_temp}"' EXIT

# Candidate tags retain their hardware-evidence exemption, but use the same
# checked-in matrix shape so a Pending row is an honest, structurally complete
# candidate record rather than a smaller stand-in table.
pending_checklist="$(make_checklist pending)"
blocked_checklist="$(make_checklist blocked)"
unsigned_checklist="$(make_checklist unsigned)"
nonpassing_checklist="$(make_checklist nonpassing)"
complete_checklist="$(make_checklist complete)"
# Markdown emphasis around the header plus a lower-case status: neither may
# hide the status column, and only `Pass` counts.
emphasised_checklist="$(make_checklist emphasised)"
# No `Status` column anywhere: an emptied or restructured checklist proves
# nothing and must not pass by omission.
headerless_checklist=$'# Hardware evidence\n\nNo table here.\n\nFinal sign-off: Release owner 2026-08-26\nEvidence index: docs/evidence/2.0.0.md\n'
# Placeholder sign-off records are not sign-off.
placeholder_signoff_checklist="$(make_checklist placeholder-signoff)"

# Every form below projects to the visible placeholder word `Pending`; run
# each one independently against each of the three release-gate fields and an
# unrelated table cell. Dash-only forms are deliberately valid only in that
# unrelated table cell.
markdown_placeholder_forms=(
    'Pending'
    '**Pending**'
    '__Pending__'
    '*Pending*'
    '_Pending_'
    '`Pending`'
    '~~Pending~~'
    '*Pend*ing'
    '**Pend**ing'
    '__Pend__ing'
    '`Pend`ing'
    '~~Pend~~ing'
    '[Pending](https://example.test)'
    '![Pending](https://example.test)'
    '[Pending][reference]'
    '[Pending][]'
    '[]()Pending'
    '[Pending]()'
    '[Pending](https://example.test'
    '[Pending'
    '> Pending'
    '# Pending'
    '- Pending'
    '1. Pending'
    '[ ] Pending'
    '&gt; Pending'
    '--'
    '---'
    '<span title="x > y">Pending</span>'
    '<!--x-->Pending'
    '<del>Pending</del>'
    '\*Pending\*'
)
zero_width_pending=$'P\u200bending'
markdown_placeholder_forms+=("${zero_width_pending}")
test "${#markdown_placeholder_forms[@]}" -eq 33

rc_root="${root_temp}/rc"
make_fixture "${rc_root}" "2.0.0-rc.1" "${pending_checklist}"
test ! -e "${rc_root}/Cargo.lock"
(cd "${rc_root}" && bash "${validator}" v2.0.0-rc.1)

pending_root="${root_temp}/pending"
make_fixture "${pending_root}" "2.0.0" "${pending_checklist}"
expect_failure "incomplete rows" run_validator "${pending_root}" v2.0.0

blocked_root="${root_temp}/blocked"
make_fixture "${blocked_root}" "2.0.0" "${blocked_checklist}"
expect_failure "incomplete rows" run_validator "${blocked_root}" v2.0.0

unsigned_root="${root_temp}/unsigned"
make_fixture "${unsigned_root}" "2.0.0" "${unsigned_checklist}"
expect_failure "Final sign-off" run_validator "${unsigned_root}" v2.0.0

nonpassing_root="${root_temp}/nonpassing"
make_fixture "${nonpassing_root}" "2.0.0" "${nonpassing_checklist}"
expect_failure "non-Pass status cells" run_validator "${nonpassing_root}" v2.0.0

complete_root="${root_temp}/complete"
make_fixture "${complete_root}" "2.0.0" "${complete_checklist}"
run_validator "${complete_root}" v2.0.0

# Release records are valid only when they are visible in the checklist
# document. Multiline comments, fenced code blocks, and an unclosed block to
# EOF must not contribute hidden sign-off or evidence records.
for visibility_kind in comment backtick tilde; do
    for hidden_field in both signoff evidence; do
        visibility_root="${root_temp}/records-${visibility_kind}-${hidden_field}"
        make_fixture "${visibility_root}" "2.0.0" \
            "$(make_visibility_checklist "records-${visibility_kind}-${hidden_field}")"
        if [[ "${hidden_field}" == "signoff" || "${hidden_field}" == "both" ]]; then
            expected_failure="Final sign-off"
        else
            expected_failure="Evidence index"
        fi
        expect_failure "${expected_failure}" run_validator \
            "${visibility_root}" v2.0.0
    done
done

for unclosed_kind in comment_unclosed backtick_unclosed; do
    unclosed_root="${root_temp}/records-${unclosed_kind}"
    make_fixture "${unclosed_root}" "2.0.0" \
        "$(make_visibility_checklist "records-${unclosed_kind}-both")"
    expect_failure "Final sign-off" run_validator "${unclosed_root}" v2.0.0
done

# A complete matrix hidden in a comment/fence is not a visible Status table,
# even when both top-level records remain visible. Conversely, genuine records
# on either side of hidden content continue to satisfy the record gate.
for visibility_kind in comment backtick tilde backtick-long tilde-long; do
    hidden_matrix_root="${root_temp}/matrix-${visibility_kind}"
    make_fixture "${hidden_matrix_root}" "2.0.0" \
        "$(make_visibility_checklist "matrix-${visibility_kind}")"
    expect_failure "no table row under a 'Status' column" \
        run_validator "${hidden_matrix_root}" v2.0.0

    for visible_first in signoff evidence; do
        visible_records_root="${root_temp}/visible-${visibility_kind}-${visible_first}"
        make_fixture "${visible_records_root}" "2.0.0" \
            "$(make_visibility_checklist "visible-${visibility_kind}-${visible_first}")"
        run_validator "${visible_records_root}" v2.0.0
    done
done

# Candidate tags retain their exemption even for a document whose complete
# matrix is hidden; stable tags above must expose the matrix.
hidden_matrix_rc_root="${root_temp}/hidden-matrix-rc"
make_fixture "${hidden_matrix_rc_root}" "2.0.0-rc.1" \
    "$(make_visibility_checklist matrix-comment)"
run_validator "${hidden_matrix_rc_root}" v2.0.0-rc.1

# Pipe-delimited text is hardware evidence only when it is a visible Markdown
# table with an undecorated, matching header delimiter. Indented/list
# continuation text, paragraph-adjacent rows, and paired block/raw HTML
# elements must not become a table merely because they contain a
# complete-looking matrix.
for invalid_table_context in \
    no-delimiter \
    mismatched-delimiter \
    bold-delimiter \
    code-delimiter \
    comment-delimiter \
    indented \
    pre \
    code \
    script \
    style \
    textarea \
    xmp \
    div \
    unclosed-script \
    unclosed-textarea \
    processing-instruction \
    cdata \
    unclosed-processing-instruction \
    unclosed-cdata \
    paragraph-no-blank \
    list-continuation \
    multiline-pre; do
    invalid_table_root="${root_temp}/invalid-table-${invalid_table_context}"
    make_fixture "${invalid_table_root}" "2.0.0" \
        "$(make_table_context_checklist "${invalid_table_context}")"
    expect_failure "no table row under a 'Status' column" \
        run_validator "${invalid_table_root}" v2.0.0
done

# A paired ordinary HTML element may precede a genuine top-level table. The
# blank visible boundary keeps the table distinct from that raw HTML block.
closed_html_boundary_root="${root_temp}/closed-html-boundary"
make_fixture "${closed_html_boundary_root}" "2.0.0" \
    "$(make_table_context_checklist closed-html-boundary)"
run_validator "${closed_html_boundary_root}" v2.0.0

# GFM permits a table immediately after an ATX heading without a blank line.
heading_boundary_root="${root_temp}/heading-boundary"
make_fixture "${heading_boundary_root}" "2.0.0" \
    "$(make_table_context_checklist heading-boundary)"
run_validator "${heading_boundary_root}" v2.0.0

# Inline and custom HTML elements do not make their blank-line-separated
# Markdown table contents raw.
for visible_inline_context in span-boundary custom-element-boundary; do
    visible_inline_root="${root_temp}/visible-inline-${visible_inline_context}"
    make_fixture "${visible_inline_root}" "2.0.0" \
        "$(make_table_context_checklist "${visible_inline_context}")"
    run_validator "${visible_inline_root}" v2.0.0
done

# CommonMark type-6 HTML blocks end at their first blank line, so the
# blank-line-separated matrix below is visible after both block-level tags.
for visible_type_6_context in div-boundary details-boundary; do
    visible_type_6_root="${root_temp}/visible-type-6-${visible_type_6_context}"
    make_fixture "${visible_type_6_root}" "2.0.0" \
        "$(make_table_context_checklist "${visible_type_6_context}")"
    run_validator "${visible_type_6_root}" v2.0.0
done

# Required rows may be distributed across separate, individually valid tables
# just as they are in the checked-in hardware checklist.
split_table_root="${root_temp}/split-table"
make_fixture "${split_table_root}" "2.0.0" \
    "$(make_table_context_checklist split)"
run_validator "${split_table_root}" v2.0.0

lowercase_status_root="${root_temp}/lowercase-status"
make_fixture "${lowercase_status_root}" "2.0.0" "$(make_checklist lowercase)"
expect_failure "non-Pass status cells" run_validator "${lowercase_status_root}" v2.0.0

internal_id_root="${root_temp}/internal-id"
make_fixture "${internal_id_root}" "2.0.0" "$(make_checklist internal-id)"
expect_failure "missing required IDs" run_validator "${internal_id_root}" v2.0.0

internal_status_root="${root_temp}/internal-status"
make_fixture "${internal_status_root}" "2.0.0" "$(make_checklist internal-status)"
expect_failure "non-Pass status cells" run_validator "${internal_status_root}" v2.0.0

duplicate_status_root="${root_temp}/duplicate-status"
make_fixture "${duplicate_status_root}" "2.0.0" "$(make_checklist duplicate-status)"
expect_failure "ambiguous checklist table headers" run_validator "${duplicate_status_root}" v2.0.0

duplicate_evidence_root="${root_temp}/duplicate-evidence"
make_fixture "${duplicate_evidence_root}" "2.0.0" "$(make_checklist duplicate-evidence)"
expect_failure "ambiguous checklist table headers" run_validator "${duplicate_evidence_root}" v2.0.0

reordered_emphasis_root="${root_temp}/reordered-emphasis"
make_fixture "${reordered_emphasis_root}" "2.0.0" "$(make_checklist reordered-emphasis)"
run_validator "${reordered_emphasis_root}" v2.0.0

missing_id_root="${root_temp}/missing-id"
make_fixture "${missing_id_root}" "2.0.0" "$(make_checklist missing)"
expect_failure "missing required IDs" run_validator "${missing_id_root}" v2.0.0

# CR-07 is a current cancellation/recovery requirement. A completed-looking
# fixture without it must fail rather than silently treating CR-06 as the end
# of that range.
missing_cr_07_root="${root_temp}/missing-cr-07"
make_fixture "${missing_cr_07_root}" "2.0.0" "$(make_checklist missing-cr-07)"
expect_failure "missing required IDs: CR-07" \
    run_validator "${missing_cr_07_root}" v2.0.0

duplicate_id_root="${root_temp}/duplicate-id"
make_fixture "${duplicate_id_root}" "2.0.0" "$(make_checklist duplicate)"
expect_failure "duplicate required IDs" run_validator "${duplicate_id_root}" v2.0.0

renamed_id_root="${root_temp}/renamed-id"
make_fixture "${renamed_id_root}" "2.0.0" "$(make_checklist renamed)"
expect_failure "missing required IDs" run_validator "${renamed_id_root}" v2.0.0

# The hardware matrices need the checked-in provenance columns, not merely an
# ID/Status/Evidence subset. Each omission must be diagnosed as structural.
for missing_column_variant in \
    missing-owner-column \
    missing-firmware-bench-column \
    missing-evidence-column; do
    case "${missing_column_variant}" in
        missing-owner-column)
            expected_missing_column="missing Owner"
            ;;
        missing-firmware-bench-column)
            expected_missing_column="missing Firmware / bench"
            ;;
        missing-evidence-column)
            expected_missing_column="missing Evidence artifact / notes"
            ;;
    esac
    missing_column_root="${root_temp}/${missing_column_variant}"
    make_fixture "${missing_column_root}" "2.0.0" \
        "$(make_checklist "${missing_column_variant}")"
    expect_failure "${expected_missing_column}" \
        run_validator "${missing_column_root}" v2.0.0
done

# A Pass row has to identify its operator, exact firmware/bench provenance,
# and evidence artifact. Empty and placeholder values are not recorded facts.
blank_owner_root="${root_temp}/blank-owner"
make_fixture "${blank_owner_root}" "2.0.0" "$(make_checklist blank-owner)"
expect_failure "Owner is <empty>" run_validator "${blank_owner_root}" v2.0.0

placeholder_owner_root="${root_temp}/placeholder-owner"
make_fixture "${placeholder_owner_root}" "2.0.0" \
    "$(make_checklist placeholder-owner)"
expect_failure "Owner is N/A" run_validator "${placeholder_owner_root}" v2.0.0

blank_firmware_bench_root="${root_temp}/blank-firmware-bench"
make_fixture "${blank_firmware_bench_root}" "2.0.0" \
    "$(make_checklist blank-firmware-bench)"
expect_failure "Firmware / bench is <empty>" \
    run_validator "${blank_firmware_bench_root}" v2.0.0

placeholder_firmware_bench_root="${root_temp}/placeholder-firmware-bench"
make_fixture "${placeholder_firmware_bench_root}" "2.0.0" \
    "$(make_checklist placeholder-firmware-bench)"
expect_failure "Firmware / bench is N/A" \
    run_validator "${placeholder_firmware_bench_root}" v2.0.0

blank_evidence_root="${root_temp}/blank-evidence"
make_fixture "${blank_evidence_root}" "2.0.0" "$(make_checklist blank-evidence)"
expect_failure "recorded non-placeholder evidence/notes" run_validator "${blank_evidence_root}" v2.0.0

placeholder_evidence_root="${root_temp}/placeholder-evidence"
make_fixture "${placeholder_evidence_root}" "2.0.0" "$(make_checklist placeholder-evidence)"
expect_failure "recorded non-placeholder evidence/notes" run_validator "${placeholder_evidence_root}" v2.0.0

# Additional rows are permitted as the matrix grows, provided their status
# remains covered by the existing table-wide checks.
future_row_root="${root_temp}/future-row"
make_fixture "${future_row_root}" "2.0.0" "$(make_checklist future)"
run_validator "${future_row_root}" v2.0.0

future_blank_evidence_root="${root_temp}/future-blank-evidence"
make_fixture "${future_blank_evidence_root}" "2.0.0" "$(make_checklist future-blank)"
expect_failure "Status+Evidence table" run_validator "${future_blank_evidence_root}" v2.0.0

future_placeholder_evidence_root="${root_temp}/future-placeholder-evidence"
make_fixture "${future_placeholder_evidence_root}" "2.0.0" "$(make_checklist future-placeholder)"
expect_failure "Status+Evidence table" run_validator "${future_placeholder_evidence_root}" v2.0.0

# Issue #632: build metadata has the same semver precedence as the base
# version, so `v2.0.0+meta` used to be a stable 2.0 publication that skipped
# the hardware gate entirely. Such tags are refused outright.
metadata_root="${root_temp}/metadata"
make_fixture "${metadata_root}" "2.0.0+meta" "${pending_checklist}"
expect_failure "build metadata" run_validator "${metadata_root}" v2.0.0+meta
expect_failure "build metadata" run_validator "${metadata_root}" v2.0.0-rc.1+meta

# Issue #632: the gate keyed off the literal major version 2, so every later
# major skipped it. It applies to any stable release from 2.0.0 onward.
major_three_root="${root_temp}/major-three"
make_fixture "${major_three_root}" "3.0.0" "${pending_checklist}"
expect_failure "incomplete rows" run_validator "${major_three_root}" v3.0.0

major_twelve_root="${root_temp}/major-twelve"
make_fixture "${major_twelve_root}" "12.0.0" "${pending_checklist}"
expect_failure "incomplete rows" run_validator "${major_twelve_root}" v12.0.0

# ... and it stays satisfiable there, rather than rejecting later majors
# unconditionally.
major_three_complete_root="${root_temp}/major-three-complete"
make_fixture "${major_three_complete_root}" "3.0.0" "${complete_checklist}"
run_validator "${major_three_complete_root}" v3.0.0

# Pre-releases of any major keep the candidate exemption.
major_three_rc_root="${root_temp}/major-three-rc"
make_fixture "${major_three_rc_root}" "3.0.0-rc.1" "${pending_checklist}"
run_validator "${major_three_rc_root}" v3.0.0-rc.1

emphasised_root="${root_temp}/emphasised"
make_fixture "${emphasised_root}" "2.0.0" "${emphasised_checklist}"
expect_failure "non-Pass status cells" run_validator "${emphasised_root}" v2.0.0

headerless_root="${root_temp}/headerless"
make_fixture "${headerless_root}" "2.0.0" "${headerless_checklist}"
expect_failure "no table row under a 'Status' column" \
    run_validator "${headerless_root}" v2.0.0

placeholder_signoff_root="${root_temp}/placeholder-signoff"
make_fixture "${placeholder_signoff_root}" "2.0.0" "${placeholder_signoff_checklist}"
expect_failure "Final sign-off" run_validator "${placeholder_signoff_root}" v2.0.0

bold_placeholder_signoff_root="${root_temp}/bold-placeholder-signoff"
make_fixture "${bold_placeholder_signoff_root}" "2.0.0" \
    "$(make_checklist bold-placeholder-signoff)"
expect_failure "Final sign-off" run_validator "${bold_placeholder_signoff_root}" v2.0.0

inline_code_placeholder_evidence_root="${root_temp}/inline-code-placeholder-evidence"
make_fixture "${inline_code_placeholder_evidence_root}" "2.0.0" \
    "$(make_checklist inline-code-placeholder-evidence)"
expect_failure "Evidence index" run_validator "${inline_code_placeholder_evidence_root}" v2.0.0

# Malformed emphasis must not hide a placeholder from either top-level record
# or a required row's evidence cell.
for malformed_signoff_variant in \
    malformed-bold-placeholder-signoff \
    malformed-prefix-placeholder-signoff \
    malformed-trailing-placeholder-signoff; do
    malformed_signoff_root="${root_temp}/${malformed_signoff_variant}"
    make_fixture "${malformed_signoff_root}" "2.0.0" \
        "$(make_checklist "${malformed_signoff_variant}")"
    expect_failure "Final sign-off" run_validator "${malformed_signoff_root}" v2.0.0
done

malformed_bold_placeholder_evidence_root="${root_temp}/malformed-bold-placeholder-evidence"
make_fixture "${malformed_bold_placeholder_evidence_root}" "2.0.0" \
    "$(make_checklist malformed-bold-placeholder-evidence)"
expect_failure "Evidence index" run_validator \
    "${malformed_bold_placeholder_evidence_root}" v2.0.0

malformed_row_placeholder_root="${root_temp}/malformed-row-placeholder"
make_fixture "${malformed_row_placeholder_root}" "2.0.0" \
    "$(make_checklist malformed-row-placeholder)"
expect_failure "Status+Evidence table" run_validator \
    "${malformed_row_placeholder_root}" v2.0.0

# Internal underscores remain ordinary content, while genuine Markdown links
# and complete emphasis remain valid evidence.
internal_evidence_root="${root_temp}/internal-evidence"
make_fixture "${internal_evidence_root}" "2.0.0" \
    "$(make_checklist internal-evidence)"
run_validator "${internal_evidence_root}" v2.0.0

formatted_evidence_root="${root_temp}/formatted-evidence"
make_fixture "${formatted_evidence_root}" "2.0.0" \
    "$(make_checklist formatted-evidence)"
run_validator "${formatted_evidence_root}" v2.0.0

dash_containing_evidence_root="${root_temp}/dash-containing-evidence"
make_fixture "${dash_containing_evidence_root}" "2.0.0" \
    "$(make_checklist dash-containing-evidence)"
run_validator "${dash_containing_evidence_root}" v2.0.0

autolink_evidence_root="${root_temp}/autolink-evidence"
make_fixture "${autolink_evidence_root}" "2.0.0" \
    "$(make_checklist autolink-evidence)"
run_validator "${autolink_evidence_root}" v2.0.0

# Keep the adversarial corpus table-driven so each disguise is independently
# proven to fail in sign-off, evidence-index, and required-row evidence.
for form_index in "${!markdown_placeholder_forms[@]}"; do
    disguise="${markdown_placeholder_forms[form_index]}"
    for field in signoff evidence row table; do
        corpus_root="${root_temp}/corpus-${form_index}-${field}"
        make_fixture "${corpus_root}" "2.0.0" \
            "$(make_corpus_checklist "${field}" "${disguise}")"
        if [[ "${field}" == "table" && ( "${disguise}" == "--" || "${disguise}" == "---" ) ]]; then
            run_validator "${corpus_root}" v2.0.0
            continue
        elif [[ "${field}" == "signoff" ]]; then
            expected_failure="Final sign-off"
        elif [[ "${field}" == "evidence" ]]; then
            expected_failure="Evidence index"
        elif [[ "${field}" == "table" ]]; then
            expected_failure="incomplete rows"
        else
            expected_failure="Status+Evidence table"
        fi
        expect_failure "${expected_failure}" run_validator "${corpus_root}" v2.0.0
    done
done

# An em dash is also a placeholder for the three record fields, but is valid
# in an unrelated table cell. Keep this outside the 33x4 corpus so the corpus
# count remains stable while covering the Unicode form explicitly.
for field in signoff evidence row table; do
    em_dash_root="${root_temp}/em-dash-${field}"
    make_fixture "${em_dash_root}" "2.0.0" \
        "$(make_corpus_checklist "${field}" "—")"
    if [[ "${field}" == "table" ]]; then
        run_validator "${em_dash_root}" v2.0.0
    elif [[ "${field}" == "signoff" ]]; then
        expect_failure "Final sign-off" run_validator \
            "${em_dash_root}" v2.0.0
    elif [[ "${field}" == "evidence" ]]; then
        expect_failure "Evidence index" run_validator \
            "${em_dash_root}" v2.0.0
    else
        expect_failure "Status+Evidence table" run_validator \
            "${em_dash_root}" v2.0.0
    fi
done

# Quoted `>` characters must not truncate HTML-ish wrappers. Exercise both
# quote styles in every context; the autolink and ordinary dash-containing
# controls prove that valid visible evidence remains accepted.
valid_markdown_controls=(
    '<span title="x > y">captured</span>'
    "<span title='x > y'>captured</span>"
    '<https://example.test/evidence>'
    'bench--capture'
    '`captured`'
)
for control_index in "${!valid_markdown_controls[@]}"; do
    control="${valid_markdown_controls[control_index]}"
    for field in signoff evidence row table; do
        control_root="${root_temp}/control-${control_index}-${field}"
        make_fixture "${control_root}" "2.0.0" \
            "$(make_valid_control_checklist "${field}" "${control}")"
        run_validator "${control_root}" v2.0.0
    done
done

# A single-quoted wrapper containing a visible placeholder must still fail;
# the positive controls above ensure the same parser accepts real content.
for field in signoff evidence row table; do
    single_quoted_pending_root="${root_temp}/single-quoted-pending-${field}"
    make_fixture "${single_quoted_pending_root}" "2.0.0" \
        "$(make_corpus_checklist "${field}" "<span title='x > y'>Pending</span>")"
    if [[ "${field}" == "table" ]]; then
        expect_failure "incomplete rows" run_validator \
            "${single_quoted_pending_root}" v2.0.0
    elif [[ "${field}" == "signoff" ]]; then
        expect_failure "Final sign-off" run_validator \
            "${single_quoted_pending_root}" v2.0.0
    elif [[ "${field}" == "evidence" ]]; then
        expect_failure "Evidence index" run_validator \
            "${single_quoted_pending_root}" v2.0.0
    else
        expect_failure "Status+Evidence table" run_validator \
            "${single_quoted_pending_root}" v2.0.0
    fi
done

# Mirror a real completed profile/transport table: serial rows have no
# numeric default port and record that fact with an em dash, while every
# status and evidence value is complete.
complete_em_dash_root="${root_temp}/complete-em-dash"
make_fixture "${complete_em_dash_root}" "2.0.0" \
    "$(make_checklist complete-em-dash)"
run_validator "${complete_em_dash_root}" v2.0.0

# Tag shapes that must never reach the version comparison at all. These fail in
# the shell prologue, so the fixture contents are irrelevant.
for malformed_tag in V2.0.0 'v2.0.0 ' ' v2.0.0' refs/tags/v2.0.0 v02.0.0 v2.0.0- v2.0; do
    expect_failure "release tag must have the form" \
        run_validator "${complete_root}" "${malformed_tag}"
done

echo "release validator regression checks passed"
