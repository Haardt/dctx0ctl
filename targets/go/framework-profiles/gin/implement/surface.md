# Implement surface

Expose only the explicitly assigned application entry points. Map transport input, results and failures to existing domain contracts and validate at the boundary. Keep business rules in the real application operation. Verify actual framework routing/composition when assigned; do not infer URLs, publish internal operations or choose a framework from the spec alone.

Register the assigned routes with the actual framework router. Decode transport values, invoke the delivered service, and map its result/failures. Verify path/query/body handling, status and content-type against explicit HTTP requirements. Follow the framework rules in common.md.
