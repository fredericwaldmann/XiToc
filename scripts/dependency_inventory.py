#!/usr/bin/env python3
"""Regenerate XiToc's locked dependency inventory with Python 3.11+ and Cargo."""
import argparse
import collections
import csv
import json
import pathlib
import re
import subprocess
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
TARGETS = {
    "Linux": "x86_64-unknown-linux-gnu",
    "Windows": "x86_64-pc-windows-msvc",
    "macOS": "aarch64-apple-darwin",
    "WASM": "wasm32-unknown-unknown",
}
DIRECT = {
    "base64": ("Document and serialization", "Encode clipboard binary assets as Base64 strings in JSON", "src/asset.rs"),
    "cap-std": ("File management", "Access native files through workspace-scoped directory capabilities", "src/storage/native.rs"),
    "earcutr": ("GUI and rendering", "Triangulate vector ink contours and holes", "src/ink_mesh.rs"),
    "eframe": ("GUI and rendering", "Run the editor in a native window or browser canvas and store preferences", "src/main.rs"),
    "egui": ("GUI and rendering", "Provide editor controls, input events, layout, and drawing primitives", "src/canvas.rs"),
    "fontdb": ("Fonts and text", "Discover and load installed fonts for native rich text", "src/text_fonts.rs"),
    "i_overlay": ("GUI and rendering", "Union translucent ink outlines and derive an antialiasing fringe", "src/ink_mesh.rs"),
    "image": ("Image handling", "Decode PNG, JPEG and WebP and convert browser clipboard images", "src/image_import.rs"),
    "js-sys": ("WebAssembly", "Read JavaScript arrays and buffers and interact with browser state", "src/image_import.rs; src/app.rs"),
    "latex-rust": ("LaTeX", "Parse and lay out math formulas and emit SVG outlines", "src/formula_render.rs"),
    "lopdf": ("Document and serialization", "Parse PDF metadata and embedded XiToc payloads", "src/pdf.rs"),
    "pdf-writer": ("Document and serialization", "Construct valid blank PDFs with embedded XiToc metadata", "src/pdf.rs"),
    "resvg": ("LaTeX", "Rasterize formula SVGs into antialiased display textures", "src/formula_render.rs"),
    "rfd": ("File management", "Choose native workspace and file paths and browser import/export files", "src/storage; src/attachment_io.rs"),
    "serde": ("Document and serialization", "Serialize and deserialize document, clipboard, and preference models", "src/document.rs; src/asset.rs"),
    "serde_json": ("Document and serialization", "Read and write JSON with exact floating-point round trips", "src/pdf.rs; src/object_clipboard.rs"),
    "sha2": ("Document and serialization", "Hash original image and attachment bytes for stable asset IDs", "src/asset.rs"),
    "tempfile": ("File management", "Manage native attachment staging, atomic export, and test directories", "src/attachment_io.rs"),
    "unicode-segmentation": ("Fonts and text", "Find user-visible character boundaries for rich-text editing", "src/text.rs"),
    "uuid": ("Document and serialization", "Generate stable random document and object IDs across platforms", "src/document.rs; src/object.rs"),
    "wasm-bindgen": ("WebAssembly", "Bind browser entry points, pen events and clipboard handlers to Rust", "src/main.rs; src/web_pressure.rs"),
    "wasm-bindgen-futures": ("WebAssembly", "Bridge JavaScript promises to async Rust file and startup tasks", "src/storage/web.rs; src/main.rs"),
    "web-sys": ("WebAssembly", "Access DOM, file, clipboard, and drag-and-drop browser APIs", "src/image_import.rs; src/main.rs"),
}

