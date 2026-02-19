# HIGH

## miniquad dependency delivery mechanism unspecified

The proposal modifies `delete_render_pass` in miniquad's `src/graphics/gl.rs`, but miniquad is consumed as a crate dependency pinned at `=0.4.8`. The proposal never says how this change gets applied.

- A local fork exists at `../miniquad` with 3 commits on top of upstream `0.4.8` (RGBA16F, right-click context menu, canvas focus). The `[patch.crates-io]` line in `Cargo.toml` is commented out but ready.
- The proposal should specify: uncomment the `miniquad = { path = '../miniquad' }` patch, apply the `delete_render_pass` change to the local fork.
- Without this, the proposal is not actionable — someone could misread it as requiring an upstream PR and block on that.

# MEDIUM

## MSAA + depth render targets panic in miniquad (pre-existing, but assumed working)

The proposal's MSAA ownership table lists `depth_texture` in `pass_owned_textures`, implying the MSAA + depth path works. It does not.

In `new_render_pass_mrt` (gl.rs:1098), when `sample_count > 1` and a depth texture is provided:

```rust
if texture.params.sample_count > 1 {
    let raw = texture.raw.texture().unwrap();  // <-- panics
    glFramebufferRenderbuffer(...);
}
```

The depth texture was created with `sample_count > 1` via `new_render_texture`, making it a `Renderbuffer`. But `.texture()` returns `None` for renderbuffers, so `.unwrap()` panics. This means `render_target_ex` with `sample_count > 1` AND `depth: true` panics today.

- The proposal doesn't introduce this bug, and fixing it isn't required by its goals.
- The non-MSAA + depth path is unaffected (sample_count ≤ 1, depth texture is a regular GL texture, `.texture().unwrap()` succeeds).
- However, the MSAA ownership table in the proposal should note that the MSAA + depth path is currently broken upstream and untested. As written, it implies this is a working path that the proposal correctly handles.

# LOW

## `depth_texture` field on `RenderPass` always `None`

The proposal sets `depth_texture: None` in the `RenderPass` constructor, even when a depth attachment was created. Users accessing `render_pass.depth_texture` get `None` regardless of `params.depth`. This is pre-existing (the current code does the same on line 475), not introduced by the proposal.

The proposal could store a `Managed` Texture2D for the depth texture too (rather than putting it in `pass_owned_textures`), which would make the public field accurate. But this would be scope creep — the proposal's stated goal is fixing the double-delete, not improving the depth texture API.

## Proposal references `SizedRenderTarget` which doesn't exist in macroquad

The "Why this only triggers on render target recreation" section references `SizedRenderTarget::get()` as the trigger for the bug. This type doesn't exist in macroquad — it's presumably downstream user code. The reference is used illustratively to explain a recreation scenario, so this doesn't affect the proposal's correctness, but it may confuse readers who search for the type.
