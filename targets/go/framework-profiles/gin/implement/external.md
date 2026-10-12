# Implement external

Implement the assigned external connection or adapter with explicit configuration, declared failure mapping and resource ownership. Make unavailable dependencies and cleanup observable. Use real integration checks for adapter claims; distinguish them from consumer tests using substitutes. Retries, deadlines and credentials follow explicit requirements/project configuration.

Keep listener, configuration, resource ownership and shutdown explicit. Bind test servers to loopback dynamic ports. Record real dependency failures; do not fall back to example replies. Follow the framework rules in common.md.
