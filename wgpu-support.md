# wgpu Support for macroquad/miniquad

## Goal

Use wgpu for rendering while keeping macroquad/miniquad for windowing, input, and other abstractions. macroquad's own GL rendering may break (crash/assert is fine, UB is not). Ideally toggled at runtime; feature flag acceptable.

## Architecture Summary

### How miniquad works today

miniquad owns the full lifecycle: window creation → GL context creation → event loop → draw/present. Per platform:

| Platform | Window handle | GL context | Event loop |
|----------|--------------|------------|------------|
| Linux X11 | `Display*` + `Window` (Xlib) | GLX or EGL | `XNextEvent` polling loop |
| Linux Wayland | `wl_display*` + `wl_surface*` | EGL on `wl_egl_window` | `wl_display_dispatch` loop |
| Windows | `HWND` | WGL (via opengl32.dll) | `PeekMessage`/`DispatchMessage` loop |
| macOS | `NSWindow` + `NSView` | NSOpenGLContext or Metal layer | `NSApp nextEvent` loop |
| Web/WASM | HTML `<canvas id="glcanvas">` | WebGL1/2 via `canvas.getContext()` | `requestAnimationFrame` |

Key problem: these handles are **local variables** inside each platform's `run()` function. They are never exposed to user code. The only exception is macOS where `NativeDisplayData` stores `view: ObjcId`.

### How wgpu creates surfaces

wgpu needs a `raw_window_handle::RawWindowHandle` + `RawDisplayHandle` (from the `raw-window-handle` v0.6 crate). These are simple structs containing raw pointers/IDs:

| Platform | RawDisplayHandle needs | RawWindowHandle needs |
|----------|----------------------|---------------------|
| Linux X11 | `XlibDisplayHandle { display: *Display, screen: i32 }` | `XlibWindowHandle { window: c_ulong, visual_id: c_ulong }` |
| Linux Wayland | `WaylandDisplayHandle { display: *wl_display }` | `WaylandWindowHandle { surface: *wl_surface }` |
| Windows | `WindowsDisplayHandle {}` (empty) | `Win32WindowHandle { hwnd: NonZeroIsize, hinstance: Option<NonZeroIsize> }` |
| macOS | `AppKitDisplayHandle {}` (empty) | `AppKitWindowHandle { ns_view: NonNull<c_void> }` |
| Web | `WebDisplayHandle {}` (empty) | `WebCanvasWindowHandle { obj: NonNull<c_void> }` (pointer to JsValue of canvas) |

wgpu then calls `Instance::create_surface_unsafe(SurfaceTargetUnsafe::RawHandle { ... })` to create a GPU surface. It handles Vulkan/Metal/DX12/GL backend selection internally.

### The GL context conflict

When using wgpu with Vulkan/Metal/DX12, there is **no conflict** with miniquad's GL context — they use different APIs on the same window. The GL context can sit unused.

On **web** there IS a conflict: a single canvas can only have one context. If miniquad calls `canvas.getContext("webgl2")`, wgpu cannot later call `canvas.getContext("webgl2")` (returns the same context) or `canvas.getContext("webgpu")` (returns null — different context type). So for web + wgpu either:
- Skip miniquad's `init_webgl()` call and let wgpu create its own context on the canvas
- Use a separate canvas element for wgpu
- Use wgpu's WebGPU backend (not WebGL), which uses `navigator.gpu` and a different canvas context type — but then miniquad's canvas already has a webgl context and we're stuck

## Approach: Expose raw window handles from miniquad

### What needs to change in miniquad

**Core change**: Store raw window/display handles in `NativeDisplayData` and expose them publicly. This is the minimal, least-invasive patch.

```rust
// In native.rs - add to NativeDisplayData:
pub struct NativeDisplayData {
    // ... existing fields ...
    
    /// Raw window handle for wgpu/raw-window-handle interop.
    /// Set by each platform backend after window creation.
    pub raw_window_handle: Option<RawWindowHandleData>,
    pub raw_display_handle: Option<RawDisplayHandleData>,
}

/// Platform-agnostic storage for raw handles (avoids raw-window-handle dep in core)
pub enum RawWindowHandleData {
    #[cfg(target_os = "linux")]
    Xlib { window: u64, visual_id: u64 },
    #[cfg(target_os = "linux")]  
    Wayland { surface: *mut std::ffi::c_void },
    #[cfg(target_os = "windows")]
    Win32 { hwnd: isize, hinstance: isize },
    #[cfg(target_vendor = "apple")]
    AppKit { ns_view: *mut std::ffi::c_void },
    #[cfg(target_arch = "wasm32")]
    WebCanvas { canvas_id: &'static str },
}
// (similar for RawDisplayHandleData)
```

