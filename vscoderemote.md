# VS Code Remote SSH 连接说明

这份文档只解决一件事：

- **从 Windows 上的 VS Code，连接到 Azure Linux 虚机进行远程开发**

当前约定：

- 服务器 IP：`40.90.172.14`
- SSH 端口：`65444`
- 登录用户：`kukisama`
- 登录方式：**账号密码**
- VS Code 方式：**Remote - SSH**

---

## 一句话理解

你真正需要做的只有 4 步：

1. 服务器已经配置好 SSH 端口 `65444`
2. Azure NSG 放行 `TCP 65444`
3. Windows 本机写一个 SSH 配置
4. 在 VS Code 里选择这个主机连接

**远程机器上不需要安装 VS Code 桌面版。**

第一次连接时，VS Code 会自动安装 `VS Code Server`。

---

## 先决条件

请确认下面几件事已经完成：

- Azure Linux 虚机已经启动
- 服务器脚本已经执行过
- Azure 网络安全组（NSG）已经放行：`TCP 65444`
- 你知道这台机器的：
	- 公网 IP
	- 用户名
	- 密码

如果这几项没完成，VS Code 这边再怎么点也连不上。

---

## 第一步：先用命令行确认能连通

先不要急着打开 VS Code。

在 Windows PowerShell 中执行：

```powershell
ssh -p 65444 kukisama@40.90.172.14
```

然后会发生两件事：

### 第一次连接

如果是第一次连接，通常会看到：

```text
The authenticity of host ... can't be established.
Are you sure you want to continue connecting (yes/no/[fingerprint])?
```

输入：

```text
yes
```

### 接着输入密码

然后会提示：

```text
kukisama@40.90.172.14's password:
```

直接输入密码并回车。

注意：**输入密码时屏幕不会显示任何字符，这是正常的。**

---

## 第二步：配置 Windows 本机 SSH

编辑下面这个文件：

```text
C:\Users\kukisama\.ssh\config
```

把下面这段放进去：

```ssh-config
Host azure-devbox
    HostName 40.90.172.14
    User kukisama
    Port 65444
    PreferredAuthentications password
    PubkeyAuthentication no
    ServerAliveInterval 30
    ServerAliveCountMax 6
```

这几个配置的作用：

- `Host azure-devbox`：给这台机器起一个短名字
- `HostName 40.90.172.14`：远程机器 IP
- `User kukisama`：登录用户名
- `Port 65444`：SSH 端口
- `PreferredAuthentications password`：优先用密码认证
- `PubkeyAuthentication no`：先别折腾密钥，直接密码登录

写好之后，在 PowerShell 里就可以直接这样连：

```powershell
ssh azure-devbox
```

---

## 第三步：在 VS Code 安装扩展

本机 VS Code 安装下面这个扩展：

- `Remote - SSH`

安装后建议重启一次 VS Code。

---

## 第四步：在 VS Code 中连接

### 方法一：命令面板连接

1. 打开 VS Code
2. 按 `Ctrl + Shift + P`
3. 输入：`Remote-SSH: Connect to Host`
4. 选择：`azure-devbox`
5. 如果提示平台，选择：`Linux`
6. 输入密码

连接成功后，VS Code 会自动：

- 安装远程 `VS Code Server`
- 建立远程工作会话
- 让你选择要打开的目录

---

## 第五步：打开远程工作目录

建议直接打开：

```text
/workspace
```

如果以后把仓库 clone 到远程机器上，可以打开：

```text
/workspace/docintel-md
```

---

## 最推荐的实际操作顺序

按这个顺序做，最省心：

### 1. 在 PowerShell 测试 SSH

```powershell
ssh azure-devbox
```

### 2. 再用 VS Code 连接

- `Ctrl + Shift + P`
- `Remote-SSH: Connect to Host`
- 选择 `azure-devbox`

### 3. 打开 `/workspace`

这样流程最稳，不容易被 VS Code 的界面提示绕晕。

---

## 常见错误与解决办法

### 错误 1：命令写成了 `IP:端口`

错误写法：

```powershell
ssh -p 65444 kukisama@40.90.172.14:65444
```

这是错的，原因是：

- `-p 65444` 已经指定了端口
- 后面的主机地址里就**不能再写 `:65444`**

正确写法：

```powershell
ssh -p 65444 kukisama@40.90.172.14
```

或者：

```powershell
ssh azure-devbox
```

---

### 错误 2：连接超时

报错示例：

```text
Connection timed out
```

一般说明：

- Azure NSG 没放行 `65444`
- 公网 IP 不对
- 虚机没开机

---

### 错误 3：连接被拒绝

报错示例：

```text
Connection refused
```

一般说明：

- 服务器 SSH 没监听在 `65444`
- 启动脚本没成功生效

---

### 错误 4：密码不通过

报错示例：

```text
Permission denied
```

一般说明：

- 用户名不对
- 密码不对
- 远程服务器没有开启密码登录

如果出现 `Permission denied (publickey)`，说明 SSH 还在优先尝试密钥认证，这时要检查本机 `config` 里是否已经写了：

```ssh-config
PreferredAuthentications password
PubkeyAuthentication no
```

---

## 为什么还要写 SSH 配置文件

因为 VS Code 的 Remote SSH 本质上就是基于 SSH 工作的。

图形界面只能帮你做一部分事情，但：

- 端口
- 用户名
- 认证方式
- 保活参数

这些信息最终还是要落到 SSH 配置里。

好消息是：**只需要写一次。**

写完以后，后面基本就只需要：

- 在终端里输入 `ssh azure-devbox`
- 或在 VS Code 里选择 `azure-devbox`

---

## 最短版速查

### PowerShell 连接命令

```powershell
ssh -p 65444 kukisama@40.90.172.14
```

### 推荐的 SSH 配置

```ssh-config
Host azure-devbox
    HostName 40.90.172.14
    User kukisama
    Port 65444
    PreferredAuthentications password
    PubkeyAuthentication no
```

### VS Code 连接入口

```text
Ctrl + Shift + P
Remote-SSH: Connect to Host
azure-devbox
```

---

## 后续建议

等这条链路跑通以后，再考虑下一步：

- 把仓库 clone 到 `/workspace`
- 在远程装 Node / Rust / Docker 等开发环境
- 把这台 Azure Linux 机器固定成长期开发机

当前阶段，不建议一开始就把脚本继续堆成“大而全”，先保证 **能稳定连接、能稳定开发** 更重要。
