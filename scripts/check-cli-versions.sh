#!/usr/bin/env bash
# Check pinned CLI versions in cli_download.rs against upstream latest releases.
#
# Usage:
#   ./scripts/check-cli-versions.sh          # human-readable report
#   ./scripts/check-cli-versions.sh --json   # JSON output for automation
#
# Exit codes:
#   0 — all versions are up to date
#   1 — updates available
#   2 — error (network, missing tools, etc.)
#
# Requires: curl, jq
# Optional: GITHUB_TOKEN env var to avoid rate limiting

set -euo pipefail

CLI_DOWNLOAD_RS="rustconn-core/src/cli_download.rs"
JSON_MODE=false
UPDATES_FOUND=0

if [[ "${1:-}" == "--json" ]]; then
    JSON_MODE=true
fi

# ── Dependency check ──────────────────────────────────────────────
for cmd in curl jq; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: $cmd is required but not installed" >&2
        exit 2
    fi
done

# ── GitHub API helper ─────────────────────────────────────────────
gh_api() {
    local url="$1"
    local -a headers=(-H "Accept: application/vnd.github+json")
    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        headers+=(-H "Authorization: Bearer $GITHUB_TOKEN")
    fi
    curl -sL "${headers[@]}" "$url"
}

# ── Extract current pinned version from cli_download.rs ───────────
get_pinned_version() {
    local component_id="$1"
    awk -v id="$component_id" '
        /id: "/ { current_id = $0; gsub(/.*id: "/, "", current_id); gsub(/".*/, "", current_id) }
        /pinned_version: Some\(/ && current_id == id {
            ver = $0
            gsub(/.*pinned_version: Some\("/, "", ver)
            gsub(/".*/, "", ver)
            print ver
            exit
        }
    ' "$CLI_DOWNLOAD_RS"
}

# ── Version check functions ───────────────────────────────────────

check_kubectl() {
    curl -sL "https://dl.k8s.io/release/stable.txt" 2>/dev/null | sed 's/^v//'
}

check_tailscale() {
    # Use pkgs.tailscale.com/stable — matches download URL source
    curl -sL "https://pkgs.tailscale.com/stable/" 2>/dev/null \
        | grep -oP 'tailscale_\K[0-9]+\.[0-9]+\.[0-9]+' \
        | sort -V | tail -1
}

check_teleport() {
    gh_api "https://api.github.com/repos/gravitational/teleport/releases/latest" \
        | jq -r '.tag_name' | sed 's/^v//'
}

check_boundary() {
    curl -sL "https://checkpoint-api.hashicorp.com/v1/check/boundary" 2>/dev/null \
        | jq -r '.current_version'
}

check_bitwarden() {
    gh_api "https://api.github.com/repos/bitwarden/clients/releases?per_page=30" \
        | jq -r '[.[] | select(.tag_name | startswith("cli-v"))][0].tag_name' \
        | sed 's/^cli-v//'
}

check_1password() {
    curl -sL "https://app-updates.agilebits.com/check/1/0/CLI2/en/2.0.0/N" 2>/dev/null \
        | jq -r '.version'
}

check_tigervnc() {
    gh_api "https://api.github.com/repos/TigerVNC/tigervnc/releases/latest" \
        | jq -r '.tag_name' | sed 's/^v//'
}

# ── Component definitions ─────────────────────────────────────────
# Format: id|display_name|check_function
COMPONENTS=(
    "vncviewer|TigerVNC|check_tigervnc"
    "tsh|Teleport|check_teleport"
    "tailscale|Tailscale|check_tailscale"
    "boundary|Boundary|check_boundary"
    "bw|Bitwarden CLI|check_bitwarden"
    "op|1Password CLI|check_1password"
    "kubectl|kubectl|check_kubectl"
)

# ── Main ──────────────────────────────────────────────────────────
results=()

for entry in "${COMPONENTS[@]}"; do
    IFS='|' read -r id name func <<< "$entry"

    pinned=$(get_pinned_version "$id")
    if [[ -z "$pinned" ]]; then
        pinned="(not pinned)"
    fi

    latest=$($func 2>/dev/null || echo "ERROR")

    if [[ "$latest" == "ERROR" || "$latest" == "null" || -z "$latest" ]]; then
        status="error"
        symbol="❌"
    elif [[ "$pinned" == "$latest" ]]; then
        status="current"
        symbol="✅"
    else
        status="update"
        symbol="⬆️"
        UPDATES_FOUND=1
    fi

    results+=("$id|$name|$pinned|$latest|$status")

    if [[ "$JSON_MODE" == false ]]; then
        printf "%-16s %-10s → %-10s %s %s\n" "$name" "$pinned" "$latest" "$symbol" \
            "$( [[ "$status" == "update" ]] && echo "UPDATE AVAILABLE" || true )"
    fi
done

# ── JSON output ───────────────────────────────────────────────────
if [[ "$JSON_MODE" == true ]]; then
    echo "{"
    echo '  "updates_available": '"$UPDATES_FOUND"','
    echo '  "components": ['
    first=true
    for r in "${results[@]}"; do
        IFS='|' read -r id name pinned latest status <<< "$r"
        if [[ "$first" == true ]]; then
            first=false
        else
            echo ","
        fi
        printf '    {"id": "%s", "name": "%s", "pinned": "%s", "latest": "%s", "status": "%s"}' \
            "$id" "$name" "$pinned" "$latest" "$status"
    done
    echo ""
    echo "  ]"
    echo "}"
else
    echo ""
    if [[ "$UPDATES_FOUND" -eq 1 ]]; then
        echo "⚠  Updates available. Review and update cli_download.rs."
    else
        echo "✅ All CLI versions are up to date."
    fi
fi

exit "$UPDATES_FOUND"
