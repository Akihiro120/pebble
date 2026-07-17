import os
import shutil
import subprocess
import platform
import argparse
import time
from dataclasses import dataclass, field
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

# --- Configuration ---
SHADER_ROOT_DIR = Path("./assets/shaders")
OUTPUT_DIR      = Path("./assets/shaders/compiled")
GLSLC_COMMAND   = "glslc"
NAGA_COMMAND    = "naga"          # optional: `cargo install naga-cli`
SPIRV_VAL_CMD   = "spirv-val"     # ships with the Vulkan SDK

GLSLC_FLAGS = [
    "--target-env=vulkan1.2",   # SPIR-V target env
    "-std=460",                 # GLSL 4.60
    "-O",                       # optimize
    # "-g",                       # debug info (stripped in --release)
]

SHADER_EXTENSIONS = {
    ".vert": "vertex",
    ".frag": "fragment",
    ".comp": "compute",
}

# --- ANSI colors (auto-disabled on unsupported terminals) ---

def _supports_color() -> bool:
    if os.environ.get("NO_COLOR") is not None:
        return False
    if platform.system() == "Windows" and "WT_SESSION" not in os.environ:
        return False
    return True

_COLOR = _supports_color()

def c(text: str, code: str) -> str:
    if not _COLOR:
        return text
    return f"\033[{code}m{text}\033[0m"

GREEN, RED, YELLOW, CYAN, DIM = "32", "31", "33", "36", "2"


# --- Result model ---

@dataclass
class StageResult:
    ok: bool
    stdout: str = ""
    stderr: str = ""

@dataclass
class ShaderResult:
    path: Path
    out_path: Path | None = None
    compile: StageResult | None = None
    validate: StageResult | None = None   # spirv-val
    naga: StageResult | None = None       # naga cross-check (wgpu-facing)

    @property
    def ok(self) -> bool:
        stages = [s for s in (self.compile, self.validate, self.naga) if s is not None]
        return all(s.ok for s in stages)

    @property
    def has_warnings(self) -> bool:
        # glslc returns 0 but can still write to stderr for warnings
        if self.compile and self.compile.ok and self.compile.stderr.strip():
            return True
        return False


# --- Tool availability ---

def tool_available(name: str) -> bool:
    return shutil.which(name) is not None


# --- Compilation pipeline ---

def run_cmd(cmd: list[str]) -> StageResult:
    try:
        result = subprocess.run(cmd, capture_output=True, text=True)
        return StageResult(ok=(result.returncode == 0), stdout=result.stdout, stderr=result.stderr)
    except FileNotFoundError:
        return StageResult(ok=False, stderr=f"'{cmd[0]}' not found on PATH")


def compile_shader(input_path: Path, run_validate: bool, run_naga: bool, verbose: bool) -> ShaderResult:
    """
    Runs the full pipeline for one shader: glslc -> spirv-val -> naga (optional).
    Every stage runs and is recorded independently, so a failure at one stage
    doesn't hide diagnostics from earlier stages.
    """
    suffix = input_path.suffix
    stage = SHADER_EXTENSIONS.get(suffix)
    result = ShaderResult(path=input_path)

    if stage is None:
        result.compile = StageResult(ok=False, stderr=f"Unknown shader stage for extension '{suffix}'")
        return result

    relative = input_path.relative_to(SHADER_ROOT_DIR)
    out_path = OUTPUT_DIR / relative.with_suffix(suffix + ".spv")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    result.out_path = out_path

    cmd = [
        GLSLC_COMMAND,
        f"-fshader-stage={stage}",
        *GLSLC_FLAGS,
        str(input_path),
        "-o", str(out_path),
    ]
    if verbose:
        print(c(f"  $ {' '.join(cmd)}", DIM))

    result.compile = run_cmd(cmd)
    if not result.compile.ok:
        # Don't bother running downstream stages against a stale/missing .spv
        return result

    if run_validate and tool_available(SPIRV_VAL_CMD):
        val_cmd = [SPIRV_VAL_CMD, str(out_path)]
        if verbose:
            print(c(f"  $ {' '.join(val_cmd)}", DIM))
        result.validate = run_cmd(val_cmd)

    if run_naga and tool_available(NAGA_COMMAND):
        # Some naga-cli versions choke on OpLine debug instructions (embedded by
        # glslc's -g flag) with a vague "unsupported instruction Line at Type"
        # error that has nothing to do with your actual shader code. So we
        # compile a separate, debug-info-free .spv purely for the naga check,
        # keeping your real -g build untouched for other tooling (RenderDoc etc).
        naga_check_flags = [f for f in GLSLC_FLAGS if f != "-g"]
        naga_check_spv = out_path.with_suffix(".naga_check.spv")
        strip_cmd = [
            GLSLC_COMMAND,
            f"-fshader-stage={stage}",
            *naga_check_flags,
            str(input_path),
            "-o", str(naga_check_spv),
        ]
        if verbose:
            print(c(f"  $ {' '.join(strip_cmd)}", DIM))
        strip_result = run_cmd(strip_cmd)

        if not strip_result.ok:
            # extremely unlikely (glslc already succeeded with -g moments ago)
            # but surface it rather than silently skipping the naga check
            result.naga = strip_result
            return result

        wgsl_out = out_path.with_suffix(".wgsl")
        naga_cmd = [NAGA_COMMAND, str(naga_check_spv), str(wgsl_out)]
        if verbose:
            print(c(f"  $ {' '.join(naga_cmd)}", DIM))
        result.naga = run_cmd(naga_cmd)

        # If it still fails even without debug info, this is a genuine
        # incompatibility -- disassemble so the user can correlate the SPIR-V
        # instruction naga complains about against their actual GLSL.
        if not result.naga.ok and tool_available("spirv-dis"):
            dis_out = out_path.with_suffix(".disasm.txt")
            dis_cmd = ["spirv-dis", str(naga_check_spv), "-o", str(dis_out)]
            if verbose:
                print(c(f"  $ {' '.join(dis_cmd)}", DIM))
            dis_result = run_cmd(dis_cmd)
            if dis_result.ok:
                result.naga.stderr += (
                    f"\n(disassembly written to {dis_out} -- search it for the "
                    f"failing instruction/type to trace it back to your GLSL)"
                )

        naga_check_spv.unlink(missing_ok=True)

    return result


