// hf2gguf - one-step Hugging Face -> quantized GGUF converter.
//
// Usage: hf2gguf <input_hf_dir_or_file> <output_gguf_path> [quant_type]
//
// Runs convert_hf_to_gguf.py to produce an F16/BF16 base GGUF, then the
// statically linked llama-quantize binary to produce the final quantized
// file. The intermediate base file is deleted on success.
//
// The llama.cpp checkout is located via $LLAMA_CPP_DIR, or the directory
// containing this executable when it lives at the repo root (or is
// symlinked there).
//
// Build: rustc -O hf2gguf.rs

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

fn fail(msg: &str) -> ! {
    eprintln!("hf2gguf: error: {msg}");
    exit(1);
}

fn absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        match env::current_dir() {
            Ok(cwd) => cwd.join(p),
            Err(e) => fail(&format!("cannot determine cwd: {e}")),
        }
    }
}

fn run(cmd: &mut Command, what: &str) {
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => fail(&format!("failed to start {what}: {e}")),
    };
    if !status.success() {
        fail(&format!("{what} exited with {status}"));
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 3 || args.iter().any(|a| a.starts_with('-')) {
        fail("usage: hf2gguf <input_hf_dir_or_file> <output_gguf_path> [quant_type]");
    }

    let quant = args.get(2).map(|s| s.to_uppercase()).unwrap_or_else(|| "Q4_K_M".into());

    // a file inside a model dir (config.json, *.safetensors, index, etc.)
    // means the parent directory is the model source
    let src = absolute(Path::new(&args[0]));
    let src = if src.is_file() {
        match src.parent() {
            Some(p) => p.to_path_buf(),
            None => fail(&format!("no parent directory for {}", src.display())),
        }
    } else {
        src
    };
    if !src.is_dir() {
        fail(&format!("input not found: {}", src.display()));
    }
    if !src.join("config.json").is_file() {
        fail(&format!("{} does not look like a HF model directory (no config.json)", src.display()));
    }

    let out = absolute(Path::new(&args[1]));
    if let Some(dir) = out.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            fail(&format!("cannot create {}: {e}", dir.display()));
        }
    }

    let is_base = quant == "F16" || quant == "BF16";
    let base_type = if is_base { quant.as_str() } else { "F16" };
    let base = if is_base {
        out.clone()
    } else {
        let stem = out.to_string_lossy();
        let stem = stem.strip_suffix(".gguf").unwrap_or(&stem);
        PathBuf::from(format!("{stem}-{}.gguf", base_type.to_lowercase()))
    };

    let llama_dir = match env::var_os("LLAMA_CPP_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            let exe = match env::current_exe().and_then(|p| fs::canonicalize(p)) {
                Ok(p) => p,
                Err(e) => fail(&format!("cannot resolve own path: {e}")),
            };
            exe.parent().unwrap_or(Path::new(".")).to_path_buf()
        }
    };
    let convert = llama_dir.join("convert_hf_to_gguf.py");
    let quantize = llama_dir.join("build/bin/llama-quantize");

    if !convert.is_file() {
        fail(&format!("converter not found: {} (set LLAMA_CPP_DIR)", convert.display()));
    }
    if !is_base {
        let ok = fs::metadata(&quantize)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        if !ok {
            fail(&format!("quantize binary not found or not executable: {} (rebuild with -DBUILD_SHARED_LIBS=OFF)", quantize.display()));
        }
    }

    println!("[hf2gguf] step 1/2: {} -> {} ({base_type})", src.display(), base.display());
    run(
        Command::new("python3")
            .arg(&convert)
            .arg(&src)
            .arg("--outfile").arg(&base)
            .arg("--outtype").arg(base_type.to_lowercase()),
        "convert_hf_to_gguf.py",
    );

    if is_base {
        println!("[hf2gguf] done (no quantization needed for {quant}): {}", out.display());
        return;
    }

    println!("[hf2gguf] step 2/2: {} -> {} ({quant})", base.display(), out.display());
    // no .so deps needed from the environment; run with a clean LD_LIBRARY_PATH
    let mut cmd = Command::new(&quantize);
    cmd.arg(&base).arg(&out).arg(&quant).env("LD_LIBRARY_PATH", "");
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => fail(&format!("failed to start llama-quantize: {e}")),
    };
    if !status.success() {
        // don't leave a stray base file on failure
        if !out.is_file() && base.is_file() {
            let _ = fs::remove_file(&base);
        }
        fail(&format!("llama-quantize exited with {status}"));
    }

    if let Err(e) = fs::remove_file(&base) {
        eprintln!("hf2gguf: warning: could not remove intermediate {}: {e}", base.display());
    } else {
        println!("[hf2gguf] removed intermediate {}", base.display());
    }
    println!("[hf2gguf] done: {}", out.display());
}
