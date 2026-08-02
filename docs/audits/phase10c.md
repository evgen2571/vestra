# Phase 10C — memory and copy optimization

## Result

The prepared CPU renderer already had a fixed three-surface RGBA8 ping-pong
workspace. This phase makes that ownership measurable, fixes the static cache
branch to use the same direct basic-colour path as dynamic rendering, and
removes its cache-publication clone: a completed static surface is moved into
the immutable cache and replaced in the mutable pool. Dynamic layers and post
effects reuse the same three surfaces. The pool is bounded by CPU capacity one
and cannot grow with frame count; it retains exactly three full-canvas surfaces.

Before this change, publishing a cacheable static layer cloned its full RGBA
surface. The current path has zero such clones: it retains the completed image
as `Arc<RgbaImage>` and installs one same-sized replacement in the pool. This
preserves cache immutability and avoids the full-frame copy. A cache-budget
bypass still renders correctly; the transferred image is dropped after
composition rather than retained.

Completed CPU frames remain independently owned `Vec<u8>` values. They cannot
return to the renderer pool because the completion ordering queue and public
`FrameSink` can retain them. FFmpeg synchronously writes `&frame.rgba`, so no
additional renderer-to-sink clone exists.

WGPU already creates its complete mutable working set at preparation: Canvas A,
Canvas B, Layer, optional Effect A/B, and optional auxiliary texture. Its
compatibility key is the immutable prepared canvas dimensions, `Rgba8Unorm`,
2D dimension, sample count one, and working texture usage. It is bounded by
that fixed set; queue ordering places a frame's final copy to its distinct
readback slot before a later command buffer reuses a working texture. Static
cache textures remain separate immutable sampled resources. Texture views are
stored with their textures.

Mapped padded WGPU rows are copied directly into one final tight RGBA `Vec`.
The conversion has one final allocation and one valid-row copy per row; aligned
and unaligned row layouts are covered by existing adapter-independent tests.

## Metrics

Internal renderer metrics now report CPU output allocations, CPU scratch
allocations/reuses/retained bytes, explicit CPU post-effect copy bytes, WGPU
working-texture allocations/reuses/retained logical bytes, and final readback
allocation/repack bytes. These are logical renderer-owned counters; they do
not claim allocator, FFmpeg, or driver memory traffic.

The focused 100-frame 4×4 dynamic Gaussian fixture reports 100 final-output
allocations, three scratch allocations, 100 scratch reuses, three retained
buffers, and 192 retained scratch bytes. The corresponding pre-existing
workspace allocation count was also three; the Phase 10C gain here is explicit
instrumentation and removal of static-cache cloning, not a second pool layered
over the existing one.

The WGPU working set is already prepared once, so its allocation counter equals
the compatible working-texture count and its reuse counter increases on later
successful submissions. Logical bytes are `width × height × 4` per retained
`Rgba8Unorm` working texture. Runtime adapter tests returned early in this
environment because no compatible adapter was available; adapter-independent
frame-plan, resource-count, and padded-row tests ran.

## Deliberately retained copies

- CPU completed-frame allocation: required independent ownership.
- CPU post-effect canvas → scratch → completed canvas copies: required because
  the public completed canvas must remain independently owned while effects use
  source and target surfaces.
- WGPU mapped rows → final tight RGBA: required CPU-accessible ownership.

Phase 10D benchmarking is deferred.

## Verification

- `cargo fmt --all -- --check`, `cargo check --workspace`, strict workspace
  Clippy, and `cargo test --workspace` passed.
- Focused CPU tests cover static-cache versus dynamic pixel parity for a fused
  basic-colour chain and the 100-frame scratch-reuse bound. Existing WGPU
  readback tests cover aligned and unaligned rows.
- Schema validation, `maturin develop --release`, `python -m pytest
  python-tests` (259 passed, 4 adapter-gated runtime early returns), mypy, and
  stubtest passed on CPython 3.13.5.
- `maturin build --release` passed. An isolated wheel imported
  `video_editor` and `video_editor.authoring`, validated a public project,
  prepared CPU rendering, and produced identical bytes on two renders.