PURPOSES = {
    "ahash": "Fast keyed hashing for GUI maps and caches",
    "ashpd": "Rust client for desktop portal APIs",
    "block2": "Rust support for Objective-C blocks (callback objects)",
    "bytemuck": "Checked conversion between plain data and bytes for graphics buffers",
    "cc": "Compile C or C++ sources from Rust build scripts",
    "cfg-if": "Select a source branch for compiler or target configuration",
    "dlib": "Load shared system libraries at runtime",
    "document-features": "Generate documentation from Cargo feature descriptions",
    "epaint": "Text layout and tessellation for egui drawing primitives",
    "getrandom": "Obtain random bytes from the operating system or browser",
    "glow": "Common API for native OpenGL and browser WebGL",
    "glutin": "Create native OpenGL contexts and surfaces",
    "harfrust": "Shape Unicode text into positioned glyphs",
    "itertools": "Common iterator operations beyond the standard library",
    "libc": "Rust declarations for C operating-system interfaces",
    "log": "Logging API shared by dependencies",
    "naga": "Parse, validate and translate GPU shaders for the optional WGPU graph",
    "pollster": "Run an async task to completion in a synchronous native API",
    "proc-macro2": "Token-stream types shared by Rust code generators",
    "quote": "Build Rust token streams from templates for procedural macros",
    "roxmltree": "Parse XML into a read-only tree for SVG and font configuration",
    "skrifa": "Read font metrics and outlines from OpenType fonts",
    "syn": "Parse Rust syntax for compile-time code generation",
    "tiny-skia": "Rasterize paths and blends for SVG output",
    "usvg": "Parse and normalize SVG into a renderable tree",
    "vello_common": "Shared vector drawing types and algorithms used by the GUI renderer",
    "vello_cpu": "CPU vector rendering used by epaint for glyph and outline work",
    "winit": "Manage native windows and dispatch operating-system input events",
    "x11rb": "Communicate with the X11 display server",
}
FONT = re.compile(r"font|glyph|skrifa|harfrust|unicode|kurbo|fontconfig")
IMAGE = re.compile(r"png|jpeg|webp|moxcms|pxfm|color_quant|image")
GRAPHICS = re.compile(r"^((wgpu|naga|ash|glow|glutin|vello|egui|eframe|epaint|winit|wayland|x11|smithay|objc2|metal)|raw-window)")
WEB = re.compile(r"^(wasm-bindgen|web-sys|js-sys|wasip|wit-bindgen)")
FILE = re.compile(r"^((cap-|fs-|tempfile|ambient-authority)|rfd|ashpd|pollster|zbus|io-lifetimes|open$)")


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True)


def metadata():
    return json.loads(run("cargo", "metadata", "--locked", "--offline", "--format-version", "1"))


def target_graph(target):
    rows = run("cargo", "tree", "--locked", "--offline", "--target", target,
               "--edges", "normal,build", "--no-dedupe", "--prefix", "depth",
               "--format", "{p}|{f}")
    features = collections.defaultdict(set)
    roots = collections.defaultdict(set)
    parents = collections.defaultdict(set)
    stack = []
    for row in rows.splitlines():
        match = re.match(r"^(\d+)(\S+) v([^ |]+)[^|]*\|(.*)$", row)
        if not match:
            raise ValueError(row)
        depth, name, version, enabled = match.groups()
        depth = int(depth)
        key = (name, version)
        stack[depth:] = [key]
        features[key].update(filter(None, enabled.split(",")))
        if depth:
            parents[key].add(stack[-2])
            roots[key].add(stack[1][0])
    return features, roots, parents


def history():
    seen = {}
    for commit in run("git", "log", "--reverse", "--topo-order", "--format=%h", "--", "Cargo.lock").splitlines():
        try:
            lock = tomllib.loads(run("git", "show", f"{commit}:Cargo.lock"))
        except subprocess.CalledProcessError:
            continue
        subject = run("git", "show", "-s", "--format=%s", commit).strip()
        for package in lock.get("package", []):
            seen.setdefault((package["name"], package["version"]), f"{commit} {subject}")
    return seen


def purpose(package):
    name = package["name"]
    if name in DIRECT:
        return DIRECT[name][1]
    if name in PURPOSES:
        return PURPOSES[name]
    if name.startswith("windows_"):
        return "Provide Windows API bindings or target-architecture import libraries"
    if name.startswith("objc2-"):
        return f"Expose Apple's {name.removeprefix('objc2-')} framework to Rust"
    if name.startswith("icu_"):
        prefix = "Provide generated Unicode data for " if name.endswith("_data") else "Implement Unicode algorithms or data structures for "
        return prefix + name.removeprefix("icu_").removesuffix("_data").replace("_", " ")
    description = package["description"]
    return " ".join(description.split()) if description else "Investigate purpose in upstream source"


