# Add a source

Adding a source is an end-to-end change. Define its canonical model and semantic validation in `vestra-core`, extend schema serialization, compile/evaluate it into plan data, then implement CPU and WGPU resource/render paths. Add Python authoring/lowering, public exports/stubs where applicable, and tests for model, schema, Python, CPU, WGPU, nesting, effects, and transitions according to the source capabilities.

Update the source reference and feature matrix from tests. Do not claim a source supports direct transform or transition endpoints unless its canonical capability and both renderer paths establish it.
