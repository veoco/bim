[中文](README.md) | English

# bim

bim is a dedicated speed testing client for bench.im website, providing interactive speed testing and background timed speed testing, as well as support for Speedtest and Librespeed speed servers.

#### Usage

```
bim 0.7.3
Simple program to test network

USAGE:
    bim [OPTIONS] <SERVER>

ARGS:
    <SERVER>    

OPTIONS:
    -6, --ipv6               Enable IPv6 only test
    -d, --deploy             Deply mode
    -h, --help               Print help information
    -s, --server-list        Enable server list search
    -t, --thread <THREAD>    Number of thread [default: 4]
    -V, --version            Print version information
```

#### Interactive speed measurement

Download the client, for Linux systems use:

```
wget https://bench.im/dl/linux/$(uname -m)/bim -O bim && chmod +x bim
```

To run the client you need to specify the server, you can use CN to refer to the server in the China region:

```
./bim CN
```

Support for country codes, server IDs and names, etc., is consistent with the search results on the bench.im website itself. Alternatively, a list of servers can be specified using the `-s` parameter, e.g:

```
./bim -s 1
```

The servers in server list #1 will be measured for speed.


#### Continuous speed measurement in the background

**Note**: All speed test results will be uploaded to the bench.im website!

After registering an account with bench.im, you can get a key in the user center and use your account email and key to enable the backend mode:

```
./bim -d user@mail.com:yourkey
```

The specific speed measurement tasks are set in bench.im. The following is a sample Systemd configuration file:

```
[Unit]
Description=bim
After=network.target

[Service]
Environment="RUST_LOG=INFO"
ExecStart=/opt/bim/bim -d user@mail.com:yourkey

[Install]
WantedBy=multi-user.target
```

#### Screenshots

![Screenshot][1]


  [1]: https://bench.im/bim.png
