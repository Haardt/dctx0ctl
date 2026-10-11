#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
step=${1:-}
if [[ ! "$step" =~ ^([1-9]|1[0-6])$ ]]; then
  echo "Usage: $0 <1..16> [-- <agent command and options>]" >&2
  exit 2
fi
shift
mapfile -t matches < <(find "$root/examples" -maxdepth 1 -type d -name "tutorial-step-$step-*" | sort)
if [[ ${#matches[@]} != 1 ]]; then
  echo "Step $step is not available yet (expected exactly one directory)." >&2
  exit 1
fi
reference=${matches[0]}
exercise=$(mktemp -d "${TMPDIR:-/tmp}/dctx0-tutorial-step-$step-XXXXXXXX")
# The complete reference is retained in Git. Only the exercise copy is reset.
mkdir -p "$exercise/dctx0"
while IFS= read -r -d '' file; do
  relative=${file#"$reference/dctx0/"}
  mkdir -p "$exercise/dctx0/$(dirname -- "$relative")"
  cp "$file" "$exercise/dctx0/$relative"
done < <(find "$reference/dctx0" -type f -name '*.dctx0' -print0)
cp "$reference/package.json" "$reference/bun.lock" "$reference/tsconfig.json" "$exercise/"
mkdir -p "$exercise/.dctx0/testing/typescript" "$exercise/src"
cp "$reference/.dctx0/testing/verification.json" "$exercise/.dctx0/testing/"
while IFS= read -r -d '' file; do
  cp "$file" "$exercise/.dctx0/testing/typescript/"
done < <(find "$reference/.dctx0/testing/typescript" -maxdepth 1 -type f ! -name 'inspection-notes.json' ! -name 'worker.ts' ! -name 'module-observer.ts' -print0)
if (( step > 1 )); then
  previous=$((step - 1))
  mapfile -t predecessors < <(find "$root/examples" -maxdepth 1 -type d -name "tutorial-step-$previous-*" | sort)
  if [[ ${#predecessors[@]} != 1 ]]; then
    echo "Previous step $previous is missing; exercise retained at $exercise" >&2
    exit 1
  fi
  cp -R "${predecessors[0]}/src/." "$exercise/src/"
fi
# Acceptance tests are supplied; production code starts at the previous step.
find "$reference/src" -type f -name '*.test.ts' -print0 |
  while IFS= read -r -d '' file; do
    relative=${file#"$reference/src/"}
    mkdir -p "$exercise/src/$(dirname -- "$relative")"
    cp "$file" "$exercise/src/$relative"
  done
# Root entry-point acceptance is supplied with the current step as well.
while IFS= read -r -d '' file; do
  cp "$file" "$exercise/"
done < <(find "$reference" -maxdepth 1 -type f -name '*.test.ts' -print0)
binary=${DCTX0_BIN:-"$root/target/release/dctx0"}
cat > "$exercise/AGENTS.md" <<EOF
# Tutorial exercise $step

Implement the complete reviewed dctx0 specification in dctx0/ using normal
TypeScript in src/. The testing adapter calls actual production code; it does
not implement business rules. Retain the spec, fixtures and acceptance tests.
Do not import testing or spec artifacts from production. Do not copy answers
or approve an inspection merely because the examples pass.

The starting production code is the previous step (empty at step 1).
Complete this step, install the frozen dependencies, run strict type checking,
run every provided test with 100% production line/function coverage, and run:

    bun run test --coverage

    "$binary" check dctx0
    "$binary" verify run dctx0 --project . --no-adopt

Native failures and UNKNOWN cases must be diagnosed, not hidden. When manual
requirements remain, inspect their actual implementation and author fresh
source/spec-bound inspection-notes.json in the testing directory before using
verify complete. Existing reference approvals are deliberately not supplied.
Explain any substantive findings and retain reproducible evidence. Do not
commit, push or alter the full reference tutorial from this exercise copy.
If server.test.ts is supplied, implement its actual server.ts entry point and
the interactive page as well: new tasks, Done/Open status, real HTTP writes,
retry after errors and safe text rendering. Export the real server so tests
can start it on PORT=0 and stop it. The normal start command uses loopback.
EOF
cat > "$exercise/TASK.md" <<EOF
Implement tutorial step $step in this fresh exercise directory. Read AGENTS.md,
the full dctx0 specification and the supplied tests, then execute the checks.
The specification includes all previous requirements and this step's additions.
EOF
printf 'Exercise: %s\nInstructions: %s\n' "$exercise" "$exercise/TASK.md"
if (( $# > 0 )); then
  if [[ $1 != -- || $# -lt 2 ]]; then
    echo 'Expected -- followed by your agent command.' >&2
    exit 2
  fi
  shift
  prompt=$(cat "$exercise/TASK.md")
  (cd -- "$exercise" && "$@" "$prompt")
fi
