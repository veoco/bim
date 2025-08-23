# bim

bim 是现代网络性能监控平台的监控客户端，与 bench.im 网页前端和 bim-server 服务器后端共同构成一个完整的网络监控解决方案。该平台旨在替代传统的 Smokeping，提供更直观、更易用的网络延迟与可达性可视化体验，并支持动态管理监控节点和目标。

## 项目概述

网页前端： https://github.com/veoco/bench.im

服务器后端：https://github.com/veoco/bim-server

监控客户端（本项目）：https://github.com/veoco/bim

## 编译步骤

```
https://github.com/veoco/bim.git
cd bim
cargo build -r
```

## 运行方法

从发布页面下载后解压：

```
tar -xf bim-x86_64-unknown-linux-musl.tar.gz
```

运行：

```
./bim -m 机器id -t 机器密钥 -s https://bench.im
```

## systemd 部署

编写 `/etc/systemd/system/bim.service` ：

```
[Unit]
Description=bim
After=network.target

[Service]
ExecStart=/your_path/bim -m 机器id -t 机器密钥 -s https://bench.im
Restart=always
RestartSec=3
DynamicUser=true
AmbientCapabilities=CAP_NET_RAW
CapabilityBoundingSet=CAP_NET_RAW
NoNewPrivileges=false

[Install]
WantedBy=multi-user.target
```