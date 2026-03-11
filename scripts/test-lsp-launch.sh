#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

JAR_PATH="${REPO_ROOT}/vendor/apex-jorje-lsp.jar"
JAR_SHA_PATH="${REPO_ROOT}/vendor/apex-jorje-lsp.jar.sha256"
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

verify_sha256() {
  local expected actual
  expected="$(tr -d '[:space:]' < "${JAR_SHA_PATH}")"
  actual="$(shasum -a 256 "${JAR_PATH}" | awk '{print $1}')"
  if [[ "${expected}" != "${actual}" ]]; then
    echo "SHA256 mismatch for ${JAR_PATH}" >&2
    echo "Expected: ${expected}" >&2
    echo "Actual:   ${actual}" >&2
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

  [[ -f "${JAR_PATH}" ]] || { echo "Missing jar: ${JAR_PATH}" >&2; exit 1; }
  [[ -d "${WORKSPACE_PATH}" ]] || { echo "Missing fixture workspace: ${WORKSPACE_PATH}" >&2; exit 1; }
  [[ -f "${SMOKE_PY}" ]] || { echo "Missing smoke test helper: ${SMOKE_PY}" >&2; exit 1; }

  if [[ -f "${JAR_SHA_PATH}" ]]; then
    verify_sha256
  fi

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
