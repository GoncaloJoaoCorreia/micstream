#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"
CHANGELOG_FILE="${2:-CHANGELOG.md}"
OUTPUT_FILE="${3:-release-notes.md}"

if [ -z "$VERSION" ]; then
  echo "Error: Version argument required (e.g., 0.1.0)" >&2
  exit 1
fi

if [ ! -f "$CHANGELOG_FILE" ]; then
  echo "Error: Changelog file '$CHANGELOG_FILE' not found." >&2
  exit 1
fi

# Clean version string (strip leading 'v')
VERSION_CLEAN="${VERSION#v}"

# Check for existence of header '## [X.Y.Z]'
if ! grep -q -E "^##[[:space:]]+\[${VERSION_CLEAN}\]" "$CHANGELOG_FILE"; then
  echo "Error: No entry found for version [${VERSION_CLEAN}] in ${CHANGELOG_FILE}." >&2
  echo "Please add a '## [${VERSION_CLEAN}] - YYYY-MM-DD' section before pushing." >&2
  exit 1
fi

# Extract lines between target header and next '## ' header, trimming outer blank lines
awk -v ver="$VERSION_CLEAN" '
  BEGIN { inside=0; first=0; last=0 }
  $0 ~ "^##[[:space:]]+\\[" ver "\\]" { inside=1; next }
  inside && /^##[[:space:]]+/ { inside=0 }
  inside {
    lines[NR] = $0
    if ($0 !~ /^[[:space:]]*$/) {
      if (first == 0) first = NR
      last = NR
    }
  }
  END {
    if (first > 0) {
      for (i = first; i <= last; i++) {
        print lines[i]
      }
    }
  }
' "$CHANGELOG_FILE" > "$OUTPUT_FILE"

if [ ! -s "$OUTPUT_FILE" ]; then
  echo "Error: Release notes for version [${VERSION_CLEAN}] are empty in ${CHANGELOG_FILE}." >&2
  exit 1
fi

echo "Successfully extracted release notes for v${VERSION_CLEAN} into ${OUTPUT_FILE}:"
cat "$OUTPUT_FILE"
