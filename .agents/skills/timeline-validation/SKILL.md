---
name: timeline-validation
description: "Use when changing or diagnosing Vestra duration resolution, composition or source timing, frame boundaries, transitions, timeline validation or media-dependent preflight."
---

# Validate timeline meaning across public surfaces

Read [project-to-plan timing](../../../docs/development/architecture/project-to-plan.md#time-domains-and-frame-boundaries)
and the [timeline concepts](../../../docs/concepts/timeline-and-time.md).
Use [testing](../../../docs/development/testing.md#timeline-and-media-checks)
for existing regression seams.

1. Write the case in project, composition-local, clip-local, source-media and
   frame coordinates. Keep nested starts local; distinguish authored clip end,
   effective clipped end, source EOF and the output's exclusive end.
2. Use the existing checked timeline/frame helpers with rational FPS. Test
   the first/last frame and timestamps on either side of boundaries, including
   `30000/1001`, zero/invalid values and overflow. Preserve each helper's
   rounding contract instead of accumulating floating-point frame steps.
3. Compare pure semantic validation with media-dependent SDK preflight,
   resolved inspection, compilation and prepared frame access. Pure validation
   must not probe assets. Preserve automatic structural duration, explicit
   clipping warnings and composition-local transitions; check current known
   validation inconsistencies before treating desired behavior as implemented.
4. Exercise nested groups, late audio, muted audio, global effect keyframes,
   playback rate/source offsets, transition endpoints and zero-frame clips.
   Test source exhaustion using generated timestamped media, not nominal FPS.
5. Assert diagnostics identify the failing canonical field/owner, and cover
   Rust/Python/JSON/CLI callers using
   [api-contract-sync](../api-contract-sync/SKILL.md) when a contract changes.

Current modes are automatic and explicit. Minimum duration, inferred child-group
duration, fill-to-end intent and selected-range video output are proposals, not
existing contracts. Keep finite caller/resource and representability checks;
the default timeline caps still exist until their implementation changes.
Report the reproducer, actual versus expected result, time domain and any
unverified media/backend path. Do not fix a boundary by adding an unexplained epsilon.
