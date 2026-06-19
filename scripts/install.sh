#!/usr/bin/env bash
set -euo pipefail

REPO_SLUG="${TIME_KEEP_REPO:-joe-broadhead/time-keep}"
RELEASE_BASE_URL="${TIME_KEEP_RELEASE_BASE_URL:-https://github.com/${REPO_SLUG}/releases}"
VERSION="${TIME_KEEP_VERSION:-latest}"
INSTALL_DIR="${TIME_KEEP_INSTALL_DIR:-$HOME/.local/bin}"
INSTALL_SKILLS="${TIME_KEEP_INSTALL_SKILLS:-0}"
SKILLS_DIR="${TIME_KEEP_SKILLS_DIR:-$HOME/.agents/skills}"
SKILL_NAME="${TIME_KEEP_SKILL_NAME:-}"
INSTALL_COMPLETIONS="${TIME_KEEP_INSTALL_COMPLETIONS:-0}"
COMPLETION_SHELL="${TIME_KEEP_COMPLETION_SHELL:-}"
COMPLETIONS_DIR="${TIME_KEEP_COMPLETIONS_DIR:-}"
NON_INTERACTIVE="${TIME_KEEP_INSTALL_NONINTERACTIVE:-0}"
VERIFY_CHECKSUM="${TIME_KEEP_VERIFY_CHECKSUM:-1}"
DRY_RUN="${TIME_KEEP_INSTALL_DRY_RUN:-0}"
DOWNLOAD_TOKEN="${TIME_KEEP_GITHUB_TOKEN:-${GITHUB_TOKEN:-${GH_TOKEN:-}}}"

usage() {
  cat <<'EOF'
Usage: install.sh [--install-dir <path>] [--install-skills [--skill <name>]]
                  [--skills-dir <path>] [--install-completions --shell <shell>]
                  [--completions-dir <path>] [--non-interactive|-y] [--dry-run]

Downloads and installs time-keep from GitHub releases.

Defaults:
  - install dir: $HOME/.local/bin
  - skills dir: $HOME/.agents/skills
  - version: latest release
  - checksum verification: enabled
  - completions: opt-in

Environment overrides:
  TIME_KEEP_REPO                    GitHub repo slug (default: joe-broadhead/time-keep)
  TIME_KEEP_RELEASE_BASE_URL        Release base URL for tests/mirrors
  TIME_KEEP_GITHUB_TOKEN            Optional token for private repos or rate limits
  TIME_KEEP_VERSION                 Release tag, such as v0.0.0 (default: latest)
  TIME_KEEP_INSTALL_DIR             Install directory for time-keep
  TIME_KEEP_INSTALL_SKILLS          1 to install Agent Skills (default: 0)
  TIME_KEEP_SKILLS_DIR              Skills destination (default: $HOME/.agents/skills)
  TIME_KEEP_SKILL_NAME              Optional single skill to install
  TIME_KEEP_INSTALL_COMPLETIONS     1 to install shell completions (default: 0)
  TIME_KEEP_COMPLETION_SHELL        Completion shell: bash, zsh, fish, powershell, or elvish
  TIME_KEEP_COMPLETIONS_DIR         Completion destination directory
  TIME_KEEP_INSTALL_NONINTERACTIVE  1 to skip prompts
  TIME_KEEP_VERIFY_CHECKSUM         1 to verify artifact checksum (default: 1)
  TIME_KEEP_INSTALL_DRY_RUN         1 to print resolved install plan without downloading
EOF
}

