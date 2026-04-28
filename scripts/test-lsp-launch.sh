#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Keep URL in sync with APEX_LSP_JAR_DOWNLOAD_URL in src/lib.rs.
JAR_URL="https://raw.githubusercontent.com/forcedotcom/salesforcedx-vscode/67dc27932e0ce43b93abe00878a2f966d0eb16a3/packages/salesforcedx-vscode-apex/jars/apex-jorje-lsp.jar"
JAR_SHA256="4b0d014f7a91d16b437868b2076a8e93ab29821dfe938c10e0e4cd9a4b2fc01d"
JAR_CACHE_DIR="${REPO_ROOT}/.cache/apex-lsp"
JAR_PATH="${JAR_CACHE_DIR}/apex-jorje-lsp.jar"
WORKSPACE_PATH="${REPO_ROOT}/scripts/fixtures/sfdx-minimal"
PROBE_FILE="force-app/main/default/triggers/SmokeTest.trigger"
COMPLETION_PREFIX="System."
SMOKE_PY="${REPO_ROOT}/scripts/lsp_smoke.py"

resolve_java() {
  if [[ -n "${ZED_SF_JAVA_BIN:-}" ]]; then
    echo "${ZED_SF_JAVA_BIN}"
    return
  fi
  if [[ -n "${JDK_HOME:-}" ]]; then
    echo "${JDK_HOME}/bin/java"
    return
  fi
  if [[ -n "${JAVA_HOME:-}" ]]; then
    echo "${JAVA_HOME}/bin/java"
    return
  fi
  command -v java
}

java_major() {
  local version_output major
  version_output="$("$1" -version 2>&1 | head -n 1)"
  major="$(echo "${version_output}" | sed -E 's/.*version "([0-9]+).*/\1/')"
  if [[ -z "${major}" || "${major}" == "${version_output}" ]]; then
    echo "Could not parse Java major version from: ${version_output}" >&2
    exit 1
  fi
  echo "${major}"
}

jar_sha256() {
  shasum -a 256 "$1" | awk '{print $1}'
}

verify_sha256() {
  local actual
  actual="$(jar_sha256 "${JAR_PATH}")"
  if [[ "${JAR_SHA256}" != "${actual}" ]]; then
    echo "SHA256 mismatch for ${JAR_PATH}" >&2
    echo "Expected: ${JAR_SHA256}" >&2
    echo "Actual:   ${actual}" >&2
    exit 1
  fi
}

ensure_jar() {
  if [[ -f "${JAR_PATH}" ]] && [[ "$(jar_sha256 "${JAR_PATH}")" == "${JAR_SHA256}" ]]; then
    return
  fi

  mkdir -p "${JAR_CACHE_DIR}"
  echo "Downloading Apex LSP jar from ${JAR_URL}"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 -o "${JAR_PATH}" "${JAR_URL}"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "${JAR_PATH}" "${JAR_URL}"
  else
    echo "Neither curl nor wget is available to download the Apex LSP jar." >&2
    exit 1
  fi
}

main() {
  local java_cmd major nested_root nested_workspace
  java_cmd="$(resolve_java)"
  if [[ ! -x "${java_cmd}" ]]; then
    echo "Java binary is not executable: ${java_cmd}" >&2
    exit 1
  fi

  major="$(java_major "${java_cmd}")"
  if (( major < 11 )); then
    echo "Java ${major} is unsupported. Apex LSP requires Java 11+." >&2
    exit 1
  fi

  [[ -d "${WORKSPACE_PATH}" ]] || { echo "Missing fixture workspace: ${WORKSPACE_PATH}" >&2; exit 1; }
  [[ -f "${SMOKE_PY}" ]] || { echo "Missing smoke test helper: ${SMOKE_PY}" >&2; exit 1; }

  ensure_jar
  verify_sha256

  echo "Using Java: ${java_cmd}"
  echo "Java major: ${major}"
  echo "Running Apex LSP smoke test against fixture workspace..."

  python3 "${SMOKE_PY}" \
    --java "${java_cmd}" \
    --jar "${JAR_PATH}" \
    --workspace "${WORKSPACE_PATH}" \
    --probe-file "${PROBE_FILE}" \
    --completion-prefix "${COMPLETION_PREFIX}" \
    --timeout-seconds 20

  nested_root="$(mktemp -d)"
  trap 'rm -rf "${nested_root:-}"' EXIT
  nested_workspace="${nested_root}/packages/sfdx-minimal"
  mkdir -p "${nested_root}/packages"
  cp -R "${WORKSPACE_PATH}" "${nested_workspace}"

  echo "Running Apex LSP smoke test against nested SFDX workspace..."

  python3 "${SMOKE_PY}" \
    --java "${java_cmd}" \
    --jar "${JAR_PATH}" \
    --workspace "${nested_root}" \
    --probe-file "packages/sfdx-minimal/${PROBE_FILE}" \
    --completion-prefix "${COMPLETION_PREFIX}" \
    --timeout-seconds 20
}

main "$@"