Each platform backend sets these right after creating the window:

- **X11 GLX path** (`glx_main_loop`): After `create_window()` returns, store `display.window` and `display.display`
- **X11 EGL path** (`egl_main_loop`): Same — window and display pointers available after `create_window()`
- **Wayland** (`run`): After `wl_compositor_create_surface()`, store `surface` and `display` pointers
- **Windows** (`run`): After `CreateWindowExW()` returns `hwnd`, store it  
- **macOS** (`run`): Already stores `view` in `NativeDisplayData`; just need to package it
- **WASM** (`run`): Store canvas element ID (actual DOM access happens in JS)

### What needs to change in miniquad's public API

Add to `miniquad::window`:
```rust
/// Returns raw window handles for wgpu interop.
/// Returns None if the backend doesn't support this yet.
pub fn raw_window_handle() -> Option<RawWindowHandleData> { ... }
pub fn raw_display_handle() -> Option<RawDisplayHandleData> { ... }
```

### Implementing HasWindowHandle / HasDisplayHandle

Two options for where the `raw-window-handle` trait impls live:

**Option A: In miniquad itself** — Add `raw-window-handle` as an optional dependency of miniquad, gated behind a feature flag. Implement `HasWindowHandle` and `HasDisplayHandle` on a wrapper struct. This is cleanest for consumers.

**Option B: In macroquad or user code** — miniquad exposes raw pointers/IDs. A thin adapter struct in macroquad (or a separate crate) converts them to `raw-window-handle` types. Keeps miniquad's dependency footprint minimal.

### What needs to change in macroquad

macroquad needs to:
1. Provide a way for user code to get the wgpu surface/device/queue (or the raw handles to create one)
2. Optionally skip creating its own `GlContext` when wgpu mode is active (to avoid wasted work, not strictly required on desktop)

Minimal macroquad change: expose a function that returns an object implementing `HasWindowHandle + HasDisplayHandle`. User code then does:

```rust
let window_handle = macroquad::window::raw_window_handle();
let instance = wgpu::Instance::new(&Default::default());
let surface = unsafe { instance.create_surface_unsafe(...) };
// ... create adapter, device, queue, render loop ...
```

## Platform-Specific Details

### Desktop (Linux, Windows, macOS)

**No GL conflict.** wgpu will use Vulkan (Linux/Windows) or Metal (macOS) by default. The existing GL context just sits unused. miniquad's `commit_frame()` / `swap_buffers()` calls will swap an empty/stale GL framebuffer — harmless.

**The GL context wastes some GPU memory** (~few MB) but causes no correctness issues. To fully avoid it, add a `no_gl_context` flag to `Conf::Platform` that skips GL context creation. But this is optional and can be deferred.

### Web/WASM

**This is the hard case.** Options:

1. **wgpu WebGPU backend on a separate canvas**: Create a second `<canvas>` element, let wgpu use it with WebGPU. miniquad's canvas with WebGL handles input events. Doable but awkward (two canvases, z-ordering issues).