validate_path_segment() {
  local value="$1"
  local label="$2"
  if [[ -z "${value}" ]]; then
    echo "${label} cannot be empty." >&2
    return 1
  fi
  if ! [[ "${value}" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
    echo "Invalid ${label} '${value}'. Use a single safe path segment." >&2
    return 1
  fi
}

resolve_skill_install_selection() {
  if [[ -n "${SKILL_NAME}" ]]; then
    validate_path_segment "${SKILL_NAME}" "skill name"
  fi
  if [[ -n "${SKILL_NAME}" && "${INSTALL_SKILLS}" != "1" ]]; then
    echo "--skill requires --install-skills." >&2
    return 1
  fi
}

validate_completion_selection() {
  if [[ "${INSTALL_COMPLETIONS}" != "1" ]]; then
    return 0
  fi
  if [[ -z "${COMPLETION_SHELL}" ]]; then
    echo "--install-completions requires --shell <bash|zsh|fish|powershell|elvish>." >&2
    return 1
  fi
  case "${COMPLETION_SHELL}" in
    bash|zsh|fish|powershell|elvish) ;;
    *)
      echo "Unsupported completion shell: ${COMPLETION_SHELL}" >&2
      return 1
      ;;
  esac
}

default_completions_dir() {
  case "${COMPLETION_SHELL}" in
    bash) printf '%s\n' "${HOME}/.local/share/bash-completion/completions" ;;
    zsh) printf '%s\n' "${HOME}/.zsh/completions" ;;
    fish) printf '%s\n' "${HOME}/.config/fish/completions" ;;
    powershell) printf '%s\n' "${HOME}/.local/share/powershell/Completions" ;;
    elvish) printf '%s\n' "${HOME}/.config/elvish/lib" ;;
    *) return 1 ;;
  esac
}

completion_file_name() {
  case "${COMPLETION_SHELL}" in
    bash) printf '%s\n' "time-keep" ;;
    zsh) printf '%s\n' "_time-keep" ;;
    fish) printf '%s\n' "time-keep.fish" ;;
    powershell) printf '%s\n' "time-keep.ps1" ;;
    elvish) printf '%s\n' "time-keep.elv" ;;
    *) return 1 ;;
  esac
}

compute_sha256() {
  local file="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    echo "Could not verify artifact hash: no sha256sum or shasum available." >&2
    return 1
  fi
}

expected_checksum() {
  local checksum_file="$1"
  local filename="$2"
  while read -r hash path _; do
    [[ -z "${hash}" || -z "${path}" ]] && continue
    path="${path#\*}"
    local candidate="${path##*/}"
    candidate="${candidate##*\\}"
    if [[ "${candidate}" == "${filename}" ]]; then
      printf '%s\n' "${hash}"
      return 0
    fi
  done < "${checksum_file}"
}

verify_checksum_file() {
  local artifact="$1"
  local checksum_file="$2"
  local expected
  local actual

  expected="$(expected_checksum "$checksum_file" "$(basename "$artifact")")"
  if [[ -z "$expected" ]]; then
    echo "Checksum file is missing entry for $(basename "$artifact")." >&2
    return 1
  fi

  actual="$(compute_sha256 "$artifact" | tr '[:upper:]' '[:lower:]')"
  expected="$(echo "$expected" | tr '[:upper:]' '[:lower:]')"

  if [[ "$actual" != "$expected" ]]; then
    echo "Checksum mismatch for $(basename "$artifact")." >&2
    echo "  Expected: $expected" >&2
    echo "  Actual:   $actual" >&2
    return 1
  fi
}

download_file() {
  local file_name="$1"
  local url="$2"
  local out="$3"

  if [[ -n "${DOWNLOAD_TOKEN}" ]]; then
    if curl -fsSL -H "Authorization: Bearer ${DOWNLOAD_TOKEN}" "${url}" -o "${out}"; then
      return 0
    fi
    echo "Authenticated download failed for ${file_name}; retrying without token." >&2
  fi

  if curl -fsSL "${url}" -o "${out}"; then
    return 0
  fi

  if command -v gh >/dev/null 2>&1; then
    echo "Direct download failed for ${file_name}; trying gh release download"
    if [[ "${VERSION}" == "latest" ]]; then
      gh release download --repo "${REPO_SLUG}" --pattern "${file_name}" --output "${out}"
    else
      gh release download "${VERSION}" --repo "${REPO_SLUG}" --pattern "${file_name}" --output "${out}"
    fi
    return 0
  fi

  echo "Download failed for ${file_name} and gh CLI is not available for fallback." >&2
  return 1
}

