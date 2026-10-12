# Implement event

Implement the declared event payload and identity/time fields using existing domain contracts. Preserve modeled delivery requirements, but do not invent ordering, retry, deduplication or exactly-once guarantees. Keep event creation separate from publication; test actual emission when publication is in scope.
