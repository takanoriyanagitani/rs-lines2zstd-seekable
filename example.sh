#!/bin/bash

wsm="./lines2zeek.wasm"

level=fast
max_line_len=32767
max_frame_count=65535

ozeek="./sample.d/out.seekable.zst"
mkdir -p "./sample.d"

run_wasi() {
	cat /dev/stdin |
		wasmtime \
			run \
			--env ENV_LEVEL=$level \
			--env ENV_MAX_LINE_SIZE=$max_line_len \
			--env ENV_MAX_FRAME_COUNT=$max_frame_count \
			"${wsm}"
}

input() {
	echo helo
	echo wrld
}

echo creating the seekable...
input | run_wasi >"${ozeek}"

echo printing using standard decoder
cat "${ozeek}" | zstdcat
echo

echo printing using zeekstd
which zeekstd | fgrep -q zeekstd || exec sh -c '
  echo zeekstd missing.
  exit 1
'
zeekstd list --detail "${ozeek}"