download_repo_archive() {
  local ref="$1"
  local out="$2"
  local archive_url="https://api.github.com/repos/${REPO_SLUG}/tarball/${ref}"

  if [[ -n "${DOWNLOAD_TOKEN}" ]]; then
    if curl -fsSL \
      -H "Authorization: Bearer ${DOWNLOAD_TOKEN}" \
      -H "Accept: application/vnd.github+json" \
      "${archive_url}" \
      -o "${out}"; then
      return 0
    fi
    echo "Authenticated archive download failed for ref '${ref}'; retrying without token." >&2
  fi

  curl -fsSL \
    -H "Accept: application/vnd.github+json" \
    "${archive_url}" \
    -o "${out}"
}

latest_release_tag() {
  local api_url="https://api.github.com/repos/${REPO_SLUG}/releases/latest"
  local response=""

  if [[ -n "${DOWNLOAD_TOKEN}" ]]; then
    response="$(curl -fsSL -H "Authorization: Bearer ${DOWNLOAD_TOKEN}" "${api_url}" 2>/dev/null || true)"
  fi
  if [[ -z "${response}" ]]; then
    response="$(curl -fsSL "${api_url}" 2>/dev/null || true)"
  fi
  if [[ -z "${response}" ]]; then
    return 0
  fi

  printf '%s' "${response}" \
    | tr -d '\n' \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'
}

installed_binary_release_tag() {
  local binary="${INSTALL_DIR}/time-keep${ext:-}"
  local version_output=""
  local version=""

  if [[ ! -x "${binary}" ]]; then
    return 0
  fi

  version_output="$("${binary}" --version 2>/dev/null || true)"
  version="$(printf '%s\n' "${version_output}" | awk 'NR == 1 {print $2}')"
  if [[ -z "${version}" ]]; then
    return 0
  fi

  case "${version}" in
    v*) printf '%s\n' "${version}" ;;
    *) printf 'v%s\n' "${version}" ;;
  esac
}

list_standalone_skills() {
  local skills_source="$1"
  find "${skills_source}" -mindepth 1 -maxdepth 1 -type d \
    -exec test -f "{}/SKILL.md" ';' -print | sed 's#.*/##' | sort
}

install_standalone_skill_from_source() {
  local skills_source="$1"
  local skills_dest="$2"
  local skill_name="$3"
  local skill_source="${skills_source}/${skill_name}"
  local installed_dir="${skills_dest}/${skill_name}"

  validate_path_segment "${skill_name}" "skill name" || return 1
  if [[ ! -f "${skill_source}/SKILL.md" ]]; then
    echo "Skill '${skill_name}' not found in repository archive." >&2
    return 1
  fi

  mkdir -p "${skills_dest}" || return 1
  rm -rf "${installed_dir}" || return 1
  cp -R "${skill_source}" "${installed_dir}" || return 1
  echo "Installed skill '${skill_name}' to ${skills_dest}"
}

install_all_standalone_skills_from_source() {
  local skills_source="$1"
  local skills_dest="$2"
  local skill_count=0
  local skill_name=""

  while IFS= read -r skill_name; do
    [[ -n "${skill_name}" ]] || continue
    install_standalone_skill_from_source "${skills_source}" "${skills_dest}" "${skill_name}" || return 1
    skill_count=$((skill_count + 1))
  done < <(list_standalone_skills "${skills_source}")

  if (( skill_count < 1 )); then
    echo "No standalone skills found in repository archive." >&2
    return 1
  fi

  echo "Installed ${skill_count} skill(s) to ${skills_dest}"
}

