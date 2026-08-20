# Add a backend

A backend consumes the same prepared/evaluated plan as CPU and WGPU and must preserve supported semantics. Implement resource preparation, frame output, cancellation/error behavior, actual-backend reporting, metrics/progress integration, selection/probing, and focused equivalence tests. It must not reinterpret canonical project semantics.

Document capability limits honestly. Backend availability, requested preference, and actual selected backend are separate facts.
