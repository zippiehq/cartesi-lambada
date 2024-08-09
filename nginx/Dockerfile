FROM ghcr.io/zippiehq/cartesi-lambada:latest

RUN apt-get -y update && apt-get -y install nginx

COPY default /etc/nginx/sites-available/default

COPY nginx-entrypoint.sh /nginx-entrypoint.sh

RUN chmod +x /entrypoint.sh

EXPOSE 8080
EXPOSE 80
EXPOSE 4001/tcp
EXPOSE 4001/udp

CMD bash nginx-entrypoint.sh