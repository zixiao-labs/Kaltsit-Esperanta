#!/usr/bin/env bash
# Regenerate the Wuling DevOps API client used by `ama10`.
#
# This script:
#   1. Syncs the OpenAPI spec from the Wuling-DevOps repo (sibling checkout, or override
#      via WULING_OPENAPI_PATH) into `crates/ama10/api/wuling-openapi.yaml`.
#   2. Patches the spec version `3.1.0` -> `3.0.3` because the spec only uses 3.0
#      idioms (`nullable: true`) and `progenitor` only consumes OpenAPI 3.0.
#   3. Runs `cargo progenitor` to generate a Rust client crate.
#   4. Copies the generated source into `crates/ama10/src/wuling_api/generated.rs`,
#      with a do-not-edit header.
#
# Re-run whenever the upstream spec changes. The generated file is committed to
# the repo so reviewers can see the API surface diff.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_SPEC="${WULING_OPENAPI_PATH:-$REPO_ROOT/../Wuling-DevOps/api/openapi.yaml}"
VENDORED_SPEC="$REPO_ROOT/crates/ama10/api/wuling-openapi.yaml"
GENERATED_FILE="$REPO_ROOT/crates/ama10/src/wuling_api/generated.rs"
GEN_NAME="wuling-api-client"
GEN_VERSION="0.0.0-stage1"

if [[ ! -f "$SOURCE_SPEC" ]]; then
    echo "ERROR: OpenAPI spec not found at: $SOURCE_SPEC" >&2
    echo "Set WULING_OPENAPI_PATH, or check out Wuling-DevOps as a sibling of this repo." >&2
    exit 1
fi

echo "==> Syncing spec from $SOURCE_SPEC"
mkdir -p "$(dirname "$VENDORED_SPEC")"
cp "$SOURCE_SPEC" "$VENDORED_SPEC"

echo "==> Patching openapi version (3.1.0 -> 3.0.3) for progenitor compatibility"
sed -i.bak 's/openapi: 3.1.0/openapi: 3.0.3/' "$VENDORED_SPEC"
rm -f "$VENDORED_SPEC.bak"

# progenitor refuses to generate when an operation lacks `operationId`. The
# upstream spec doesn't set them yet, so we deterministically inject one of the
# form `{method}_{slug(path)}` (e.g. `get_orgs_by_org_slug_projects`) before
# generating. If/when upstream adds explicit operationIds, this step becomes a
# no-op for those operations.
echo "==> Injecting operationIds where missing"
python3 - "$VENDORED_SPEC" <<'PY'
import re
import sys
from pathlib import Path

import yaml  # PyYAML

METHODS = {"get", "post", "put", "patch", "delete", "head", "options", "trace"}
PATH_PARAM_RE = re.compile(r"^\{(\w+)\}$")


def slugify(path: str) -> str:
    # /api/v1/orgs/{org_slug}/projects -> orgs_by_org_slug_projects
    path = path.lstrip("/")
    for prefix in ("api/v1/", "api/"):
        if path.startswith(prefix):
            path = path[len(prefix):]
            break
    parts: list[str] = []
    for seg in path.split("/"):
        if not seg:
            continue
        m = PATH_PARAM_RE.match(seg)
        if m:
            parts.append(f"by_{m.group(1)}")
        else:
            parts.append(re.sub(r"[^a-zA-Z0-9]+", "_", seg).strip("_"))
    return "_".join(p for p in parts if p) or "root"


spec_path = Path(sys.argv[1])
spec = yaml.safe_load(spec_path.read_text())

# Drop git-smart-HTTP endpoints. They use custom content types
# (application/x-git-upload-pack-request etc.) that progenitor refuses, and
# they're streaming binary protocols that we don't want to generate a JSON
# client for anyway. Use libgit2 / `git` directly for these.
paths = spec.get("paths") or {}
dropped_paths = [p for p in list(paths.keys()) if ".git/" in p]
for p in dropped_paths:
    del paths[p]