def collect_shaders() -> list[Path]:
    if not SHADER_ROOT_DIR.exists():
        print(c(f"Error: shader directory '{SHADER_ROOT_DIR}' not found.", RED))
        return []
    shaders = []
    for ext in SHADER_EXTENSIONS:
        shaders.extend(SHADER_ROOT_DIR.rglob(f"*{ext}"))
    return sorted(shaders)


def print_result(result: ShaderResult):
    name = result.path.name
    if result.ok and not result.has_warnings:
        size_kb = result.out_path.stat().st_size / 1024 if result.out_path and result.out_path.exists() else 0
        print(c("  ✓", GREEN) + f" {name:40s} -> {result.out_path.name} ({size_kb:.1f} KB)")
        return

    if result.ok and result.has_warnings:
        print(c("  ⚠", YELLOW) + f" {name:40s} compiled with warnings")
        print(c(f"    [glslc warning]\n{_indent(result.compile.stderr)}", YELLOW))
        return

    print(c("  ✗", RED) + f" {name}")
    if result.compile and not result.compile.ok:
        err = (result.compile.stderr + result.compile.stdout).strip()
        print(c(f"    [glslc error]\n{_indent(err)}", RED))
    if result.validate and not result.validate.ok:
        err = (result.validate.stderr + result.validate.stdout).strip()
        print(c(f"    [spirv-val error]\n{_indent(err)}", RED))
    if result.naga and not result.naga.ok:
        err = (result.naga.stderr + result.naga.stdout).strip()
        print(c(f"    [naga/wgpu compatibility error]\n{_indent(err)}", RED))
        hint = _naga_hint(err)
        if hint:
            print(c(f"    [hint] {hint}", CYAN))


def _indent(text: str, spaces: int = 6) -> str:
    pad = " " * spaces
    return "\n".join(pad + line for line in text.splitlines()) if text else pad + "(no output)"


# Known naga error substrings -> plain-English hints. naga's SPIR-V frontend
# often reports generic instruction/type names rather than anything mapping
# cleanly back to GLSL source, so we pattern-match the common recurring cases.
_NAGA_HINTS = [
    ("unsupported instruction Line", "This is usually caused by OpLine debug info in the SPIR-V "
     "(from glslc's -g flag) that this naga-cli version can't parse -- unrelated to your actual "
     "shader logic. The naga check now compiles a debug-info-free variant automatically, so if "
     "you still see this, try `cargo install naga-cli --force` to update."),
    ("Bad matrix width", "A matrix type is being used where WGSL/naga expects only scalars or "
     "vectors -- most commonly a mat4/mat3 used directly as a vertex attribute or an "
     "interface-block (varying) member. Split it into vec4 columns and reassemble with mat4(...) "
     "inside main()."),
    ("invalid location", "A @location index collision or gap between shader stages -- check that "
     "vertex output locations and fragment input locations line up exactly."),
    ("Type(", "A type used in this shader isn't representable in WGSL as naga understands it. "
     "Common culprits: matrices or arrays placed directly on an interface-block member or vertex "
     "attribute, or an unsupported precision/opaque type."),
]


def _naga_hint(error_text: str) -> str | None:
    for needle, hint in _NAGA_HINTS:
        if needle in error_text:
            return hint
    return None


