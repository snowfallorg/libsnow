#!/usr/bin/env bash
set -euo pipefail

CHANNEL=""
DRY_RUN=false
NO_UPLOAD=false
MAX_RELEASES=3
BUCKET="${S3_BUCKET:-libsnow}"
OUTPUT="/tmp/dbs"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)    DRY_RUN=true; shift ;;
    --no-upload)  NO_UPLOAD=true; shift ;;
    --max)        MAX_RELEASES="$2"; shift 2 ;;
    --bucket)     BUCKET="$2"; shift 2 ;;
    --output)     OUTPUT="$2"; shift 2 ;;
    -*)           echo "Unknown option: $1" >&2; exit 1 ;;
    *)
      if [ -z "${CHANNEL}" ]; then
        CHANNEL="$1"
      else
        echo "Unexpected argument: $1" >&2; exit 1
      fi
      shift
      ;;
  esac
done

if [ -z "${CHANNEL}" ]; then
  echo "Usage: $0 <channel> [--dry-run] [--no-upload] [--max N] [--bucket NAME] [--output DIR]" >&2
  exit 1
fi

mkdir -p "${OUTPUT}"

TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR}"' EXIT
PAIRS_FILE="${TMPDIR}/pairs.txt"

MARKER=""
TRUNCATED="true"

while [ "${TRUNCATED}" = "true" ]; do
  URL="https://nix-releases.s3.amazonaws.com/?delimiter=/&prefix=${CHANNEL}/&marker=${MARKER}"
  RESPONSE=$(curl -sf "${URL}")
  echo "${RESPONSE}" \
    | grep -oP '<Key>\K[^<]+|<LastModified>\K[^<]+' \
    | paste - - \
    >> "${PAIRS_FILE}"
  TRUNCATED=$(echo "${RESPONSE}" | grep -oP '<IsTruncated>\K[^<]+' || echo "false")
  if [ "${TRUNCATED}" = "true" ]; then
    MARKER=$(awk 'END{print $1}' "${PAIRS_FILE}")
  fi
done

if [ ! -s "${PAIRS_FILE}" ]; then
  echo "No releases found for ${CHANNEL}"
  exit 0
fi

mapfile -t RELEASES < <(
  sort -k2 "${PAIRS_FILE}" \
    | tail -n "${MAX_RELEASES}" \
    | cut -f1 \
    | awk -F'/' '{print $NF}'
)

PENDING=()
for (( i=${#RELEASES[@]}-1; i>=0; i-- )); do
  RELEASE="${RELEASES[$i]}"

  GIT_REV=$(curl -sf "https://releases.nixos.org/${CHANNEL}/${RELEASE}/git-revision" || true)
  if [ -z "${GIT_REV}" ]; then continue; fi
  GIT_REV=$(echo "${GIT_REV}" | tr -d '[:space:]')

  if aws s3api head-object --bucket "${BUCKET}" --key "db/${GIT_REV}" >/dev/null 2>&1; then
    break
  fi

  PENDING+=("${RELEASE}|${GIT_REV}")
done

if [ ${#PENDING[@]} -eq 0 ]; then
  echo "Up to date for ${CHANNEL}"
  exit 0
fi

echo "${#PENDING[@]} new revision(s) to process for ${CHANNEL}"

if [ "${DRY_RUN}" = true ]; then
  for (( i=${#PENDING[@]}-1; i>=0; i-- )); do
    IFS='|' read -r RELEASE GIT_REV <<< "${PENDING[$i]}"
    echo "  ${CHANNEL}/${RELEASE} -> ${GIT_REV}"
  done
  exit 0
fi

GENERATED=0
FAILED=0

for (( i=${#PENDING[@]}-1; i>=0; i-- )); do
  IFS='|' read -r RELEASE GIT_REV <<< "${PENDING[$i]}"

  if generate-db --channel "${CHANNEL}" --release "${RELEASE}" --output "${OUTPUT}" --verbose; then
    DB_FILE="${OUTPUT}/${GIT_REV}.db"
    if [ ! -f "${DB_FILE}" ]; then
      echo "ERROR: expected ${DB_FILE} not found" >&2
      FAILED=$((FAILED + 1))
      continue
    fi

    brotli --rm "${DB_FILE}"
    BR_FILE="${DB_FILE}.br"

    if [ "${NO_UPLOAD}" = true ]; then
      GENERATED=$((GENERATED + 1))
    else
      aws s3api put-object \
        --bucket "${BUCKET}" \
        --key "db/${GIT_REV}" \
        --body "${BR_FILE}" \
        --content-encoding br
      GENERATED=$((GENERATED + 1))
      rm -f "${BR_FILE}"
    fi
  else
    echo "FAILED ${CHANNEL}/${RELEASE}" >&2
    FAILED=$((FAILED + 1))
  fi
done

echo "Done: ${GENERATED} generated, ${FAILED} failed"
[ ${FAILED} -gt 0 ] && [ ${GENERATED} -eq 0 ] && exit 1
exit 0
