#!/usr/bin/env bash
#
# Run the end-to-end test suite in a container.

set -euo pipefail

CRATE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE=reedline-bash-tests

if ! command -v docker >/dev/null; then
    echo "docker is not installed, and this script is nothing without it" >&2
    exit 1
fi


if ! build_log=$(DOCKER_BUILDKIT=1 docker build \
        --file "$CRATE/tests/Dockerfile" \
        --tag "$IMAGE" \
        "$CRATE" 2>&1); then
    printf '%s\n' "$build_log" >&2
    exit 1
fi

name="rlb-e2e-$$"
trap 'docker rm --force "$name" >/dev/null 2>&1 || true' EXIT INT TERM

status=0
docker run --rm --name "$name" \
    --network none \
    --read-only \
    --tmpfs /tmp:exec \
    --tmpfs /home/test:uid=1001,gid=1001,mode=0700 \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    "$IMAGE" "$@" || status=$?
exit $status
