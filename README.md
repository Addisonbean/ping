# Ping

## Requirements

- [Rust](https://www.rust-lang.org/) (any relatively recent version should do)

## Building and Running

The program can be built for development using `cargo build` or for production using `cargo build --release`.

The program requires root privileges because it sends raw packets, so it cannot be run using `cargo run`. You can run it directly by using `sudo ./target/debug/ping` for development builds and `sudo ./target/release/ping` for release builds.

Alternatively, on Linux, you can run `sudo setcap cap_net_raw+ep target/debug/ping` to give the binary permission to use send raw packets without executing the program as root. However, this will need to be done after every compilation.

## Usage

```
ping

USAGE:
    ping [FLAGS] [OPTIONS] <address>

FLAGS:
    -h, --help       Prints help information
    -4               Force ping to use IPv4.
    -6               Force ping to use IPv6.
    -V, --version    Prints version information

OPTIONS:
    -c, --count <packet_count>    Stop sending packets after <packet_count> packets have been sent.
    -W, --wait <timeout>          The number of seconds to wait for a reply. Default is 2.
    -t, --ttl <ttl>               The time to live for the icmp echo request, in seconds. Default is 64.

ARGS:
    <address>    The ip or hostname to ping
```
