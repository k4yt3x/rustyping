# rustyping

A prettier lightweight colored ping utility written in Rust.

![screenshot](https://user-images.githubusercontent.com/21986859/121847661-ebf8d700-ccb6-11eb-84e0-e33528d0ca2a.png)

## Installation

There are three installation options:

- From the [releases](https://github.com/k4yt3x/rustyping/releases) page
- From [crates.io](https://crates.io/crates/rustyping)
- Compiling it yourself

You will need `cargo` and `rustup` for the compilation. `rustup` is required since rustyping requires the nightly channel of the Rust toolchain. Currently, rustyping only support UNIX platforms (Linux and macOS). If you want it to be supported on other platforms, open an issue.

```shell
git clone https://github.com/k4yt3x/rustyping.git
cd rustyping
cargo build --release
cargo install --path .
strip $(which rp)
```

rustyping's binary will be installed to Cargo's binary directory (e.g., `~/.cargo/bin/rp`). You can use the command `rp` to launch rustyping. Note that programs on Linux require the CAP_NET_RAW capability to be able to open raw sockets as an non-root user. The command below gives rustyping's binary this capability.

```shell
sudo setcap cap_net_raw=+eip $(which rp)
```

## Unrestricted Mode

By default, non-root users can send pings at a minimal interval of 200ms or 0.2s. This is to prevent normal users from being able to cause ICMP floods. If you wish to disable this safety feature, you can compile rustyping with the `unrestricted` feature.

```shell
cargo build --release --features unrestricted
```

## Usages

You can see the usages using the `-h/--help` switch.

```console
USAGE:
    rp [OPTIONS] <DESTINATION>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --count <COUNT>          stop after <count> replies [default: 0]
    -i, --interval <INTERVAL>    seconds between sending each packet [default: 1.0]
    -W, --timeout <TIMEOUT>      time to wait for response [default: 2.0]

ARGS:
    <DESTINATION>    dns name or ip address
```
