import os
import subprocess
import platform
import argparse
import time
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

# --- Configuration ---
SHADER_ROOT_DIR = Path("./examples/shaders")
OUTPUT_DIR      = Path("./examples/shaders/compiled")
GLSLC_COMMAND   = "glslc"
GLSLC_FLAGS     = [
    "--target-env=vulkan1.2",   # SPIR-V target env
    "-std=460",                 # GLSL 4.60
    "-O",                       # optimize
    # "-g",                       # include debug info (remove for release)
]

SHADER_EXTENSIONS = {
    ".vert": "vertex",
    ".frag": "fragment",
    ".comp": "compute",
}

# --- Compilation ---

def compile_shader(input_path: Path) -> tuple[bool, str]:
    """
    Compiles a single GLSL shader to SPIR-V.
    Returns (success, message).
    """
    suffix    = input_path.suffix
    stage     = SHADER_EXTENSIONS.get(suffix)
    if stage is None:
        return False, f"Unknown shader stage for extension '{suffix}'"

    # mirror the source directory structure under OUTPUT_DIR
    relative  = input_path.relative_to(SHADER_ROOT_DIR)
    out_path  = OUTPUT_DIR / relative.with_suffix(suffix + ".spv")
    out_path.parent.mkdir(parents=True, exist_ok=True)

    cmd = [
        GLSLC_COMMAND,
        f"-fshader-stage={stage}",
        *GLSLC_FLAGS,
        str(input_path),
        "-o", str(out_path),
    ]

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            # glslc writes errors to stdout, warnings to stderr
            error = (result.stderr + result.stdout).strip()
            return False, f"GLSL ERROR in {input_path.name}:\n    {error}"

        size_kb = out_path.stat().st_size / 1024
        return True, f"OK  {input_path.name:40s} -> {out_path.name} ({size_kb:.1f} KB)"

    except FileNotFoundError:
        return False, f"ERROR: '{GLSLC_COMMAND}' not found. Is the Vulkan SDK installed?"

def collect_shaders() -> list[Path]:
    if not SHADER_ROOT_DIR.exists():
        print(f"Error: shader directory '{SHADER_ROOT_DIR}' not found.")
        return []

    shaders = []
    for ext in SHADER_EXTENSIONS:
        shaders.extend(SHADER_ROOT_DIR.rglob(f"*{ext}"))
    return sorted(shaders)

def compile_all(shaders: list[Path], jobs: int = 4) -> tuple[int, int]:
    success = 0
    failure = 0
    with ThreadPoolExecutor(max_workers=jobs) as executor:
        futures = {executor.submit(compile_shader, s): s for s in shaders}
        for future in as_completed(futures):
            ok, msg = future.result()
            prefix = "  ✓" if ok else "  ✗"
            print(f"{prefix} {msg}")
            if ok: success += 1
            else:  failure += 1
    return success, failure

# --- Watch Mode ---

def get_mtimes(shaders: list[Path]) -> dict[Path, float]:
    return {s: s.stat().st_mtime for s in shaders if s.exists()}

def watch(jobs: int):
    print("Watching for shader changes... (Ctrl+C to stop)\n")
    shaders  = collect_shaders()
    mtimes   = get_mtimes(shaders)

    # initial full build
    success, failure = compile_all(shaders, jobs)
    print(f"\n  Initial build: {success} OK, {failure} failed\n")

    try:
        while True:
            time.sleep(0.5)
            # re-scan in case new files were added
            shaders     = collect_shaders()
            new_mtimes  = get_mtimes(shaders)
            changed     = [
                s for s in shaders
                if new_mtimes.get(s) != mtimes.get(s)
            ]
            if changed:
                print(f"\n  Detected {len(changed)} change(s):")
                compile_all(changed, jobs)
                mtimes = new_mtimes
    except KeyboardInterrupt:
        print("\nWatch stopped.")

# --- Entry Point ---

def main():
    parser = argparse.ArgumentParser(description="WebGPU/wgpu-native GLSL -> SPIR-V shader compiler")
    parser.add_argument("--watch",  "-w", action="store_true", help="Watch for changes and recompile")
    parser.add_argument("--jobs",   "-j", type=int, default=4,  help="Parallel compile jobs (default: 4)")
    parser.add_argument("--release","-r", action="store_true", help="Release build (strip debug info)")
    args = parser.parse_args()

    if args.release and "-g" in GLSLC_FLAGS:
        GLSLC_FLAGS.remove("-g")

    print(f"  Shader dir : {SHADER_ROOT_DIR}")
    print(f"  Output dir : {OUTPUT_DIR}")
    print(f"  Target     : Vulkan 1.2 / SPIR-V (GLSL 460)")
    print(f"  Mode       : {'release' if args.release else 'debug'}\n")

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    if args.watch:
        watch(args.jobs)
    else:
        shaders = collect_shaders()
        if not shaders:
            print("No shaders found.")
            return
        print(f"  Found {len(shaders)} shader(s)\n")
        success, failure = compile_all(shaders, args.jobs)
        print(f"\n  {success} OK, {failure} failed")
        exit(0 if failure == 0 else 1)

if __name__ == "__main__":
    main()
