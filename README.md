# hf2gguf

One-step Hugging Face to quantized GGUF converter, built on top of llama.cpp.

Wraps `convert_hf_to_gguf.py` (unquantized base GGUF) and a statically
linked `llama-quantize` (final quantized GGUF) into a single command,
then deletes the intermediate file on success.

## Requirements

- A llama.cpp checkout built with static linking:

  ```sh
  cmake -B build -DBUILD_SHARED_LIBS=OFF -DCMAKE_BUILD_TYPE=Release
  cmake --build build -j
  ```

- System Python with the converter dependencies installed (no venv needed):

  ```sh
  pip3 install --break-system-packages -r requirements/requirements-convert_hf_to_gguf.txt
  ```

## Install

```sh
cp hf2gguf /path/to/llama.cpp/hf2gguf
ln -s /path/to/llama.cpp/hf2gguf /usr/local/bin/hf2gguf
```

The script locates llama.cpp via `$LLAMA_CPP_DIR`, or the directory
containing the script itself when placed at the repo root (symlinks
are resolved, so a PATH symlink works).

## Usage

```sh
hf2gguf <input_hf_dir_or_file> <output_gguf_path> [quant_type]
```

- `quant_type` defaults to `Q4_K_M`. Any `llama-quantize` type works
  (`Q4_0`, `Q8_0`, `Q5_K_M`, ...). `F16` and `BF16` skip the quantize
  step entirely.
- The input can be the model directory itself or any file inside it
  (e.g. `config.json`, `model.safetensors.index.json`) - the parent
  directory is used automatically.

## Example

```sh
hf2gguf /opt/my-model/config.json /opt/my-model/model.gguf Q4_K_M
```

Produces `/opt/my-model/model.gguf`, writing and then removing the
temporary `model-f16.gguf` along the way.
