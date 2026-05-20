#!/usr/bin/env sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 /path/to/stat-rain.sock" >&2
  exit 2
fi

socket="$1"

while IFS= read -r line; do
  stat-rain send --socket "$socket" --message "$line"
done
