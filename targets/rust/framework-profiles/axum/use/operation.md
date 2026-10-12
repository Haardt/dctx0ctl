# Use operation

Call the actual delivered operation with valid domain arguments. Respect its preconditions, result and failure contract, effects and state behavior. Do not reimplement it locally or replace it with an example answer.

Call the delivered service through its established interface. When testing HTTP, call through the actual framework handler instead of bypassing middleware or transport mapping. Follow the framework rules in common.md.