install_skills_from_ref() {
  local ref="$1"
  local skills_dest="$2"
  local requested_skill="$3"
  local archive_ref="${ref//\//-}"
  local archive_path="${tmp_dir}/repo-${archive_ref}.tar.gz"
  local extract_dir="${tmp_dir}/repo-${archive_ref}"
  local skills_source=""

  download_repo_archive "${ref}" "${archive_path}" || return 1
  mkdir -p "${extract_dir}" || return 1
  tar -xzf "${archive_path}" -C "${extract_dir}" || return 1
  skills_source="$(find "${extract_dir}" -type d -path "*/.github/skills" | head -n 1)"
  if [[ -z "${skills_source}" ]]; then
    echo "Skills directory not found in repository archive for ref '${ref}'." >&2
    return 1
  fi

  if [[ -n "${requested_skill}" ]]; then
    install_standalone_skill_from_source "${skills_source}" "${skills_dest}" "${requested_skill}" || return 1
  else
    install_all_standalone_skills_from_source "${skills_source}" "${skills_dest}" || return 1
  fi
}

resolve_skills_ref() {
  local detected_latest_tag=""
  local installed_tag=""

  if [[ "${VERSION}" != "latest" ]]; then
    printf '%s\n' "${VERSION}"
    return 0
  fi

  detected_latest_tag="$(latest_release_tag)"
  if [[ -z "${detected_latest_tag}" ]]; then
    installed_tag="$(installed_binary_release_tag)"
    if [[ -n "${installed_tag}" ]]; then
      echo "Could not resolve latest release tag; falling back to installed binary version ${installed_tag} for skills." >&2
      printf '%s\n' "${installed_tag}"
      return 0
    fi
    echo "Could not resolve latest release tag or infer the installed binary version for skills installation." >&2
    echo "Set TIME_KEEP_VERSION to an explicit release tag, or retry after GitHub is reachable." >&2
    return 1
  fi

  printf '%s\n' "${detected_latest_tag}"
}

install_selected_skills() {
  local requested_skill="$1"
  local skills_ref=""

  skills_ref="$(resolve_skills_ref)"
  if [[ -n "${requested_skill}" ]]; then
    echo "Installing skill '${requested_skill}' from ref '${skills_ref}' into ${SKILLS_DIR}"
  else
    echo "Installing skills from ref '${skills_ref}' into ${SKILLS_DIR}"
  fi

  install_skills_from_ref "${skills_ref}" "${SKILLS_DIR}" "${requested_skill}"
}

install_completion_file() {
  local binary="$1"
  local completion_dir="$2"
  local completion_file=""

  completion_file="$(completion_file_name)"
  mkdir -p "${completion_dir}"
  "${binary}" completions "${COMPLETION_SHELL}" > "${completion_dir}/${completion_file}"
  echo "Installed ${COMPLETION_SHELL} completions to ${completion_dir}/${completion_file}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --non-interactive|-y)
      NON_INTERACTIVE="1"
      ;;
    --dry-run)
      DRY_RUN="1"
      ;;
    --install-skills)
      INSTALL_SKILLS="1"
      ;;
    --skills-dir)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --skills-dir" >&2
        exit 1
      fi
      SKILLS_DIR="$2"
      shift
      ;;
    --skill)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --skill" >&2
        exit 1
      fi
      SKILL_NAME="$2"
      shift
      ;;
    --install-completions)
      INSTALL_COMPLETIONS="1"
      ;;
    --shell)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --shell" >&2
        exit 1
      fi
      COMPLETION_SHELL="$2"
      shift
      ;;
    --completions-dir)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --completions-dir" >&2
        exit 1
      fi
      COMPLETIONS_DIR="$2"
      shift
      ;;
    --install-dir)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --install-dir" >&2
        exit 1
      fi
      INSTALL_DIR="$2"
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift
done

resolve_skill_install_selection
validate_completion_selection

OS="$(uname -s)"
ARCH="$(uname -m)"

asset_os=""
case "${OS}" in
  Linux) asset_os="linux" ;;
  Darwin) asset_os="macos" ;;
  MINGW*|MSYS*|CYGWIN*) asset_os="windows" ;;
  *) echo "Unsupported OS: ${OS}" >&2; exit 1 ;;
esac

