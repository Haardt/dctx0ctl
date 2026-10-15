# Rust Cinema HTTP transport review — 2026-10-15

Reviewer: `codex-rust-cinema-review`. This is an explicit source inspection of
the transport delta, separate from automated native testing. It does not approve
new architecture, change the Cinema specification, or turn review tasks into PASS.

Native macOS Intel acceptance exposed HTTP 400 responses containing `Cannot read
request`. Windows acceptance exposed one failed loopback request. The built-in
server uses a nonblocking listener for shutdown polling, while its HTTP parser
expects bounded blocking reads. Accepted sockets can inherit the listener mode
on BSD and Windows, as described by [Apple's accept manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/accept.2.html)
and [Microsoft's accept documentation](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept).
[Linux accept](https://man7.org/linux/man-pages/man2/accept.2.html) does not inherit
O_NONBLOCK. This platform difference explains the observed timing-sensitive
failure; the corrected behavior still requires native acceptance.

I inspected `src/httpapi.rs`, the request fragmentation regression in
`tests/production.rs`, and the service/persistence boundaries they exercise. The
only production change sets each accepted stream to blocking mode before the
existing five-second read/write timeouts and request parser. A failed mode change
closes that connection. The listener remains nonblocking for shutdown polling.
Routes, JSON/status translation, size limits, validation, event dispatch and SQL
ownership are unchanged. No dctx0 dependency enters production.

The regression sends a request line, headers and two body fragments with delays,
then verifies HTTP 200 and reads the actual persisted film through a second HTTP
request. The native platform runner now executes the application's production
tests on every host, including this regression. The local all-feature application
tests pass; native platform acceptance remains required.

Only these two source bindings in the authored inspection fixture are renewed:

| File | Previous SHA-256 | Reviewed SHA-256 |
| --- | --- | --- |
| `src/httpapi.rs` | `1ac7a9e19de917797989438a3e5eaa37bd5bba5454bd3421bde9d8cd5cbe4bc0` | `c7379b3a17125c30a48f80170fa7a1a84d496898c2d2e0fb41ce3921691f751c` |
| `tests/production.rs` | `45fe4ae1d74a1d481fee5a87ed32a91be45ca890671d6109fca5285a6a788726` | `3d8a27d8edcd97917e8499118ad070381098a134d8ba17da13ba8d372554c21a` |

All other source hashes and existing requirement judgements remain intact.
The acceptance script still rejects source drift; it does not renew these hashes
or create this inspection at runtime. This scoped inspection is not an exhaustive
concurrency, networking or architectural proof.
