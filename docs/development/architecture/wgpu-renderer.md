# WGPU renderer

The WGPU backend discovers an adapter, creates device/context state, prepares GPU resources, uploads evaluated source data, encodes source/effect passes, and reads completed frames back for output. It reuses resource pools and staging/readback state across a prepared render. Nested composition and effect work remain plan-driven dispatch, not a second project evaluator.

Backend reporting records the actual graphics backend, adapter, and classification. Adapter selection is architecture; commands and hardware policy belong in [GPU validation](../gpu-validation.md).
