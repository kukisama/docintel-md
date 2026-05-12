#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive
DEV_USER="${DEV_USER:-kukisama}"

apt-get update -y && apt-get install -y \
	openssh-server \
	git \
	curl \
	wget \
	ca-certificates \
	unzip \
	zip \
	tar \
	gzip \
	build-essential \
	procps \
	file \
	less

install -d /etc/ssh/sshd_config.d
printf 'Port 65444\n' >/etc/ssh/sshd_config.d/99-port-65444.conf
install -d -m 0755 /run/sshd
sshd -t
systemctl disable --now ssh.socket >/dev/null 2>&1 || true
systemctl disable --now xrdp >/dev/null 2>&1 || true
systemctl enable ssh >/dev/null 2>&1 || true
systemctl stop ssh || systemctl stop sshd || true
systemctl start ssh || systemctl start sshd

install -d -m 0755 /workspace

if id "${DEV_USER}" >/dev/null 2>&1; then
	USER_HOME="$(getent passwd "${DEV_USER}" | cut -d: -f6)"
	install -d -m 0700 -o "${DEV_USER}" -g "${DEV_USER}" "${USER_HOME}/.ssh"
	install -d -m 0755 -o "${DEV_USER}" -g "${DEV_USER}" "${USER_HOME}/.cache"
	install -d -m 0755 -o "${DEV_USER}" -g "${DEV_USER}" "${USER_HOME}/.config"
	install -d -m 0755 -o "${DEV_USER}" -g "${DEV_USER}" "${USER_HOME}/.vscode-server"
	chown "${DEV_USER}:${DEV_USER}" /workspace
fi

echo "Done. Open Azure NSG TCP 65444 only, then connect with VS Code Remote SSH or: ssh -p 65444 ${DEV_USER}@<public-ip>. VS Code Server will be installed automatically on first Remote SSH connection."