def compile_all(shaders: list[Path], jobs: int, run_validate: bool, run_naga: bool, verbose: bool) -> list[ShaderResult]:
    results = []
    with ThreadPoolExecutor(max_workers=jobs) as executor:
        futures = {
            executor.submit(compile_shader, s, run_validate, run_naga, verbose): s
            for s in shaders
        }
        for future in as_completed(futures):
            result = future.result()
            print_result(result)
            results.append(result)
    return results


def print_summary(results: list[ShaderResult]):
    ok = sum(1 for r in results if r.ok and not r.has_warnings)
    warned = sum(1 for r in results if r.ok and r.has_warnings)
    failed = [r for r in results if not r.ok]

    print()
    print(c(f"  {ok} OK", GREEN) + ", " +
          c(f"{warned} warned", YELLOW) + ", " +
          c(f"{len(failed)} failed", RED if failed else DIM))

    if failed:
        print()
        print(c("  Failed shaders:", RED))
        for r in failed:
            reason = "glslc" if (r.compile and not r.compile.ok) else \
                     "spirv-val" if (r.validate and not r.validate.ok) else \
                     "naga/wgpu" if (r.naga and not r.naga.ok) else "unknown"
            print(f"    - {r.path} ({reason})")

    return len(failed)


# --- Watch Mode ---

def get_mtimes(shaders: list[Path]) -> dict[Path, float]:
    return {s: s.stat().st_mtime for s in shaders if s.exists()}


def watch(jobs: int, run_validate: bool, run_naga: bool, verbose: bool):
    print("Watching for shader changes... (Ctrl+C to stop)\n")
    shaders = collect_shaders()
    mtimes = get_mtimes(shaders)

    results = compile_all(shaders, jobs, run_validate, run_naga, verbose)
    print_summary(results)

    try:
        while True:
            time.sleep(0.5)
            shaders = collect_shaders()
            new_mtimes = get_mtimes(shaders)
            changed = [s for s in shaders if new_mtimes.get(s) != mtimes.get(s)]
            if changed:
                print(f"\n  Detected {len(changed)} change(s):")
                results = compile_all(changed, jobs, run_validate, run_naga, verbose)
                print_summary(results)
                mtimes = new_mtimes
    except KeyboardInterrupt:
        print("\nWatch stopped.")


# --- Entry Point ---

def main():
    parser = argparse.ArgumentParser(description="WebGPU/wgpu-native GLSL -> SPIR-V shader compiler")
    parser.add_argument("--watch", "-w", action="store_true", help="Watch for changes and recompile")
    parser.add_argument("--jobs", "-j", type=int, default=4, help="Parallel compile jobs (default: 4)")
    parser.add_argument("--release", "-r", action="store_true", help="Release build (strip debug info)")
    parser.add_argument("--no-validate", action="store_true", help="Skip spirv-val validation pass")
    parser.add_argument("--no-naga", action="store_true",
                         help="Skip the naga/wgpu compatibility cross-check (on by default). "
                              "Requires `naga-cli` to actually run; skipped automatically if absent.")
    parser.add_argument("--quiet", "-q", action="store_true",
                         help="Suppress per-command output (verbose is on by default)")
    args = parser.parse_args()

    if args.release and "-g" in GLSLC_FLAGS:
        GLSLC_FLAGS.remove("-g")

    run_validate = not args.no_validate
    if run_validate and not tool_available(SPIRV_VAL_CMD):
        print(c(f"  Note: '{SPIRV_VAL_CMD}' not found on PATH, skipping validation pass.", YELLOW))
        run_validate = False

    run_naga = not args.no_naga
    if run_naga and not tool_available(NAGA_COMMAND):
        print(c(f"  Note: '{NAGA_COMMAND}' not found on PATH (install with `cargo install naga-cli`), "
                f"skipping wgpu compatibility check.", YELLOW))
        run_naga = False

    verbose = not args.quiet

    print(f"  Shader dir : {SHADER_ROOT_DIR}")
    print(f"  Output dir : {OUTPUT_DIR}")
    print(f"  Target     : Vulkan 1.2 / SPIR-V (GLSL 460)")
    print(f"  Mode       : {'release' if args.release else 'debug'}")
    print(f"  Validate   : {'on' if run_validate else 'off'}")
    print(f"  Naga check : {'on' if run_naga else 'off'}\n")

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    if args.watch:
        watch(args.jobs, run_validate, run_naga, verbose)
    else:
        shaders = collect_shaders()
        if not shaders:
            print("No shaders found.")
            return
        print(f"  Found {len(shaders)} shader(s)\n")
        results = compile_all(shaders, args.jobs, run_validate, run_naga, verbose)
        failed_count = print_summary(results)
        exit(0 if failed_count == 0 else 1)


if __name__ == "__main__":
    main()
