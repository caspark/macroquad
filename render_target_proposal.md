# Render Target Texture Ownership Fix

## Context

Commit `975aa6a` ("fix: skip MSAA resolve for non-multisampled render targets") changed
the `sample_count` check in `render_target_ex()` from `!= 0` to `> 1`. This fixed a
catastrophic performance bug where every non-MSAA render target was needlessly creating
MSAA resolve infrastructure (resolve textures, resolve FBOs, full-framebuffer
`glBlitFramebuffer` on every `end_render_pass`), causing ~2000 unnecessary blits per
frame and dropping from 60 FPS to 3 FPS.

That fix is correct but exposes a latent texture ownership bug: **non-MSAA render target
textures can be silently destroyed**, causing them to read back as all zeros and appear
transparent when sampled in shaders.

A quick workaround was committed as `7c0e871` ("fix: prevent double-delete of non-MSAA
render target textures") which uses `Texture2D::unmanaged()` to sidestep the
double-delete. This proposal describes why that workaround is insufficient and proposes a
proper fix to replace it.

**All code references below describe the state after commit `975aa6a` but before commit
`7c0e871`** — i.e. the `> 1` fix is applied but no texture ownership fix is in place yet.

## Background: Texture Ownership in macroquad

macroquad has two layers of texture management:

**miniquad layer**: `TextureId` is an index into a `Vec<Texture>`. `delete_texture()`
calls `glDeleteTextures` / `glDeleteRenderbuffers` but does **not** remove the Vec entry
(it becomes stale — the GL name is freed but the Rust-side entry with its old params and
raw GL ID remains). `delete_render_pass()` deletes the FBO and then calls
`delete_texture()` on all entries in `color_textures` and `depth_texture`.

**macroquad layer**: `Texture2D` wraps a `TextureHandle` which comes in three flavors:

- `Managed(Arc<TextureSlotGuarded>)` — ref-counted. When the last `Arc` drops,
  `TextureSlotGuarded::drop` schedules the `TextureId` for deferred deletion.
  `garbage_collect()` (runs once per frame at end-of-frame) looks up the miniquad
  `TextureId` from the slot, calls `delete_texture()`, and removes the slot.
- `ManagedWeak(TextureSlotId)` — non-owning reference (created by `weak_clone()`). Does
  not prevent or trigger cleanup.
- `Unmanaged(TextureId)` — raw handle with no lifecycle management whatsoever.

The `Managed` path is the safe one — `Arc` ref-counting ensures the GL texture lives as
long as any owning `Texture2D` exists.

## How render_target_ex Works (After Commit 975aa6a)

`render_target_ex()` creates these GL resources:

1. **color_texture** — `new_render_texture(sample_count: N)`. For MSAA (N > 1), this is
   a GL renderbuffer. For non-MSAA (N ≤ 1), this is a regular GL texture.
2. **depth_texture** (optional) — `new_render_texture(format: Depth)`.
3. **color_resolve_texture** (MSAA only) — a regular GL texture that receives the
   resolved (downsampled) content via `glBlitFramebuffer` at each `end_render_pass`.

These are assembled into:

- **miniquad `RenderPass`** (`RenderPassInternal`) — owns the FBO. Stores:
  - `gl_fb: GLuint` — the main framebuffer object
  - `color_textures: Vec<TextureId>` — the FBO color attachments
  - `resolves: Option<Vec<(GLuint, TextureId)>>` — resolve FBOs and their target textures
  - `depth_texture: Option<TextureId>`
- **macroquad `RenderTarget`** — has:
  - `texture: Texture2D` — the user-facing texture to sample/read (stored via
    `store_texture()` → `Managed` handle)
  - `render_pass: RenderPass` — wraps the miniquad render pass in an `Arc`

The key variable is which texture becomes `RenderTarget.texture`:

| Path | `RenderTarget.texture` points to |
|---|---|
| MSAA (`sample_count > 1`) | `color_resolve_texture` (separate from FBO attachment) |
| Non-MSAA (`sample_count ≤ 1`) | `color_texture` (IS the FBO attachment) |

## The Double-Ownership Bug

### MSAA path — no conflict

| Resource | In render pass `color_textures`? | In macroquad `store_texture()`? |
|---|---|---|
| color_texture (renderbuffer) | ✅ yes | ✗ no |
| color_resolve_texture | ✗ no (only in `resolves`) | ✅ yes |
| depth_texture | ✅ yes | ✗ no |

Different textures in each system. `delete_render_pass` deletes the renderbuffer and
depth texture; `garbage_collect` deletes the resolve texture. No conflict.

### Non-MSAA path — DOUBLE OWNERSHIP

| Resource | In render pass `color_textures`? | In macroquad `store_texture()`? |
|---|---|---|
| color_texture (GL texture) | ✅ yes | ✅ **yes — SAME texture** |
| depth_texture | ✅ yes | ✗ no |

`color_texture` is registered in **both** cleanup systems.

## The Double-Delete Sequence

When a `RenderTarget` is dropped (e.g. when `SizedRenderTarget::get()` detects a size
change and replaces `self.built`):

1. Rust drops `RenderTarget` fields in declaration order: `texture: Texture2D` first,
   then `render_pass: RenderPass`.

2. **`Texture2D` drop** — the `Managed(Arc<TextureSlotGuarded>)` inner ref-count
   decreases. If this was the last owning `Arc`, `TextureSlotGuarded::drop` schedules the
   `TextureId` for deferred deletion (pushed onto `TexturesContext::removed`).

3. **`RenderPass` drop** — checks `Arc::strong_count(&self.render_pass) < 2`, then calls
   miniquad's `delete_render_pass()`. This iterates `color_textures` and **immediately**
   calls `glDeleteTextures()` on the color_texture's raw GL ID. The GL name is now freed.

4. Application creates a new render target. GL's `glGenTextures` may **reuse** the
   now-freed GL name for the new color texture. The new texture gets (say) GL ID 47 —
   the same ID the old texture had.

5. At end of frame, `garbage_collect()` runs. For each entry in `removed`, it looks up
   the miniquad `TextureId` in the `Textures` Vec. The entry is still there (since
   `delete_texture` in step 3 didn't remove it from the Vec). The entry still contains
   the old raw GL ID (47). `garbage_collect` calls `glDeleteTextures(47)` — **deleting
   the new texture**.

Result: the new render target's GL texture is destroyed. `get_texture_data()` returns all
zeros. Shader sampling returns transparent black.

### Confirmed via GL error checking

Adding `glCheckFramebufferStatus` and `glGetError` calls to miniquad's `read_pixels`
(which creates a temporary FBO and attaches the texture) confirmed:

- `GL_FRAMEBUFFER_INCOMPLETE_ATTACHMENT` (0x8CD6) — the texture is invalid/deleted
- `GL_INVALID_FRAMEBUFFER_OPERATION` (0x0506) — `glReadPixels` fails on incomplete FBO

### Why this only triggers on render target recreation

If a render target is created once and never dropped, neither cleanup path fires. The bug
requires drop + recreation, which happens when:

- A `SizedRenderTarget` detects a size change (common: first frame often renders at a
  small/default size, then immediately recreates at the actual window size)
- User explicitly drops and recreates a render target
- Any render target is dropped at all (GL ID recycling can affect unrelated textures
  created between the two deletes)

## Why the Unmanaged Workaround (Commit 7c0e871) Is Insufficient

The current workaround changes the non-MSAA path from:
```rust
Texture2D { texture: context.textures.store_texture(texture) }  // Managed
```
to:
```rust
Texture2D::unmanaged(texture)  // Unmanaged
```

This prevents the double-delete because `Unmanaged` handles don't schedule
garbage collection. Only `delete_render_pass` deletes the texture. However:

1. **No ref-counting safety**: `Unmanaged` `Texture2D` values can be freely
   `clone()`'d, but there's no mechanism to keep the GL texture alive.
   `RenderTarget.texture.clone()` creates another `Unmanaged` copy. When the
   `RenderTarget` drops, `delete_render_pass` deletes the GL texture, and all clones
   become dangling references to a deleted GL texture. With `Managed`, the `Arc` prevents
   this — the texture lives until the last owning reference drops.

2. **Semantic mismatch**: Every other texture created by macroquad's public API
   (`Texture2D::from_image`, `load_texture`, MSAA `render_target_ex`, etc.) uses
   `Managed` handles. Non-MSAA render target textures silently being `Unmanaged` violates
   the user's reasonable expectation that `Texture2D` values are safe to hold.

## Pre-existing Issues Discovered

While investigating, two additional bugs were found in miniquad's GL backend
`delete_render_pass()`:

1. **Resolve FBOs leak**: The `resolves` field stores `(GLuint, TextureId)` pairs where
   the `GLuint` is a GL framebuffer handle created via `glGenFramebuffers` in
   `new_render_pass_mrt`. `delete_render_pass` never calls `glDeleteFramebuffers` on
   these. Each MSAA render target that is dropped leaks one resolve FBO per color
   attachment.

2. **Backend inconsistency**: The Metal backend's `delete_render_pass` does NOT delete
   textures — it only releases the render pass descriptor. The GL backend does delete
   textures. The same macroquad code has different ownership semantics depending on
   platform.

## Proposed Fix

**Principle**: The render pass owns FBOs. Textures are owned by whoever created them.
This matches the Metal backend's existing behavior, fixes the double-delete, and fixes
the resolve FBO leak. It replaces the `unmanaged` workaround entirely.

### miniquad change: `delete_render_pass` in `src/graphics/gl.rs`

Stop deleting textures; start deleting resolve FBOs:

```rust
fn delete_render_pass(&mut self, render_pass: RenderPass) {
    let pass = self.passes.remove(render_pass.0);
    unsafe {
        glDeleteFramebuffers(1, &pass.gl_fb as *const _);
        if let Some(resolves) = &pass.resolves {
            for (resolve_fb, _) in resolves {
                glDeleteFramebuffers(1, resolve_fb as *const _);
            }
        }
    }
    // Textures NOT deleted — caller manages their lifetime.
    // This matches the Metal backend's behavior.
}
```

**Before → After**:

| Resource | Before | After |
|---|---|---|
| Main FBO (`gl_fb`) | ✅ deleted | ✅ deleted |
| Resolve FBOs (`resolves[].0`) | ❌ **leaked** | ✅ deleted |
| Color textures (`color_textures`) | deleted here | ✗ caller's responsibility |
| Resolve textures (`resolves[].1`) | ✗ not deleted | ✗ caller's responsibility |
| Depth texture (`depth_texture`) | deleted here | ✗ caller's responsibility |

### macroquad change: `RenderPass` struct and `render_target_ex` in `src/texture.rs`

Add a field to track textures that the render pass created but that no `Texture2D`
manages:

```rust
pub struct RenderPass {
    pub color_texture: Texture2D,
    pub depth_texture: Option<Texture2D>,
    pub(crate) render_pass: Arc<miniquad::RenderPass>,
    /// Textures created for the render pass internals (MSAA renderbuffers, depth
    /// attachments) that are not managed by any Texture2D. Explicitly deleted on drop.
    pub(crate) pass_owned_textures: Vec<miniquad::TextureId>,
}
```

Update `RenderPass::drop`:

```rust
impl Drop for RenderPass {
    fn drop(&mut self) {
        if Arc::strong_count(&self.render_pass) < 2 {
            let ctx = get_quad_context();
            ctx.delete_render_pass(*self.render_pass);
            for tex in &self.pass_owned_textures {
                ctx.delete_texture(*tex);
            }
        }
    }
}
```

Update `render_target_ex`:

```rust
pub fn render_target_ex(width: u32, height: u32, params: RenderTargetParams) -> RenderTarget {
    let context = get_context();

    let color_texture = get_quad_context().new_render_texture(/* ... same as before ... */);
    let depth_texture = /* ... same as before ... */;

    let render_pass;
    let texture;
    let mut pass_owned_textures = Vec::new();

    if params.sample_count > 1 {
        let color_resolve_texture = get_quad_context().new_render_texture(/* ... */);
        render_pass = get_quad_context().new_render_pass_mrt(
            &[color_texture],
            Some(&[color_resolve_texture]),
            depth_texture,
        );
        texture = color_resolve_texture;
        // MSAA renderbuffer: no Texture2D manages it, render pass must clean it up.
        pass_owned_textures.push(color_texture);
    } else {
        render_pass = get_quad_context().new_render_pass_mrt(
            &[color_texture],
            None,
            depth_texture,
        );
        texture = color_texture;
        // Non-MSAA: color_texture will be managed by Texture2D below.
    }

    // Depth texture is never exposed via Texture2D, so always pass-owned.
    if let Some(dt) = depth_texture {
        pass_owned_textures.push(dt);
    }

    // User-facing texture is always Managed — safe with Arc ref-counting.
    let texture = Texture2D {
        texture: context.textures.store_texture(texture),
    };

    let render_pass = RenderPass {
        color_texture: texture.clone(),
        depth_texture: None,
        render_pass: Arc::new(render_pass),
        pass_owned_textures,
    };
    RenderTarget {
        texture,
        render_pass,
    }
}
```

### Ownership After Fix

**MSAA path**:

| Resource | Owned by | Deleted when |
|---|---|---|
| color_texture (renderbuffer) | `pass_owned_textures` | `RenderPass::drop` |
| color_resolve_texture | `Texture2D` (Managed) | last `Arc` drops → `garbage_collect` |
| depth_texture | `pass_owned_textures` | `RenderPass::drop` |
| Main FBO | miniquad render pass | `delete_render_pass` |
| Resolve FBO(s) | miniquad render pass | `delete_render_pass` |

**Non-MSAA path**:

| Resource | Owned by | Deleted when |
|---|---|---|
| color_texture | `Texture2D` (Managed) | last `Arc` drops → `garbage_collect` |
| depth_texture | `pass_owned_textures` | `RenderPass::drop` |
| Main FBO | miniquad render pass | `delete_render_pass` |

Every resource has exactly one owner. No double-deletes. `Texture2D` clones are
ref-counted and safe to hold past the `RenderTarget`'s lifetime.

## What This Fixes

1. **The double-delete bug** — non-MSAA render target textures are no longer deleted by
   both `delete_render_pass` and `garbage_collect`.
2. **Resolve FBO leak** — MSAA render pass cleanup now deletes the resolve FBOs that were
   previously leaked.
3. **Backend consistency** — GL `delete_render_pass` now matches Metal's behavior of not
   deleting textures.
4. **Ref-counting safety** — all user-facing `Texture2D` values use `Managed` handles,
   so clones keep the GL texture alive. Replaces the `Unmanaged` workaround.

## Risk Assessment

- **External callers of `delete_render_pass`**: Only macroquad's `RenderPass::drop` calls
  it. No other callers exist in macroquad's codebase.
- **Existing MSAA behavior**: MSAA renderbuffers were previously deleted inside
  `delete_render_pass`; now deleted by `pass_owned_textures` in `RenderPass::drop`. Same
  call site, same timing — the `Drop` impl calls `delete_render_pass` then immediately
  iterates `pass_owned_textures`.
- **Existing depth texture behavior**: Same as MSAA — moved from `delete_render_pass` to
  `pass_owned_textures`. Same timing.
- **Resolve textures (MSAA)**: Were never deleted by `delete_render_pass` — the GL
  backend only iterated `color_textures` and `depth_texture`, not `resolves[].1`. Were
  and remain managed by `Texture2D`'s `garbage_collect`. No change.
- **Struct size**: `RenderPass` gains one `Vec<TextureId>`. Typically 0–2 entries. The
  `RenderPass` is behind an `Arc` so this doesn't affect `Clone` cost.
