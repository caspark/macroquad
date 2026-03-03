#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""Build the wgpu_triangle wasm example and serve it in the browser.

wgpu on wasm uses web-sys/js-sys, so the wasm module imports wasm-bindgen
glue functions (__wbg_*, __wbindgen_*).  We must run wasm-bindgen on the
compiled wasm and apply the macroquad compatibility patch before serving.

Flow (mirrors wizard-pixels task_web_build_impl):
  1. cargo build --target wasm32-unknown-unknown --example wgpu_triangle
  2. wasm-bindgen --target web --no-typescript  →  wgpu_triangle.js + wgpu_triangle_bg.wasm
  3. Patch wgpu_triangle.js so miniquad's load() can drive it
  4. Serve everything with correct MIME types
"""

import re
import subprocess
import shutil
import tempfile
import http.server
import threading
import webbrowser
import sys
from pathlib import Path

HERE = Path(__file__).parent

def run(cmd, **kwargs):
    print(f"+ {' '.join(str(c) for c in cmd)}")
    subprocess.run(cmd, check=True, **kwargs)

def apply_mq_wasm_bindgen_patch(js_path: Path) -> None:
    """Apply macroquad wasm-bindgen compatibility patch for wasm-bindgen >= 0.2.100+.

    wasm-bindgen 0.2.100+ generates:
      - 107x  `import * as importN from "env"` at the top (bare specifiers,
        rejected by browsers without an import map — must be removed)
      - __wbg_get_imports() returns
            { "./foo_bg.js": import0, "env": import1, … "env": import107 }
        where import0 contains the wbg-specific JS helpers and import1..N
        are the miniquad "env" functions (duplicate keys — only last survives)

    Patch goals:
      1. Strip all bare `import * as importN from "env"` lines.
      2. Export set_wasm so the HTML can hand wasm_exports to the wbg helpers.
      3. Make __wbg_get_imports() return just import0 (the wbg helpers) so the
         HTML can register them under the right namespace with miniquad_add_plugin.
      4. Make the async init() return those helpers instead of booting the wasm
         (miniquad's load() does the actual instantiation).
    """
    text = js_path.read_text(encoding="utf-8")

    # 1. Remove all  `import * as importN from "env"`  lines (numbered imports).
    text = re.sub(r'^import \* as import\d+ from "env"\n', "", text, flags=re.MULTILINE)
    # Also handle the old single-import format just in case.
    text = text.replace("import * as __wbg_star0 from 'env';", "")

    # 2. Export set_wasm alongside the wasm variable declaration.
    text = re.sub(
        r"\blet wasmModule, wasm;",
        "let wasmModule, wasm; export const set_wasm = (w) => wasm = w;",
        text,
        count=1,
    )
    # Old format fallback.
    text = re.sub(
        r"\blet wasm;",
        "let wasm; export const set_wasm = (w) => wasm = w;",
        text,
        count=1,
    )

    # 3. Make __wbg_get_imports() return just import0 (wbg helpers).
    #    The generated return block looks like:
    #        return {
    #            __proto__: null,
    #            "./wgpu_triangle_bg.js": import0,
    #            "env": import1,
    #            ...
    #        };
    #    Replace that whole block with `return import0;`.
    text = re.sub(
        r'return \{\s*__proto__: null,\s*"[^"]+": import0,[\s\S]*?\};\n\}',
        "return import0;\n}",
        text,
        count=1,
    )
    # Old format fallback.
    text = text.replace("imports['env'] = __wbg_star0;", "return imports.wbg;")

    # 4. Make async init() return the helpers instead of booting the wasm.
    #    Only patch the last occurrence (inside __wbg_init, not initSync).
    old = "const imports = __wbg_get_imports();"
    new = "return __wbg_get_imports();"
    # Replace last occurrence only.
    idx = text.rfind(old)
    if idx != -1:
        text = text[:idx] + new + text[idx + len(old):]
    # Old format fallback (replace all).
    else:
        text = text.replace(old, new)

    js_path.write_text(text, encoding="utf-8")

INDEX_HTML = """\
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>wgpu triangle</title>
  <style>
    html, body, canvas {
      margin: 0; padding: 0;
      width: 100%; height: 100%;
      overflow: hidden; position: absolute;
      background: black; z-index: 0;
    }
  </style>
</head>
<body>
  <canvas id="glcanvas" tabindex="1"></canvas>
  <script src="mq_js_bundle.js"></script>
  <script type="module">
    import init, { set_wasm } from "./wgpu_triangle.js";
    window.addEventListener("load", async () => {
      // init() now returns the wbg helper object (patched to not boot the wasm).
      let wbg = await init();
      miniquad_add_plugin({
        // wasm-bindgen >= 0.2.100 imports its helpers under "./wgpu_triangle_bg.js"
        register_plugin: (a) => (a["./wgpu_triangle_bg.js"] = wbg),
        on_init: () => set_wasm(wasm_exports),
        version: "0.0.1",
        name: "wbg",
      });
      load("./wgpu_triangle_bg.wasm");
    });
  </script>
</body>
</html>
"""

def main():
    release = "--release" in sys.argv

    # 1. Build
    build_cmd = [
        "cargo", "build",
        "--target", "wasm32-unknown-unknown",
        "--example", "wgpu_triangle",
    ]
    if release:
        build_cmd.append("--release")
    run(build_cmd, cwd=HERE)

    profile = "release" if release else "debug"
    wasm_src = (
        HERE / "target" / "wasm32-unknown-unknown" / profile
        / "examples" / "wgpu_triangle.wasm"
    )

    # 2. Stage into a temp dir
    serve_dir = Path(tempfile.mkdtemp(prefix="wgpu-web-"))
    try:
        # Run wasm-bindgen to generate JS glue + patched wasm
        run([
            "wasm-bindgen", str(wasm_src),
            "--out-dir", str(serve_dir),
            "--target", "web",
            "--no-typescript",
        ])

        # 3. Apply macroquad compatibility patch to the generated JS
        js_file = serve_dir / "wgpu_triangle.js"
        apply_mq_wasm_bindgen_patch(js_file)

        # Copy miniquad JS bundle and write index.html
        shutil.copy(HERE / "js" / "mq_js_bundle.js", serve_dir / "mq_js_bundle.js")
        (serve_dir / "index.html").write_text(INDEX_HTML, encoding="utf-8")

        print(f"\nServing from {serve_dir}")

        # Find a free port
        import socket
        with socket.socket() as s:
            s.bind(("", 0))
            port = s.getsockname()[1]

        url = f"http://localhost:{port}"

        import functools
        handler = functools.partial(
            http.server.SimpleHTTPRequestHandler,
            directory=str(serve_dir),
        )
        httpd = http.server.HTTPServer(("", port), handler)
        http.server.SimpleHTTPRequestHandler.extensions_map[".wasm"] = "application/wasm"

        print(f"Listening on {url}  (Ctrl-C to stop)\n")
        threading.Timer(0.3, lambda: webbrowser.open(url)).start()

        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nStopped.")
    finally:
        shutil.rmtree(serve_dir, ignore_errors=True)

if __name__ == "__main__":
    main()
