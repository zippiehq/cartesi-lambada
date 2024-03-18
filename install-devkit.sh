#!/bin/bash
echo "Installing Cartesi devkit....."
echo	
apt-get update && apt-get install -y --no-install-recommends curl ca-certificates qemu-system-misc openssh-server jq iproute2 git joe containerd
if [ $? != 0 ]; then
	echo "Failed to download Ubuntu dependencies"
	exit 1
fi
sed -i '/^#Port 22/s/^#//' /etc/ssh/sshd_config \
    && sed -i '/^#PasswordAuthentication/s/^#//' /etc/ssh/sshd_config \
    && sed -i '/^#PermitEmptyPasswords no/s/^#PermitEmptyPasswords no/PermitEmptyPasswords yes/' /etc/ssh/sshd_config \
    && sed -i '/^#PermitRootLogin prohibit-password/s/^#PermitRootLogin prohibit-password/PermitRootLogin yes/' /etc/ssh/sshd_config \
    && echo "root:lambada" | chpasswd
mkdir -p /run/sshd
ARCH=`uname -m | sed s/aarch64/arm64/g | sed s/x86_64/amd64/g`
curl -fOL https://github.com/coder/code-server/releases/download/v4.21.0/code-server_4.21.0_$ARCH.deb && dpkg -i code-server_4.21.0_$ARCH.deb && rm -f code-server_4.21.0_$ARCH.deb
if [ $? != 0 ]; then
	echo "Failed to get and install code server"
	exit 1
fi
mkdir -p /root/.config/code-server/ && printf 'bind-addr: 127.0.0.1:8081\nauth: none\ncert: false\n' > /root/.config/code-server/config.yaml
curl -OL https://github.com/moby/buildkit/releases/download/v0.12.5/buildkit-v0.12.5.linux-$ARCH.tar.gz && mkdir -p buildkit && cd buildkit && tar xf ../buildkit-v0.12.5.linux-$ARCH.tar.gz && cp bin/* /usr/bin && cd .. && rm -rf ../buildkit*
if [ $? != 0 ]; then
	echo "Failed to get and install buildkit"
	exit 1
fi
curl -OL https://github.com/containerd/nerdctl/releases/download/v1.7.3/nerdctl-1.7.3-linux-$ARCH.tar.gz && tar xf nerdctl-1.7.3-linux-$ARCH.tar.gz && cp nerdctl /usr/bin && rm -rf nerdctl*
if [ $? != 0 ]; then
	echo "Failed to get and install nerdctl"
	exit 1
fi
