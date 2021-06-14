# rustyping

A prettier lightweight ping utility written in Rust.

![screenshot](https://user-images.githubusercontent.com/21986859/121847661-ebf8d700-ccb6-11eb-84e0-e33528d0ca2a.png)

## Installation

You can either download the latest release from the [releases](https://github.com/k4yt3x/rustyping/releases) page or compile it yourself. You will need `cargo` for the compilation.

```shell
git clone https://github.com/k4yt3x/rustyping.git
cd rustyping
cargo build --release
cargo install --path .
strip $(which rp)
```

rustyping's binary will be installed to Cargo's binary directory (e.g., `~/.cargo/bin/rp`). You can use the command `rp` to launch rustyping. Note that programs on Linus require the CAP_NET_RAW capability to be able to open raw sockets as an non-root user. The command below gives rustyping's binary this capability.

```shell
sudo setcap cap_net_raw=+eip $(which rp)
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
