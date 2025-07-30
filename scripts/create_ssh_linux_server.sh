#!/bin/bash

# --- Configuration ---
IMAGE_NAME="custom-ubuntu-ssh"
CONTAINER_NAME="ubuntu-ssh-container"
SSH_PORT=2222

SSH_USERNAME="user"
SSH_PASSWORD="abc123"

KEY_DIR="./build_context/ssh_keys"
DOCKERFILE_DIR="./build_context"

# --- Preparation ---
echo "📁 Setting up SSH key directory..."
mkdir -p "$KEY_DIR"
mkdir -p "$KEY_DIR/host_keys"
touch "$KEY_DIR/host_keys/.keep"

echo "🔐 Generating SSH key pair..."
if [ ! -f "$KEY_DIR/id_rsa" ]; then
    ssh-keygen -t rsa -b 2048 -f "$KEY_DIR/id_rsa" -N ""
else
    echo "SSH key pair already exists, skipping generation..."
fi

# --- Dockerfile ---
echo "📝 Creating Dockerfile..."
mkdir -p "$DOCKERFILE_DIR"
cat > "$DOCKERFILE_DIR/Dockerfile" <<EOF
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:\$PATH"

# Update and install required packages in a single RUN command
RUN apt-get update && apt-get install -y --no-install-recommends \\
    apt-transport-https \\
    build-essential \\
    ca-certificates \\
    curl \\
    git \\
    gnupg \\
    libssl-dev \\
    lsb-release \\
    openssh-server \\
    pkg-config \\
    sshpass \\
    sudo \\
    && rm -rf /var/lib/apt/lists/* \\
    && mkdir /var/run/sshd

# Install Docker
RUN curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg && \\
    echo "deb [arch=\$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu \$(lsb_release -cs) stable" > /etc/apt/sources.list.d/docker.list && \\
    apt-get update && apt-get install -y --no-install-recommends docker-ce docker-ce-cli containerd.io docker-compose-plugin && \\
    rm -rf /var/lib/apt/lists/*

# Install rustup and Rust toolchain
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y && \\
    rm -rf /root/.cargo/registry /root/.cargo/git

# Create a new user and add to docker group
RUN useradd -m -s /bin/bash $SSH_USERNAME && \\
    echo "$SSH_USERNAME:$SSH_PASSWORD" | chpasswd && \\
    usermod -aG sudo $SSH_USERNAME && \\
    usermod -aG docker $SSH_USERNAME

# Setup SSH directory and authorized_keys
RUN mkdir -p /home/$SSH_USERNAME/.ssh
COPY ssh_keys/id_rsa.pub /home/$SSH_USERNAME/.ssh/authorized_keys

RUN chmod 700 /home/$SSH_USERNAME/.ssh && \\
    chmod 600 /home/$SSH_USERNAME/.ssh/authorized_keys && \\
    chown -R $SSH_USERNAME:$SSH_USERNAME /home/$SSH_USERNAME/.ssh

# Copy persistent SSH host keys if they exist
COPY ssh_keys/host_keys /tmp/ssh_keys/host_keys
RUN if [ -f /tmp/ssh_keys/host_keys/ssh_host_rsa_key ]; then \\
        cp /tmp/ssh_keys/host_keys/ssh_host_* /etc/ssh/; \\
    fi

# Configure SSH to allow password authentication
RUN sed -i 's/#PasswordAuthentication yes/PasswordAuthentication yes/' /etc/ssh/sshd_config && \\
    sed -i 's/#PubkeyAuthentication yes/PubkeyAuthentication yes/' /etc/ssh/sshd_config

# Configure sudo to not require password for docker group
RUN echo '%docker ALL=(ALL) NOPASSWD: /usr/bin/docker' >> /etc/sudoers

# Create a startup script that starts both Docker daemon and SSH
RUN echo '#!/bin/bash' > /start.sh && \\
    echo 'service docker start' >> /start.sh && \\
    echo 'sleep 3' >> /start.sh && \\
    echo 'chmod 666 /var/run/docker.sock' >> /start.sh && \\
    echo 'ssh-keygen -A' >> /start.sh && \\
    echo 'exec /usr/sbin/sshd -D' >> /start.sh && \\
    chmod +x /start.sh

EXPOSE 22
CMD ["/start.sh"]
EOF

# --- Build Docker Image ---
echo "🐳 Building Docker image: $IMAGE_NAME"
if ! docker build -t "$IMAGE_NAME" "$DOCKERFILE_DIR"; then
    echo "❌ Docker build failed! Exiting..."
    exit 1
fi

# --- Remove old container if exists ---
if [ "$(docker ps -aq -f name=$CONTAINER_NAME)" ]; then
    echo "🧹 Cleaning up old container..."
    docker rm -f "$CONTAINER_NAME"
fi

# --- Run the Container ---
echo "🚀 Starting container: $CONTAINER_NAME on port $SSH_PORT"
if ! docker run -d --name "$CONTAINER_NAME" -p "$SSH_PORT:22" --privileged -v /var/run/docker.sock:/var/run/docker.sock "$IMAGE_NAME"; then
    echo "❌ Container failed to start! Exiting..."
    exit 1
fi

echo "⏳ Waiting for container to start..."
sleep 8

# Check if container is actually running
if ! docker ps | grep -q "$CONTAINER_NAME"; then
    echo "❌ Container is not running! Checking logs..."
    docker logs "$CONTAINER_NAME"
    exit 1
fi

# Extract SSH host keys for future use
echo "💾 Backing up SSH host keys..."
for key in rsa ecdsa ed25519; do
    docker cp "$CONTAINER_NAME:/etc/ssh/ssh_host_${key}_key" "$KEY_DIR/host_keys/" 2>/dev/null || true
    docker cp "$CONTAINER_NAME:/etc/ssh/ssh_host_${key}_key.pub" "$KEY_DIR/host_keys/" 2>/dev/null || true
done

# Test Docker access
echo "🧪 Testing Docker access..."
if ssh -i "$KEY_DIR/id_rsa" -o StrictHostKeyChecking=no -p "$SSH_PORT" "$SSH_USERNAME@localhost" 'docker --version' >/dev/null 2>&1; then
    echo "✅ Docker access working without sudo!"
else
    echo "⚠️  Docker requires sudo - CI pipeline will need to use 'sudo docker'"
fi

# --- Output Connection Info ---
echo "======================================"
echo "✅ SSH Server is up and running!"
echo "🔗 Connect using:"
echo "   ssh -i $KEY_DIR/id_rsa $SSH_USERNAME@localhost -p $SSH_PORT"
echo "🗝️  Or login with password: $SSH_PASSWORD"
echo "📂 SSH Private Key: $KEY_DIR/id_rsa"
echo "📂 SSH Public Key : $KEY_DIR/id_rsa.pub"
echo ""
echo "🐳 Docker is available inside the container!"
echo "   Test with: ssh -p $SSH_PORT $SSH_USERNAME@localhost 'docker --version'"
echo ""
echo "🔧 To avoid SSH host key warnings in the future:"
echo "   Run: ssh-keygen -R \"[localhost]:$SSH_PORT\""
echo "   Then connect normally"
echo "======================================"
