#!/usr/bin/env bash
# Check CLI version resolution endpoints are reachable and return valid data.
#
# Since most CLI components now auto-resolve their latest version at install
# time, this script verifies that the upstream APIs/endpoints are accessible
# and returning expected data formats.
#
# TigerVNC is the only component with a static pinned version — it's checked
# against the GitHub releases API.
#
# Usage:
#   ./scripts/check-cli-versions.sh          # human-readable report
#   ./scripts/check-cli-versions.sh --json   # JSON output for automation
#
# Exit codes:
#   0 — all endpoints reachable, TigerVNC up to date
#   1 — updates available (TigerVNC) or endpoints unreachable
#   2 — error (missing tools, etc.)
#
# Requires: curl
# Optional: GITHUB_TOKEN env var to avoid rate limiting
#
# NOTE: This script avoids grep -P (PCRE) for macOS compatibility.
# All parsing uses sed/awk which work on both BSD (macOS) and GNU (Linux).

set -euo pipefail

CLI_DOWNLOAD_RS="rustconn-core/src/cli_download/components.rs"
JSON_MODE=false
ISSUES_FOUND=0

if [[ "${1:-}" == "--json" ]]; then
    JSON_MODE=true
fi

# ── Dependency check ──────────────────────────────────────────────
if ! command -v curl &>/dev/null; then
    echo "ERROR: curl is required but not installed" >&2
    exit 2
fi

# ── GitHub API helper ─────────────────────────────────────────────
gh_api() {
    local url="$1"
    local -a headers=(-H "Accept: application/vnd.github+json")
    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        headers+=(-H "Authorization: Bearer $GITHUB_TOKEN")
    fi
    curl -sL --max-time 10 "${headers[@]}" "$url"
}

# ── Endpoint check functions ──────────────────────────────────────

check_kubectl_endpoint() {
    local ver
    ver=$(curl -sL --max-time 10 "https://dl.k8s.io/release/stable.txt" 2>/dev/null)
    if [[ "$ver" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "${ver#v}"
    else
        echo "ERROR"
    fi
}

check_tailscale_endpoint() {
    local ver
    ver=$(curl -sL --max-time 10 "https://pkgs.tailscale.com/stable/" 2>/dev/null \
        | sed -n 's/.*tailscale_\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)_amd64\.tgz.*/\1/p' \
        | head -1)
    if [[ -n "$ver" ]]; then
        echo "$ver"
    else
        echo "ERROR"
    fi
}

check_teleport_endpoint() {
    local tag
    tag=$(gh_api "https://api.github.com/repos/gravitational/teleport/releases/latest" \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\([^"]*\)".*/\1/p' | head -1)
    if [[ -n "$tag" ]]; then
        echo "$tag"
    else
        echo "ERROR"
    fi
}

check_boundary_endpoint() {
    local ver
    ver=$(curl -sL --max-time 10 "https://checkpoint-api.hashicorp.com/v1/check/boundary" 2>/dev/null \
        | sed -n 's/.*"current_version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    if [[ -n "$ver" ]]; then
        echo "$ver"
    else
        echo "ERROR"
    fi
}

check_hoop_endpoint() {
    local ver
    ver=$(curl -sL --max-time 10 "https://releases.hoop.dev/release/latest.txt" 2>/dev/null | tr -d '[:space:]')
    if [[ "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "$ver"
    else
        echo "ERROR"
    fi
}

check_bitwarden_endpoint() {
    local tag
    tag=$(gh_api "https://api.github.com/repos/bitwarden/clients/releases?per_page=30" \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"cli-v\([^"]*\)".*/\1/p' | head -1)
    if [[ -n "$tag" ]]; then
        echo "$tag"
    else
        echo "ERROR"
    fi
}

check_1password_endpoint() {
    local ver
    ver=$(curl -sL --max-time 10 "https://app-updates.agilebits.com/check/1/0/CLI2/en/2.0.0/N" 2>/dev/null \
        | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    if [[ -n "$ver" ]]; then
        echo "$ver"
    else
        echo "ERROR"
    fi
}

check_tigervnc_pinned() {
    # TigerVNC is the only pinned component — check if update available
    local pinned latest
    pinned=$(awk '/id: "vncviewer"/{found=1} found && /pinned_version: Some/{gsub(/.*Some\("/,""); gsub(/".*/,""); print; exit}' "$CLI_DOWNLOAD_RS")
    latest=$(gh_api "https://api.github.com/repos/TigerVNC/tigervnc/releases/latest" \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\{0,1\}\([^"]*\)".*/\1/p' | head -1)
    echo "${pinned}|${latest}"
}

# ── Component definitions ─────────────────────────────────────────
# Format: id|display_name|check_function|type (auto|pinned)
COMPONENTS=(
    "kubectl|kubectl|check_kubectl_endpoint|auto"
    "tailscale|Tailscale|check_tailscale_endpoint|auto"
    "tsh|Teleport|check_teleport_endpoint|auto"
    "boundary|Boundary|check_boundary_endpoint|auto"
    "hoop|Hoop.dev|check_hoop_endpoint|auto"
    "bw|Bitwarden CLI|check_bitwarden_endpoint|auto"
    "op|1Password CLI|check_1password_endpoint|auto"
    "vncviewer|TigerVNC|check_tigervnc_pinned|pinned"
)

# ── Main ──────────────────────────────────────────────────────────
results=()

for entry in "${COMPONENTS[@]}"; do
    IFS='|' read -r id name func type <<< "$entry"

    if [[ "$type" == "pinned" ]]; then
        result=$($func 2>/dev/null || echo "ERROR|ERROR")
        IFS='|' read -r pinned latest <<< "$result"
        if [[ "$latest" == "ERROR" || -z "$latest" ]]; then
            status="error"
            symbol="❌"
            ISSUES_FOUND=1
        elif [[ "$pinned" == "$latest" ]]; then
            status="current"
            symbol="✅"
        else
            status="update"
            symbol="⬆️"
            ISSUES_FOUND=1
        fi
        results+=("$id|$name|$pinned|$latest|$status")
        if [[ "$JSON_MODE" == false ]]; then
            printf "%-16s %-10s → %-10s %s %s\n" "$name" "$pinned" "$latest" "$symbol" \
                "$( [[ "$status" == "update" ]] && echo "UPDATE AVAILABLE" || true )"
        fi
    else
        latest=$($func 2>/dev/null || echo "ERROR")
        if [[ "$latest" == "ERROR" || -z "$latest" ]]; then
            status="error"
            symbol="❌"
            ISSUES_FOUND=1
        else
            status="ok"
            symbol="✅"
        fi
        results+=("$id|$name|auto|$latest|$status")
        if [[ "$JSON_MODE" == false ]]; then
            printf "%-16s %-10s   %-10s %s %s\n" "$name" "(auto)" "$latest" "$symbol" \
                "$( [[ "$status" == "error" ]] && echo "ENDPOINT UNREACHABLE" || true )"
        fi
    fi
done

# ── JSON output ───────────────────────────────────────────────────
if [[ "$JSON_MODE" == true ]]; then
    echo "{"
    echo '  "issues_found": '"$ISSUES_FOUND"','
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
    if [[ "$ISSUES_FOUND" -eq 1 ]]; then
        echo "⚠  Issues found. Check endpoints or update TigerVNC."
    else
        echo "✅ All CLI version endpoints reachable. Auto-resolve working."
    fi
fi

exit "$ISSUES_FOUND"
