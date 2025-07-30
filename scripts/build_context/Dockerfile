FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:$PATH"

# Update and install required packages in a single RUN command
RUN apt-get update && apt-get install -y --no-install-recommends \
    apt-transport-https \
    build-essential \
    ca-certificates \
    curl \
    git \
    gnupg \
    libssl-dev \
    lsb-release \
    openssh-server \
    pkg-config \
    sshpass \
    sudo \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir /var/run/sshd

# Install Docker
RUN curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" > /etc/apt/sources.list.d/docker.list && \
    apt-get update && apt-get install -y --no-install-recommends docker-ce docker-ce-cli containerd.io docker-compose-plugin && \
    rm -rf /var/lib/apt/lists/*

# Install rustup and Rust toolchain
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y && \
    rm -rf /root/.cargo/registry /root/.cargo/git

# Create a new user and add to docker group
RUN useradd -m -s /bin/bash user && \
    echo "user:abc123" | chpasswd && \
    usermod -aG sudo user && \
    usermod -aG docker user

# Setup SSH directory and authorized_keys
RUN mkdir -p /home/user/.ssh
COPY ssh_keys/id_rsa.pub /home/user/.ssh/authorized_keys

RUN chmod 700 /home/user/.ssh && \
    chmod 600 /home/user/.ssh/authorized_keys && \
    chown -R user:user /home/user/.ssh

# Copy persistent SSH host keys if they exist
COPY ssh_keys/host_keys /tmp/ssh_keys/host_keys
RUN if [ -f /tmp/ssh_keys/host_keys/ssh_host_rsa_key ]; then \
        cp /tmp/ssh_keys/host_keys/ssh_host_* /etc/ssh/; \
    fi

# Configure SSH to allow password authentication
RUN sed -i 's/#PasswordAuthentication yes/PasswordAuthentication yes/' /etc/ssh/sshd_config && \
    sed -i 's/#PubkeyAuthentication yes/PubkeyAuthentication yes/' /etc/ssh/sshd_config

# Configure sudo to not require password for docker group
RUN echo '%docker ALL=(ALL) NOPASSWD: /usr/bin/docker' >> /etc/sudoers

# Create a startup script that starts both Docker daemon and SSH
RUN echo '#!/bin/bash' > /start.sh && \
    echo 'service docker start' >> /start.sh && \
    echo 'sleep 3' >> /start.sh && \
    echo 'chmod 666 /var/run/docker.sock' >> /start.sh && \
    echo 'ssh-keygen -A' >> /start.sh && \
    echo 'exec /usr/sbin/sshd -D' >> /start.sh && \
    chmod +x /start.sh

EXPOSE 22
CMD ["/start.sh"]
