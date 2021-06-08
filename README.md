# rcping

A prettier ping utility written in Rust.

![image](https://user-images.githubusercontent.com/21986859/121146945-6a183200-c80e-11eb-878c-f9fb10682944.png)

## Installation

You can either download the latest release from the [releases](https://github.com/k4yt3x/rcping/releases) page or compile it yourself. You will need `cargo` for the compilation.

```shell
git clone https://github.com/k4yt3x/rcping.git
cd rcping
cargo build --release
cargo install --path .
```

rcping requires the CAP_NET_RAW capability to be ran as a non-root user.

```shell
sudo setcap cap_net_raw=+eip $(which rcping)
```

## Usages

You can see the usages using the `-h/--help` switch.

```console
USAGE:
    rcping [OPTIONS] <DESTINATION>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --count <COUNT>          stop after <count> replies [default: 0]
    -i, --interval <INTERVAL>    seconds between sending each packet [default: 1]

ARGS:
    <DESTINATION>    dns name or ip address
```
