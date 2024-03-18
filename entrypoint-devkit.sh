#!/bin/bash
if [ x$NO_SSH = x ]; then
	ssh-keygen -A
	/usr/sbin/sshd -D -e &
fi

bash /entrypoint-lambada.sh &

code-server --bind-addr 0.0.0.0:8081 --disable-telemetry --disable-getting-started-override 2>&1 &> /tmp/code-server.log &
containerd 2>&1 &> /dev/null & 
buildkitd --oci-worker=true 2>&1 &> /dev/null &

sleep 15
export TERM=xterm
clear

echo "==============================================================================="
echo "Now you can access the Cartesi development environment on http://localhost:8081"
echo
echo "or"
echo
HOSTNAME=`hostname`
echo "docker exec -it $HOSTNAME /bin/bash"
echo "==============================================================================="

while true; do
  sleep 3600
done

