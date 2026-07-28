#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE_SPEC="${WULING_OPENAPI_PATH:-$REPO_ROOT/../Wuling-DevOps/api/openapi.yaml}"
VENDORED_SPEC="$REPO_ROOT/crates/ama10/api/wuling-openapi.yaml"
TYPE_SCHEMA="$REPO_ROOT/crates/ama10/api/wuling-client-types.json"

if [[ ! -f "$SOURCE_SPEC" ]]; then
    echo "Wuling OpenAPI source not found: $SOURCE_SPEC" >&2
    exit 1
fi

cp "$SOURCE_SPEC" "$VENDORED_SPEC"

python3 - "$VENDORED_SPEC" "$TYPE_SCHEMA" <<'PY'
import json
import sys

import yaml

source_path, output_path = sys.argv[1:]
with open(source_path, encoding="utf-8") as source_file:
    openapi = yaml.safe_load(source_file)

selected = {
    "DeviceCodeResponse": {
        "properties": [
            "device_code",
            "expires_in",
            "interval",
            "user_code",
            "verification_uri",
            "verification_uri_complete",
        ],
        "required": [
            "device_code",
            "expires_in",
            "interval",
            "user_code",
            "verification_uri",
            "verification_uri_complete",
        ],
    },
    "OAuthError": {
        "properties": ["error", "error_description"],
        "required": ["error"],
    },
    "OAuthTokenResponse": {
        "properties": [
            "access_token",
            "expires_in",
            "refresh_token",
            "scope",
            "token_type",
        ],
        "required": [
            "access_token",
            "expires_in",
            "scope",
            "token_type",
        ],
    },
    "User": {
        "properties": ["avatar_url", "display_name", "username"],
        "required": ["avatar_url", "display_name", "username"],
    },
    "WellKnownDoc": {
        "properties": [
            "authorization_endpoint",
            "desktop_official_client_id",
            "device_authorization_endpoint",
            "frontend_device_verification_uri",
            "issuer",
            "revocation_endpoint",
            "scopes_supported",
            "token_endpoint",
        ],
        "required": [
            "authorization_endpoint",
            "desktop_official_client_id",
            "device_authorization_endpoint",
            "frontend_device_verification_uri",
            "issuer",
            "revocation_endpoint",
            "scopes_supported",
            "token_endpoint",
        ],
    },
}


def project_schema(value):
    if isinstance(value, list):
        return [project_schema(item) for item in value]
    if not isinstance(value, dict):
        return value
    allowed = {
        "$ref",
        "enum",
        "items",
        "maximum",
        "maxItems",
        "maxLength",
        "minimum",
        "minItems",
        "minLength",
        "properties",
        "type",
    }
    projected = {}
    for key, item in value.items():
        if key == "properties":
            projected[key] = {
                property_name: project_schema(property_schema)
                for property_name, property_schema in item.items()
            }
        elif key in allowed:
            projected[key] = project_schema(item)
    return projected


schemas = openapi["components"]["schemas"]
definitions = {}
for name, selection in selected.items():
    if name not in schemas:
        raise SystemExit(f"required Wuling schema is missing: {name}")
    definition = project_schema(schemas[name])
    definition["properties"] = {
        property_name: definition["properties"][property_name]
        for property_name in selection["properties"]
    }
    definition["required"] = selection["required"]
    definitions[name] = definition

properties = {
    "device_code": {"$ref": "#/$defs/DeviceCodeResponse"},
    "oauth_error": {"$ref": "#/$defs/OAuthError"},
    "oauth_token": {"$ref": "#/$defs/OAuthTokenResponse"},
    "user": {"$ref": "#/$defs/User"},
    "well_known": {"$ref": "#/$defs/WellKnownDoc"},
}
output = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "WulingApiTypes",
    "type": "object",
    "required": list(properties),
    "properties": properties,
    "$defs": definitions,
}

with open(output_path, "w", encoding="utf-8") as output_file:
    json.dump(output, output_file, ensure_ascii=False, indent=2)
    output_file.write("\n")
PY

echo "Updated $VENDORED_SPEC"
echo "Updated $TYPE_SCHEMA"
