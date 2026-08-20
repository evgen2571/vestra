# Add an effect

Start with the core effect descriptor catalog. It defines id, scope, parameter kinds, defaults, ranges, and generated-schema/Python metadata. Add canonical representation and validation, compilation/evaluation, CPU and WGPU effect passes, Python class exposure, tests, and the exact reference table. Do not duplicate descriptor facts in each layer.

Test scope, serialization, invalid ranges, animation/signal eligibility, and each renderer path before marking support.
