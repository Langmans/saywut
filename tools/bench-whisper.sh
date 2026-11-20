#!/usr/bin/env bash
# Builds the release CLI once per whisper backend and times it on the TTS clips.
#
#   tools/bench-whisper.sh [cpu] [vulkan] [cuda]   (default: all three)
#
# Each backend gets its own target directory, so switching features does not
# rebuild whisper.cpp every time. The Vulkan build nests a sub-project so deep
# that on Windows paths pass the 260-character limit (error C1083) under the
# repository's target/; point SAYWUT_BENCH_DIR at a short path such as
# C:/Users/<you>/.sw there. Needs the model in models/ and the clips in
# testdata/tts/ (tools/tts-clips.ps1). Prints seconds per clip and the
# real-time factor (processing time / audio length; below 1 is faster than
# real time).
set -euo pipefail
cd "$(dirname "$0")/.."

backends=("$@")
[ ${#backends[@]} -eq 0 ] && backends=(cpu vulkan cuda)

for backend in "${backends[@]}"; do
    features=()
    [ "$backend" != cpu ] && features=(--features "$backend")
    target="${SAYWUT_BENCH_DIR:-target/bench}/${backend:0:2}"
    echo "== $backend: building"
    cargo build --quiet --release -p saywut-cli --target-dir "$target" "${features[@]}"
    bin="$target/release/saywut"
    for wav in testdata/tts/*.wav; do
        name=$(basename "$wav" .wav)
        lang=${name%%-*}
        audio_ms=$(( ($(stat -c %s "$wav") - 44) / 32 ))
        start=$(date +%s%N)
        "$bin" --model-dir models transcribe "$wav" -l "$lang" -f txt > "$target/$name.txt"
        took_ms=$(( ($(date +%s%N) - start) / 1000000 ))
        awk -v b="$backend" -v n="$name" -v t="$took_ms" -v a="$audio_ms" \
            'BEGIN { printf "%-8s %-20s %6.1f s   RTF %.2f\n", b, n, t / 1000, t / a }'
    done
done