case "${ARCH}" in
  x86_64|amd64) asset_arch="x86_64" ;;
  arm64|aarch64)
    if [[ "${asset_os}" == "macos" ]]; then
      asset_arch="arm64"
    else
      echo "Unsupported arch for release assets: ${ARCH}" >&2
      exit 1
    fi
    ;;
  *) echo "Unsupported arch: ${ARCH}" >&2; exit 1 ;;
esac

if [[ "${asset_os}" == "windows" ]]; then
  ext=".exe"
else
  ext=""
fi

asset="time-keep-${asset_os}-${asset_arch}.tar.gz"
checksum_file="time-keep-${asset_os}-${asset_arch}.sha256"
if [[ "${VERSION}" == "latest" ]]; then
  url="${RELEASE_BASE_URL}/latest/download/${asset}"
  checksum_url="${RELEASE_BASE_URL}/latest/download/${checksum_file}"
else
  url="${RELEASE_BASE_URL}/download/${VERSION}/${asset}"
  checksum_url="${RELEASE_BASE_URL}/download/${VERSION}/${checksum_file}"
fi

if [[ "${INSTALL_COMPLETIONS}" == "1" && -z "${COMPLETIONS_DIR}" ]]; then
  COMPLETIONS_DIR="$(default_completions_dir)"
fi

if [[ "${DRY_RUN}" == "1" ]]; then
  cat <<EOF
time-keep install dry run
  repo: ${REPO_SLUG}
  release base url: ${RELEASE_BASE_URL}
  version: ${VERSION}
  asset: ${asset}
  asset url: ${url}
  checksum: ${checksum_file}
  checksum url: ${checksum_url}
  install dir: ${INSTALL_DIR}
  install skills: ${INSTALL_SKILLS}
  skills dir: ${SKILLS_DIR}
  skill name: ${SKILL_NAME:-<all>}
  install completions: ${INSTALL_COMPLETIONS}
  completion shell: ${COMPLETION_SHELL:-<none>}
  completions dir: ${COMPLETIONS_DIR:-<none>}
  verify checksum: ${VERIFY_CHECKSUM}
EOF
  exit 0
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

echo "Downloading ${url}"
download_file "${asset}" "${url}" "${tmp_dir}/${asset}"

if [[ "${VERIFY_CHECKSUM}" == "1" ]]; then
  echo "Downloading ${checksum_url}"
  download_file "${checksum_file}" "${checksum_url}" "${tmp_dir}/${checksum_file}"
  echo "Verifying SHA-256 checksum"
  verify_checksum_file "${tmp_dir}/${asset}" "${tmp_dir}/${checksum_file}"
fi

tar -xzf "${tmp_dir}/${asset}" -C "${tmp_dir}"
binary_path="$(find "${tmp_dir}" -type f -name "time-keep${ext}" | head -n 1)"
if [[ -z "${binary_path}" ]]; then
  echo "time-keep binary was not found in ${asset}." >&2
  exit 1
fi

mkdir -p "${INSTALL_DIR}"
cp "${binary_path}" "${INSTALL_DIR}/time-keep${ext}"
chmod +x "${INSTALL_DIR}/time-keep${ext}"

installed_binary="${INSTALL_DIR}/time-keep${ext}"

if [[ "${INSTALL_COMPLETIONS}" == "1" ]]; then
  install_completion_file "${installed_binary}" "${COMPLETIONS_DIR}"
fi

if [[ "${INSTALL_SKILLS}" == "1" ]]; then
  install_selected_skills "${SKILL_NAME}"
elif [[ "${NON_INTERACTIVE}" != "1" && -t 0 ]]; then
  read -r -p "Install time-keep agent skills? [y/N]: " choice
  choice="${choice:-N}"
  case "${choice}" in
  [Yy])
    INSTALL_SKILLS="1"
    SKILL_NAME=""
    install_selected_skills ""
    ;;
  esac
fi

echo "Installed time-keep to ${installed_binary}"
echo "Add ${INSTALL_DIR} to your PATH if needed."
