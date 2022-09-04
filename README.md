中文 | [English](README_en.md)

# bim

bim 是 bench.im 网站专用的测速客户端，提供交互式测速和后台定时测速功能，同时支持 Speedtest 和 Librespeed 的测速服务器。

#### 使用方法

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

#### 交互式测速

下载客户端，对于 Linux 系统使用：

```
wget https://bench.im/dl/linux/$(uname -m)/bim -O bim && chmod +x bim
```

运行客户端需要指定服务器，可以用 CN 来指代中国区域的服务器：

```
./bim CN
```

支持地区代码、服务器 ID 和名称等，与 bench.im 网站本身的搜索结果一致。另外，也可以使用 `-s` 参数来指定服务器清单，如：

```
./bim -s 1
```

将对 1 号服务器清单中的服务器测速。


#### 后台持续测速

**注意**：所有测速结果将上传到 bench.im 网站保存！

在 bench.im 注册账号后即可在用户中心获得密钥，使用账户邮箱和密钥即可开启后台模式：

```
./bim -d user@mail.com:yourkey
```

具体测速任务请在 bench.im 设置。以下是 Systemd 示例配置文件：

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

#### 运行截图

![运行截图][1]


  [1]: https://bench.im/bim.png