2. **Skip miniquad's `init_webgl()`, let wgpu own the canvas**: Add a flag so miniquad doesn't call `init_webgl()` in its JS glue. wgpu then calls `getContext("webgpu")` or `getContext("webgl2")` on the same canvas. miniquad still handles input events from the canvas (those don't depend on the GL context). This is the cleanest approach but requires changing the JS glue to be conditional.

3. **Drop web wgpu support**: Keep miniquad's normal WebGL path for web. Only enable wgpu on desktop. Simplest option.

**Recommendation**: Start with option 3 (desktop-only wgpu). Web support can be added later with option 2 once the desktop path is proven.

## Implementation Plan

### Phase 1: miniquad — expose raw handles + skip_graphics_context

1. Add `raw-window-handle = { version = "0.6", optional = true }` dep, gated behind `rwh-06` feature
2. Add `skip_graphics_context: bool` to `Conf::Platform` (default `false`)
3. Store raw window/display handles in `NativeDisplayData` from each platform backend after window creation (6 points: x11_glx, x11_egl, wayland, windows, macos, wasm)
4. Add `miniquad::window::raw_window_handle()` / `raw_display_handle()` public accessors
5. Implement `HasWindowHandle` + `HasDisplayHandle` on a wrapper struct (behind `rwh-06` feature)
6. When `skip_graphics_context` is true:
   - **X11 GLX/EGL**: Create window, skip GL context creation, skip `swap_buffers` in event loop, skip `gl::load_gl_funcs`
   - **Wayland**: Create `wl_surface` + `wl_egl_window`, skip EGL context, skip `eglSwapBuffers`
   - **Windows**: Create `HWND`, skip WGL context creation, skip `SwapBuffers`
   - **macOS**: Create `NSWindow` + `NSView` (plain view, not OpenGL or Metal view), skip `gl_context`/timer setup
   - **WASM**: Skip `init_webgl()` in JS glue, still set up canvas for input events
7. `new_rendering_backend()` panics with clear message when `skip_graphics_context` is true

### Phase 2: macroquad integration

1. Add `raw-window-handle` as optional dep behind a `wgpu-compat` feature flag (enables miniquad's `rwh-06`)
2. Expose `macroquad::window::raw_window_handle()` that returns an object implementing `HasWindowHandle + HasDisplayHandle`
3. When `skip_graphics_context` is active, macroquad's `Context::new()` skips creating `QuadGl`, `UiContext`, etc. (they all need a `RenderingBackend` which won't exist)
4. User code creates wgpu Instance/Surface/Device/Queue and renders in the macroquad `loop { ... next_frame().await }` loop

## Decisions Made

1. **`raw-window-handle` trait impls live in miniquad** — behind an optional `rwh-06` feature flag. The crate is tiny/no_std/zero-deps and is the universal standard. Any miniquad user benefits, not just macroquad.
2. **Skip GL context creation via `Conf::Platform::skip_graphics_context: bool`** — When set, platform backends create the window but skip GL/Metal context creation and `swap_buffers`/`commit_frame`. `new_rendering_backend()` returns a stub that panics. Requires auditing each platform event loop for GL calls.
3. **Web included from day one** — `skip_graphics_context` on WASM skips `init_webgl()` in the JS glue, leaving the canvas free for wgpu to call `getContext("webgpu")` or `getContext("webgl2")`. The canvas element is exposed via the same raw-window-handle mechanism as desktop handles.

## Current Implementation Status

### ✅ Completed (Desktop - Linux verified)

**miniquad patches (`~/srb/miniquad`):**
- `Cargo.toml`: Added `raw-window-handle` optional dep behind `rwh-06` feature
- `src/conf.rs`: Added `skip_graphics_context: bool` to `Platform`
- `src/native.rs`: Added `RawWindowHandleData`, `RawDisplayHandleData` enums; added fields to `NativeDisplayData`
- `src/lib.rs`: Added `MiniquadWindow` struct implementing `HasWindowHandle` + `HasDisplayHandle`; added `window::raw_window_handle()` and `window::raw_display_handle()` accessors
- `src/native/linux_x11.rs`: Added `no_gl_main_loop()` for skip_graphics_context; set raw handles in all three paths (no_gl, glx, egl)
- `src/native/linux_wayland.rs`: Skip EGL when skip_graphics_context; set raw handles
- `src/native/windows.rs`: Skip WGL context and SwapBuffers when skip_graphics_context; set raw handles
- `src/native/macos.rs`: Create plain NSView when skip_graphics_context; skip GL/Metal context; set raw handles

**macroquad patches (`~/srb/macroquad`):**
- `Cargo.toml`: Enabled `rwh-06` feature on miniquad; added `wgpu`, `pollster`, `raw-window-handle`, `image` as dev-deps; patched miniquad to local fork
- `examples/wgpu_triangle.rs`: Working example that renders an RGB triangle via wgpu, with screenshot capture support

### 🔲 Not yet done
- Web/WASM support (JS glue changes for conditional `init_webgl()`)
- Testing on Windows and macOS (code written but not tested on those platforms)

## Risk Assessment

- **Low risk**: Exposing handles from miniquad. Additive, no existing behavior changes.
- **Medium risk**: `skip_graphics_context` on desktop. Each platform event loop currently assumes GL is available — need to audit for unconditional GL calls (`swap_buffers`, `glFlush`, `make_current`, etc.) and guard them. The macOS path is most complex (two view types, timer-based redraw).
- **Medium risk**: `skip_graphics_context` on WASM. JS glue needs conditional `init_webgl()`. All the GL wrapper functions in `gl.js` will be undefined/null — must ensure nothing in miniquad's WASM event handling path calls them. Input event handlers are pure DOM and should be fine.
- **Low risk**: wgpu surface creation from raw handles. This is exactly what `raw-window-handle` was designed for and what winit/SDL/every other windowing lib does.
