FROM --platform=linux/risc64 riscv64/ubuntu:22.04
RUN apt-get update && apt-get install -y python3-requests
COPY ./app.py /init.py
CMD ["python3", "/init.py"]