def category(package, roots):
    name = package["name"]
    if name in DIRECT:
        return DIRECT[name][0]
    if any("proc-macro" in target["kind"] for target in package["targets"]):
        return "Development and testing / host code generation"
    for pattern, label in ((WEB, "WebAssembly"), (GRAPHICS, "GUI and rendering"),
                           (FONT, "Fonts and text"), (IMAGE, "Image handling"), (FILE, "File management")):
        if pattern.search(name):
            return label
    if name in {"cc", "pkg-config", "autocfg", "version_check", "rustversion", "find-msvc-tools",
                "jobserver", "shlex", "proc-macro2", "quote", "syn"}:
        return "Development and testing / build support"
    categories = {DIRECT[root][0] for root in roots if root in DIRECT}
    return next(iter(categories)) if len(categories) == 1 else "Other / shared infrastructure"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "docs/DEPENDENCY_INVENTORY.csv")
    args = parser.parse_args()
    data = metadata()
    locked = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    packages = {(p["name"], p["version"]): p for p in data["packages"]}
    locked_keys = {(p["name"], p["version"]) for p in locked}
    if len(locked_keys) != len(locked) or locked_keys != packages.keys():
        raise ValueError("Cargo.lock and metadata differ or have ambiguous package identities")
    root = data["resolve"]["root"]
    direct = {d["name"] for p in data["packages"] if p["id"] == root for d in p["dependencies"]}
    if direct != DIRECT.keys():
        raise ValueError(f"Review the direct dependency catalog: {direct ^ DIRECT.keys()}")
    ids = {p["id"]: (p["name"], p["version"]) for p in packages.values()}
    kinds = collections.defaultdict(set)
    resolved_parents = collections.defaultdict(set)
    children = collections.defaultdict(set)
    for node in data["resolve"]["nodes"]:
        for edge in node["deps"]:
            key = ids[edge["pkg"]]
            kinds[key].update(value["kind"] or "normal" for value in edge["dep_kinds"])
            resolved_parents[key].add(ids[node["id"]])
            children[ids[node["id"]]].add(key)
    locked_roots = collections.defaultdict(set)
    for dependency in children[ids[root]]:
        pending = [dependency]
        visited = set()
        while pending:
            key = pending.pop()
            if key in visited:
                continue
            visited.add(key)
            locked_roots[key].add(dependency[0])
            pending.extend(children[key])
    graphs = {label: target_graph(target)[0] for label, target in TARGETS.items()}
    all_features, all_roots, all_parents = target_graph("all")
    first_seen = history()
    duplicates = collections.Counter(name for name, _ in packages)
    fields = [
        "Name", "Version", "Purpose", "Direct/Transitive", "Introduced By",
        "Feature/Subsystem", "Dependency Kind", "Target Applicability", "Enabled Features",
        "Duplicate Version", "Recommendation", "Evidence or Notes",
    ]
    with args.output.open("w", encoding="utf-8", newline="") as output:
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow(fields)
        for key, package in sorted(packages.items()):
            name, version = key
            roots = all_roots[key] or locked_roots[key]
            targets = [label for label, graph in graphs.items() if key in graph]
            scope = ", ".join(targets)
            if not scope:
                scope = "Other Cargo targets" if key in all_features else "Lockfile only in audited configuration"
            descriptions = [
                f"{label}: {','.join(sorted(graph[key])) or '(none)'}"
                for label, graph in graphs.items() if key in graph
            ]
            if not descriptions and key in all_features:
                descriptions.append("all-target tree: " + (",".join(sorted(all_features[key])) or "(none)"))
            if not descriptions:
                descriptions.append("(inactive in normal/build tree)")
            tags = sorted(kinds[key])
            if package["id"] == root:
                tags.append("workspace application")
            if any("proc-macro" in target["kind"] for target in package["targets"]):
                tags.append("host procedural macro")
            if any("custom-build" in target["kind"] for target in package["targets"]):
                tags.append("has build script")
            parents = all_parents[key] or resolved_parents[key]
            note = ["Parents: " + ", ".join(f"{n} {v}" for n, v in sorted(parents))] if parents else []
            if key in first_seen:
                note.append("First lock entry: " + first_seen[key])
            if key not in all_features:
                note.append("Lockfile resolution path only")
            if key == ("sha2", "0.11.1"):
                note.append("Direct asset hashing aligned with lopdf in this audit")
            if name in DIRECT:
                note.append("XiToc usage: " + DIRECT[name][2])
            if package["repository"]:
                note.append("Upstream: " + package["repository"])
            review = duplicates[name] > 1 or not targets or not package["description"]
            recommendation = "Keep / review upstream alignment" if review else "Keep"
            if package["id"] == root:
                recommendation = "Workspace root"
            writer.writerow([
                name, version, "XiToc application" if package["id"] == root else purpose(package),
                "Workspace root" if package["id"] == root else ("Direct" if name in direct else "Transitive"),
                "XiToc" if package["id"] == root else ", ".join(sorted(roots)) or "See resolved parents",
                "Workspace" if package["id"] == root else category(package, roots),
                ", ".join(tags), scope, " | ".join(descriptions),
                "Yes" if duplicates[name] > 1 else "No", recommendation, "; ".join(note),
            ])


if __name__ == "__main__":
    main()
