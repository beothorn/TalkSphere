# Platform adapter exports

`TcpProvider` supplies the SocketProvider contract using OS TCP readiness.
`SystemClock` supplies monotonic time and replaceable deadline wakeups.
Construction is explicit and fallible. Neither exports authentication substitutes.
See [TCP](tcp.md), [clock](clock.md) and the [adapter guide](README.md).
