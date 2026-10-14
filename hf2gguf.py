#!/usr/bin/env python3
"""hf2gguf - one-step Hugging Face -> quantized GGUF converter.

Usage: hf2gguf <input_hf_dir_or_file> <output_gguf_path> [quant_type]

Runs convert_hf_to_gguf.py to produce an F16/BF16 base GGUF, then the
statically linked llama-quantize binary to produce the final quantized
file. The intermediate base file is deleted on success.

The llama.cpp checkout is located via $LLAMA_CPP_DIR, or the directory
containing this script when it lives at the repo root.
"""

import os
import subprocess
import sys

LLAMA_DIR = os.environ.get("LLAMA_CPP_DIR") or os.path.dirname(os.path.realpath(__file__))
CONVERT = os.path.join(LLAMA_DIR, "convert_hf_to_gguf.py")
QUANTIZE = os.path.join(LLAMA_DIR, "build", "bin", "llama-quantize")

F16_BASE_TYPES = {"F16", "BF16"}  # produced directly, no quantize step


def fail(msg):
    sys.exit(f"hf2gguf: error: {msg}")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("-")]
    if len(args) < 2 or len(args) > 3 or len(args) != len(sys.argv) - 1:
        fail("usage: hf2gguf <input_hf_dir_or_file> <output_gguf_path> [quant_type]")

    src, out = args[0], args[1]
    quant = args[2].upper() if len(args) == 3 else "Q4_K_M"

    # a file inside a model dir (config.json, *.safetensors, index, etc.)
    # means the parent directory is the model source
    if os.path.isfile(src):
        src = os.path.dirname(os.path.abspath(src))
    if not os.path.isdir(src):
        fail(f"input not found: {src}")
    if not os.path.isfile(os.path.join(src, "config.json")):
        fail(f"{src} does not look like a HF model directory (no config.json)")

    out = os.path.abspath(out)
    os.makedirs(os.path.dirname(out), exist_ok=True)

    base_type = quant if quant in F16_BASE_TYPES else "F16"
    base = out if quant in F16_BASE_TYPES else out.removesuffix(".gguf") + f"-{base_type.lower()}.gguf"

    if not os.path.isfile(CONVERT):
        fail(f"converter not found: {CONVERT}")
    if quant not in F16_BASE_TYPES and not os.access(QUANTIZE, os.X_OK):
        fail(f"quantize binary not found or not executable: {QUANTIZE} (rebuild with -DBUILD_SHARED_LIBS=OFF)")

    print(f"[hf2gguf] step 1/2: {src} -> {base} ({base_type})")
    subprocess.run(
        [sys.executable, CONVERT, src, "--outfile", base, "--outtype", base_type.lower()],
        check=True,
    )

    try:
        if quant in F16_BASE_TYPES:
            print(f"[hf2gguf] done (no quantization needed for {quant}): {out}")
        else:
            print(f"[hf2gguf] step 2/2: {base} -> {out} ({quant})")
            subprocess.run(
                [QUANTIZE, base, out, quant],
                check=True,
                env={**os.environ, "LD_LIBRARY_PATH": ""},
            )
            os.remove(base)
            print(f"[hf2gguf] removed intermediate {base}")
            print(f"[hf2gguf] done: {out}")
    finally:
        if quant not in F16_BASE_TYPES and not os.path.isfile(out) and os.path.isfile(base):
            os.remove(base)  # quantize failed: don't leave a stray base file


if __name__ == "__main__":
    main()
