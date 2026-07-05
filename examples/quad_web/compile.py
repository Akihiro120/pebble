#!/usr/bin/env python3
"""
build.py — build a Pebble web example to WASM and serve it locally.

Usage:
    python build.py [crate_name] [--release] [--port 8000] [--no-serve]

Examples:
    python build.py draw_quad_web
    python build.py draw_quad_web --release --port 3000
    python build.py draw_quad_web --no-serve      # just build, don't launch a server

Requirements (checked automatically, with a helpful message if missing):
    - rustup target: wasm32-unknown-unknown
    - wasm-bindgen-cli (must match the `wasm-bindgen` crate version in Cargo.toml)

Run this script from inside the example crate's own directory
(the one containing that crate's Cargo.toml with `crate-type = ["cdylib"]`).
"""

import argparse
import http.server
import shutil
import socketserver
import subprocess
import sys
from pathlib import Path


def run(cmd, **kwargs):
    print(f"$ {' '.join(cmd)}")
    result = subprocess.run(cmd, **kwargs)
    if result.returncode != 0:
        sys.exit(f"Command failed ({result.returncode}): {' '.join(cmd)}")
    return result


def check_tool(name, help_text):
    if shutil.which(name) is None:
        sys.exit(f"error: `{name}` not found on PATH.\n{help_text}")


def check_wasm_target():
    result = subprocess.run(
        ["rustup", "target", "list", "--installed"],
        capture_output=True, text=True,
    )
    if "wasm32-unknown-unknown" not in result.stdout:
        sys.exit(
            "error: wasm32-unknown-unknown target is not installed.\n"
            "Fix: rustup target add wasm32-unknown-unknown"
        )


def find_crate_name(cargo_toml: Path) -> str:
    for line in cargo_toml.read_text().splitlines():
        line = line.strip()
        if line.startswith("name") and "=" in line:
            return line.split("=", 1)[1].strip().strip('"')
    sys.exit(f"error: could not find `name = \"...\"` in {cargo_toml}")


def main():
    parser = argparse.ArgumentParser(description="Build and serve a Pebble WASM example.")
    parser.add_argument("crate_name", nargs="?", default=None,
                         help="Crate name to build (defaults to the crate in the current directory)")
    parser.add_argument("--release", action="store_true", help="Build in release mode")
    parser.add_argument("--port", type=int, default=8000, help="Local server port (default 8000)")
    parser.add_argument("--no-serve", action="store_true", help="Build only, do not launch a server")
    parser.add_argument("--out-dir", default="pkg", help="Output directory for wasm-bindgen artifacts")
    args = parser.parse_args()

    cwd = Path.cwd()
    cargo_toml = cwd / "Cargo.toml"
    if not cargo_toml.exists():
        sys.exit(f"error: no Cargo.toml found in {cwd}\n"
                  f"Run this script from inside your web example crate's directory.")

    crate_name = args.crate_name or find_crate_name(cargo_toml)

    check_tool("cargo", "Install Rust: https://rustup.rs")
    check_tool("wasm-bindgen", "Install: cargo install wasm-bindgen-cli\n"
                                "(version must match the `wasm-bindgen` crate version in Cargo.toml)")
    check_wasm_target()

    # 1. cargo build --target wasm32-unknown-unknown
    build_cmd = ["cargo", "build", "--target", "wasm32-unknown-unknown"]
    if args.release:
        build_cmd.append("--release")
    run(build_cmd)

    profile_dir = "release" if args.release else "debug"
    wasm_path = cwd / "target" / "wasm32-unknown-unknown" / profile_dir / f"{crate_name}.wasm"
    if not wasm_path.exists():
        sys.exit(f"error: expected wasm artifact not found at {wasm_path}\n"
                  f"Check that [lib] crate-type includes \"cdylib\" and the crate name matches.")

    # 2. wasm-bindgen --out-dir pkg --target web <wasm file>
    out_dir = cwd / args.out_dir
    run([
        "wasm-bindgen",
        "--out-dir", str(out_dir),
        "--target", "web",
        "--no-typescript",
        str(wasm_path),
    ])

    print(f"\nBuild complete. Output in: {out_dir}")

    # 3. copy index.html next to pkg/ if it doesn't already exist here
    index_src = Path(__file__).parent / "index.html"
    index_dst = cwd / "index.html"
    if index_src.exists() and not index_dst.exists():
        shutil.copy(index_src, index_dst)
        print(f"Copied starter index.html to {index_dst}")

    if args.no_serve:
        return

    # 4. serve the current directory
    class QuietHandler(http.server.SimpleHTTPRequestHandler):
        def end_headers(self):
            # WASM needs the correct MIME type; most servers get this right by
            # default, but set it explicitly to avoid environment-specific issues.
            self.send_header("Cache-Control", "no-store")
            super().end_headers()

        def guess_type(self, path):
            if path.endswith(".wasm"):
                return "application/wasm"
            return super().guess_type(path)

    with socketserver.TCPServer(("", args.port), QuietHandler) as httpd:
        url = f"http://localhost:{args.port}"
        print(f"\nServing {cwd} at {url}")
        print("Press Ctrl+C to stop.\n")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nStopped.")


if __name__ == "__main__":
    main()
