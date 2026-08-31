# Sockets

Normative contract for the `Socket` namespace: TCP connections and the
WebSocket framing built on them.

Every rule here was derived by probing the running implementation over loopback.

## Permission

Every `Socket` method requires the `Socket` permission. Without it the call is a
**fatal** `PermissionError` / `SZ6001` — not catchable, like every other
namespace gate. See `security.md`.

## API

```text
Socket.listen(port)              -> int    // listener id
Socket.accept(listenerId)        -> int    // connection id
Socket.connect(host, port)       -> int    // connection id
Socket.send(id, data)            -> int    // bytes written
Socket.recv(id, maxBytes)        -> string
Socket.close(id)                 -> null
Socket.sendWsFrame(id, data)     -> null
Socket.recvWsFrame(id)           -> string | null
```

Ids are positive integers, unique within a runtime, and a listener id and a
connection id are drawn from the same space — they are never equal. An id is
meaningful only to the evaluator that created it.

## Reading and writing

`send` returns the number of bytes written. `recv(id, maxBytes)` returns **at
most** `maxBytes` and leaves the remainder queued: sending `"abcdef"` and then
calling `recv(conn, 3)` yields `"abc"`, and the next `recv` yields `"def"`. A
caller that needs a whole message must loop; there is no framing in `recv`
itself.

`recv` blocks until data is available. There is no timeout, no non-blocking
mode and no way to poll, so a `recv` on a connection whose peer never writes
waits until the process is stopped.

## Closing

`close` is idempotent, and closing an id that was never issued is a **no-op**
rather than an error. This is asymmetric with the rest of the namespace: `send`,
`recv` and `accept` on an unknown id are all `SocketError`. The asymmetry is
deliberate — closing something already gone is what a cleanup path does — but it
means a typo in a `close` is silent.

Using an id after closing it is `SocketError`, the same as an id that never
existed. There is no separate "already closed" diagnostic.

## Errors

| Failure | Diagnostic |
| --- | --- |
| The `Socket` permission is not declared | fatal `PermissionError` / `SZ6001` |
| Wrong arity | catchable `TypeError` / `SZ4002` |
| A port or id that is not an integer, or data that is not a string | catchable `TypeError` / `SZ4002` |
| Unknown `Socket` member | catchable `ReferenceError` / `SZ4001` |
| `connect` to a port nothing is listening on | catchable `SocketError` / `SZ4000` |
| `send`, `recv`, `accept` or `recvWsFrame` on an unknown or closed id | catchable `SocketError` / `SZ4000` |

`SocketError` has no code of its own and falls through to `SZ4000`, which
fourteen other kinds also share. Matching on the code alone cannot tell a socket
failure from a GUI failure or an integer overflow; read `kind` as well. See
`errors.md`.

## WebSocket frames

`sendWsFrame` and `recvWsFrame` layer RFC 6455 text framing over an established
connection. They do **not** perform the HTTP upgrade handshake: the caller is
responsible for it, using ordinary `send` and `recv` first.

`recvWsFrame` is the one method in the namespace that does not report a read
failure as an error. It returns `null` for **both** "no message is available
yet" and "the read failed", printing the failure on stderr and continuing. The
two are indistinguishable to the caller.

This is a deliberate compatibility fallback, not an oversight: `null` is also
the documented "no message" answer, `serez-http` and `serez-strike` both branch
on it, and making failure raise would break them. It is recorded as debt in
`errors.md` and stays until a migration is designed.

The frame payload ceiling is 16 MiB (`limits.md`); a larger frame is rejected.

## Not specified

- **The security posture.** `Socket` is permission-gated but not otherwise
  constrained: there is no allowlist of hosts or ports, no TLS, and a granted
  program may connect anywhere the host can. `security.md` is the contract for
  what a permission does and does not mean.
- **Timeouts, non-blocking reads and cancellation.** None exist. See the note
  under Reading and writing.
- **UDP, IPv6 specifics and socket options.** Absent.

## Conformance evidence

- `tests/unit_socket.sz`: loopback lifecycle, id shape, idempotent close.
- `tests/unit_websocket.sz`, `tests/54_websocket_e2e.sz`,
  `tests/55_websocket_integral.sz`, `tests/62_websocket_full_integral.sz`:
  framing.
- `tests/sec_socket_no_permission.sz`: the fatal permission gate.
