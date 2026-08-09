#!/usr/bin/env bash
set -euo pipefail

cache=${1:?Gradle modules cache is required}
output=${2:?output manifest is required}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

find "$cache" -type f -print0 | xargs -0 -P16 -n1 bash -c '
  set -euo pipefail
  cache=$1
  work=$2
  source=$3
  relative=${source#"$cache"/}
  group=${relative%%/*}
  rest=${relative#*/}
  module=${rest%%/*}
  rest=${rest#*/}
  version=${rest%%/*}
  filename=${relative##*/}
  path=${group//./\/}/$module/$version/$filename
  source_digest=$(sha256sum "$source" | cut -d" " -f1)
  candidate="$work/candidate.$$"
  for base in \
    https://dl.google.com/dl/android/maven2 \
    https://repo.maven.apache.org/maven2 \
    https://plugins.gradle.org/m2 \
    https://storage.googleapis.com/download.flutter.io; do
    url=$base/$path
    if curl -L --fail --silent --output "$candidate" "$url" &&
       [ "$(sha256sum "$candidate" | cut -d" " -f1)" = "$source_digest" ]; then
      digest=$(printf "%s" "$source_digest" | xxd -r -p | base64 -w0)
      key=$(printf "%s" "$path" | sha256sum | cut -d" " -f1)
      printf "%s\t%s\tsha256-%s\n" "$path" "$url" "$digest" > "$work/$key"
      rm -f "$candidate"
      exit 0
    fi
  done
  rm -f "$candidate"
  echo "No repository URL found for $relative" >&2
  exit 1
' _ "$cache" "$work"

{
  printf '[\n'
  first=1
  while IFS=$'\t' read -r path url integrity; do
    if (( first )); then first=0; else printf ',\n'; fi
    printf '  {"path":"%s","url":"%s","integrity":"%s"}' "$path" "$url" "$integrity"
  done < <(sort "$work"/*)
  printf '\n]\n'
} > "$output"