# progenitor 0.14's `extract_responses` panics on
#     assert!(response_types.len() <= 1)
# when an operation declares more than one distinct *success* response schema
# (e.g. 201 TokenResponse + 202 PendingAccountResponse on /auth/register).
# Keep only the lowest-numbered 2xx response so the generated client gets a
# typed happy-path; any other 2xx the server returns will surface as
# Error::UnexpectedResponse and must be intercepted by hand-written wrappers
# (see wuling_api.rs). The vendored spec is itself a transform target, so the
# dropped responses are also absent from the file checked into the repo;
# reviewers see the post-transform contract the Rust client targets.
def _success_schema_keys(resp):
    keys = set()
    for body in ((resp or {}).get("content") or {}).values():
        if not isinstance(body, dict):
            continue
        schema = body.get("schema") or {}
        if "$ref" in schema:
            keys.add(schema["$ref"])
        else:
            keys.add(("inline", yaml.safe_dump(schema, sort_keys=True)))
    return keys

dropped_responses = []
for path, ops in paths.items():
    if not isinstance(ops, dict):
        continue
    for method, op in ops.items():
        if method.lower() not in METHODS or not isinstance(op, dict):
            continue
        responses = op.get("responses") or {}
        success_codes = [c for c in responses.keys()
                         if str(c).isdigit() and str(c).startswith("2")]
        if len(success_codes) <= 1:
            continue
        union = set().union(*(_success_schema_keys(responses[c])
                              for c in success_codes))
        if len(union) <= 1:
            continue
        keep = min(success_codes, key=int)
        for c in success_codes:
            if c == keep:
                continue
            del responses[c]
            dropped_responses.append((method.upper(), path, str(c), keep))

injected = 0
patched_bodies = 0
for path, ops in paths.items():
    if not isinstance(ops, dict):
        continue
    for key, op in ops.items():
        if key.lower() not in METHODS or not isinstance(op, dict):
            continue
        if not op.get("operationId"):
            op["operationId"] = f"{key.lower()}_{slugify(path)}"
            injected += 1
        # Safety net: if any future operation lands a content entry without
        # a schema, default to a binary blob so progenitor doesn't choke.
        for container in (op.get("requestBody"),) + tuple(
            (op.get("responses") or {}).values()
        ):
            if not isinstance(container, dict):
                continue
            for body in (container.get("content") or {}).values():
                if not isinstance(body, dict):
                    continue
                if "schema" not in body:
                    body["schema"] = {"type": "string", "format": "binary"}
                    patched_bodies += 1

spec_path.write_text(yaml.safe_dump(spec, sort_keys=False, allow_unicode=True))
print(
    f"    dropped {len(dropped_paths)} git-smart-HTTP paths, "
    f"injected {injected} operationIds, {patched_bodies} binary schemas"
)
if dropped_responses:
    print(
        f"    dropped {len(dropped_responses)} secondary 2xx responses "
        f"(progenitor multi-success-type limitation):"
    )
    for m, p, code, kept in dropped_responses:
        print(f"      - {m} {p}: removed {code}, kept {kept}")
PY

if ! command -v cargo-progenitor >/dev/null 2>&1; then
    echo "==> cargo-progenitor not found; installing..."
    cargo install cargo-progenitor
fi

# progenitor's bundled rustfmt config uses unstable features
# (wrap_comments, normalize_doc_attributes), so the formatting step crashes
# under stable rustfmt. Pick the most recently installed nightly toolchain
# and point RUSTFMT at its rustfmt binary.
NIGHTLY_TC="$(rustup toolchain list 2>/dev/null \
    | awk '{print $1}' \
    | grep -E '^nightly-[0-9]' \
    | sort -r \
    | head -1 \
    || true)"
if [[ -z "$NIGHTLY_TC" ]]; then
    echo "ERROR: no dated nightly toolchain installed." >&2
    echo "Install one with:" >&2
    echo "    rustup toolchain install nightly --component rustfmt" >&2
    exit 1
