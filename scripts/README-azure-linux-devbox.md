# Azure Linux 开发机初始化脚本

用于把 Azure Ubuntu VM 准备成带轻量 GUI 的开发机：

- SSH 端口改为 `65444`
- xrdp/RDP 端口改为 `65445`
- 安装 XFCE 桌面环境
- 安装 xrdp
- 安装 VS Code 桌面版
- 安装 Docker / Docker Compose
- 安装常见 Tauri Linux 构建依赖

脚本文件：

```text
scripts/azure-linux-devbox-bootstrap.sh
```

## 推荐 Azure VM

建议从官方镜像开始：

```text
Ubuntu Server 24.04 LTS
Standard_D4ds_v5
Premium SSD 256 GiB
SSH public key 登录
```

## Azure NSG 端口

脚本会修改 VM 内部服务端口，但 **Azure NSG 仍然需要你在 Portal 里单独放行**。

建议规则：

| 端口 | 用途 | 建议来源 |
|---:|---|---|
| `65444/tcp` | SSH | 只允许你的公网 IP |
| `65445/tcp` | xrdp/RDP | 不建议公网开放；优先用 SSH tunnel |

如果是新 VM，建议先临时保留 `22/tcp` 只允许你的 IP，确认 `65444` 可以 SSH 后再删除 `22/tcp` 入站规则。

## 在 Azure Portal 执行

路径大致是：

```text
Azure Portal
  -> Virtual machines
  -> 你的 Linux VM
  -> Run command
  -> RunShellScript
```

把 `azure-linux-devbox-bootstrap.sh` 的完整内容粘贴进去执行。

默认配置就是：

```bash
SSH_PORT=65444
RDP_PORT=65445
```

如果你的用户名不是 `azureuser`，可以在脚本内容前面加：

```bash
export DEV_USER="你的用户名"
```

如果希望脚本同时设置 xrdp 登录密码，可以加：

```bash
export DEVBOX_USER_PASSWORD="临时强密码"
```

> 注意：在 Portal Run Command 里写明文密码会进入执行记录/日志风险。更安全做法是脚本跑完后用 SSH 登录，再执行 `sudo passwd <user>` 设置本地密码。

## 执行后 SSH 连接

脚本成功后，用新端口连接：

```text
ssh -p 65444 <user>@<public-ip-or-dns>
```

如果 Docker 已安装，首次执行后需要重新登录一次，让 `docker` 用户组生效。

## 推荐的安全 RDP 方式

不建议直接把 `65445` 暴露到公网。推荐用 SSH tunnel：

```text
ssh -p 65444 -L 65445:localhost:65445 <user>@<public-ip-or-dns>
```

然后本机远程桌面连接：

```text
localhost:65445
```

登录 xrdp 时使用 Linux 本地用户名和密码。

## 常用开关

脚本支持通过环境变量关闭部分安装：

```bash
export INSTALL_VSCODE=false
export INSTALL_DOCKER=false
export INSTALL_TAURI_DEPS=false
```

端口也可以覆盖：

```bash
export SSH_PORT=65444
export RDP_PORT=65445
```

## 重要提醒

- 脚本只改 VM 内部配置，不会自动修改 Azure NSG。
- 改 SSH 端口前务必确保 NSG 已允许 `65444/tcp`，否则可能需要用 Azure Serial Console 或 Run Command 修复。
- xrdp 需要本地用户密码，SSH key 不能直接用于 xrdp 登录。
- GUI 会占用内存；日常开发仍推荐 VS Code Remote SSH，xrdp 作为备用图形界面。
