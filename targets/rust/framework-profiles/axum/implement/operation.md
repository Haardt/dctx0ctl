# Implement operation

Implement the actual operation with its declared inputs, result, failures, preconditions, postconditions and effects. Reuse delivered types and collaborators; keep unmodeled I/O out of pure operations. Respect action order and state boundaries. Exercise success and relevant failure paths against the real implementation, not a reconstructed expected answer.

Keep domain behavior in the application operation. Framework handlers translate and delegate; they must not implement a second copy of business rules. Preserve cancellation and modeled effects at existing boundaries. Follow the framework rules in common.md.