fi
NIGHTLY_RUSTFMT="$(rustup which rustfmt --toolchain "$NIGHTLY_TC" 2>/dev/null || true)"
if [[ -z "$NIGHTLY_RUSTFMT" || ! -x "$NIGHTLY_RUSTFMT" ]]; then
    echo "ERROR: rustfmt missing from toolchain $NIGHTLY_TC." >&2
    echo "Install with: rustup component add rustfmt --toolchain $NIGHTLY_TC" >&2
    exit 1
fi

OUT_DIR="$(mktemp -d -t wuling-progenitor-XXXXXX)"
trap 'rm -rf "$OUT_DIR"' EXIT

echo "==> Generating client into $OUT_DIR"
echo "    using nightly rustfmt: $NIGHTLY_RUSTFMT"
RUSTFMT="$NIGHTLY_RUSTFMT" cargo progenitor \
    --input "$VENDORED_SPEC" \
    --output "$OUT_DIR" \
    --name "$GEN_NAME" \
    --version "$GEN_VERSION"

if [[ ! -f "$OUT_DIR/src/lib.rs" ]]; then
    echo "ERROR: progenitor did not produce src/lib.rs in $OUT_DIR" >&2
    exit 1
fi

# Nightly rustfmt's `normalize_doc_attributes` pass occasionally fails to
# insert a separating newline when converting #[doc = "..."] attributes into
# `///` line comments, leaving the next item declaration glued onto the same
# line as a doc comment. Because `///` consumes to end-of-line, the function
# signature becomes part of the comment and its body parses as orphan code
# at the impl level, blowing up the build. Detect and split those lines.
echo "==> Post-processing: splitting doc comments fused to following item"
python3 - "$OUT_DIR/src/lib.rs" <<'PY'
import re
import sys

path = sys.argv[1]
src = open(path).read()

ITEM = re.compile(
    r'\s{2,}(pub\s+(?:async\s+)?fn\b'
    r'|pub\s+(?:struct|enum|trait|use|mod|const|static|type)\b'
    r'|impl\b|#\[)'
)

out_lines = []
splits = 0
for line in src.splitlines():
    stripped = line.lstrip()
    if not stripped.startswith('///'):
        out_lines.append(line)
        continue
    m = ITEM.search(line)
    if not m:
        out_lines.append(line)
        continue
    splits += 1
    indent = line[:len(line) - len(stripped)]
    out_lines.append(line[:m.start()].rstrip())
    out_lines.append(indent + line[m.start():].lstrip())

open(path, 'w').write('\n'.join(out_lines) + '\n')
print(f"    split {splits} fused doc/item lines")

# Re-scan to verify no fused lines remain
remaining_fused = []
for line_num, line in enumerate(open(path).readlines(), 1):
    stripped = line.lstrip()
    if stripped.startswith('///') and ITEM.search(line):
        remaining_fused.append((line_num, line.rstrip()))

if remaining_fused:
    print(f"\nERROR: {len(remaining_fused)} fused doc/item lines remain in {path}:", file=sys.stderr)
    for line_num, line in remaining_fused[:5]:  # Show first 5
        print(f"  Line {line_num}: {line[:80]}...", file=sys.stderr)
    sys.exit(1)
PY

mkdir -p "$(dirname "$GENERATED_FILE")"
{
    cat <<'HEADER'
// @generated
// AUTO-GENERATED by script/regen-wuling-api.sh. Do not edit by hand.
// Source: crates/ama10/api/wuling-openapi.yaml
//
// To regenerate after the upstream spec changes:
//     script/regen-wuling-api.sh

#![allow(
    clippy::all,
    dead_code,
    unused_imports,
    unused_qualifications,
    missing_docs,
    rustdoc::broken_intra_doc_links
)]

HEADER
    cat "$OUT_DIR/src/lib.rs"
} > "$GENERATED_FILE"

echo "==> Generated client written to $GENERATED_FILE"

# Surface the runtime dep version that the generated code expects, so the
# operator can sync `progenitor-client` in the workspace Cargo.toml if it drifts.
if [[ -f "$OUT_DIR/Cargo.toml" ]]; then
    echo
    echo "==> Generated crate's progenitor-client dep:"
    grep -E '^progenitor-client' "$OUT_DIR/Cargo.toml" || true
fi
