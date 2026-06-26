#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MULTILINGUAL=0

for arg in "$@"; do
    case "$arg" in
        --multilingual|-m)
            MULTILINGUAL=1
            ;;
        *)
            echo "usage: $0 [--multilingual]" >&2
            exit 2
            ;;
    esac
done

if [ "$MULTILINGUAL" -eq 1 ]; then
    MODEL_DIR="$PROJECT_ROOT/models/nemotron_multi"
    REPO_FOLDER="nemotron-3.5-asr-streaming-0.6b-onnx"
else
    MODEL_DIR="$PROJECT_ROOT/models/nemotron"
    REPO_FOLDER="nemotron-speech-streaming-en-0.6b"
fi

BASE_URL="https://huggingface.co/altunenes/parakeet-rs/resolve/main/$REPO_FOLDER"
FILES=(
    "encoder.onnx"
    "encoder.onnx.data"
    "decoder_joint.onnx"
    "tokenizer.model"
)

mkdir -p "$MODEL_DIR"

for file in "${FILES[@]}"; do
    target="$MODEL_DIR/$file"
    if [ -f "$target" ]; then
        echo "Already exists: $target"
        continue
    fi

    echo "Downloading $file..."
    curl -L --fail --retry 3 --output "$target" "$BASE_URL/$file?download=true"
done

echo "Nemotron files are ready in $MODEL_DIR"
