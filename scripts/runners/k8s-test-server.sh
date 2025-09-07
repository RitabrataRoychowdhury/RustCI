#!/bin/bash

set -e

# --- Configuration ---
IMAGE_NAME="custom-ubuntu-k3s"
CONTAINER_NAME="ubuntu-k3s-container"
SSH_PORT=2223           # SSH like EC2
API_PORT=6443           # Kubernetes API exposed like EC2
K3S_DATA_DIR="./k3s_data"
KEY_DIR="./k3s_keys"
DOCKERFILE_DIR="./k3s_build"

SSH_USERNAME="k8suser"
SSH_PASSWORD="abc123"

# --- Preparation ---
echo "📁 Preparing directories..."
mkdir -p "$KEY_DIR" "$DOCKERFILE_DIR" "$K3S_DATA_DIR"

# Clean up old SSH known_hosts entry
ssh-keygen -R "[localhost]:$SSH_PORT" 2>/dev/null || true

# Generate SSH key pair if missing
if [ ! -f "$KEY_DIR/id_rsa" ]; then
    echo "🔐 Generating SSH key pair..."
    ssh-keygen -t rsa -b 2048 -f "$KEY_DIR/id_rsa" -N ""
else
    echo "✅ SSH key pair already exists, skipping..."
fi

# --- Dockerfile ---
echo "📝 Creating Dockerfile..."
cat > "$DOCKERFILE_DIR/Dockerfile" <<'EOF'
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# Install base dependencies
RUN apt-get update && apt-get install -y \
    curl \
    openssh-server \
    sudo \
    git \
    iptables \
    iproute2 \
    socat \
    conntrack \
    ebtables \
    ethtool \
    util-linux \
    mount \
    bash-completion \
    nano \
    && mkdir -p /var/run/sshd /run/sshd   # ✅ FIX: ensure both paths exist

# --- Install kubectl ---
RUN KUBECTL_VERSION=$(curl -L -s https://dl.k8s.io/release/stable.txt) && \
    curl -LO "https://dl.k8s.io/release/${KUBECTL_VERSION}/bin/linux/amd64/kubectl" && \
    chmod +x kubectl && mv kubectl /usr/local/bin/

# --- Install Helm ---
RUN curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# --- Install k3s binary ---
RUN curl -sfL https://github.com/k3s-io/k3s/releases/latest/download/k3s \
    -o /usr/local/bin/k3s && \
    chmod +x /usr/local/bin/k3s

# Create user
ARG SSH_USERNAME
ARG SSH_PASSWORD
RUN useradd -m -s /bin/bash $SSH_USERNAME && \
    echo "$SSH_USERNAME:$SSH_PASSWORD" | chpasswd && \
    usermod -aG sudo $SSH_USERNAME

# Setup SSH keys
RUN mkdir -p /home/$SSH_USERNAME/.ssh
COPY id_rsa.pub /home/$SSH_USERNAME/.ssh/authorized_keys
RUN chown -R $SSH_USERNAME:$SSH_USERNAME /home/$SSH_USERNAME/.ssh && \
    chmod 700 /home/$SSH_USERNAME/.ssh && \
    chmod 600 /home/$SSH_USERNAME/.ssh/authorized_keys

# Allow password auth
RUN sed -i 's/#PasswordAuthentication yes/PasswordAuthentication yes/' /etc/ssh/sshd_config && \
    sed -i 's/#PubkeyAuthentication yes/PubkeyAuthentication yes/' /etc/ssh/sshd_config

# Startup script: Run k3s + SSH
RUN echo '#!/bin/bash' > /start.sh && \
    echo 'echo "🚀 Starting k3s server..."' >> /start.sh && \
    echo 'nohup k3s server --tls-san 0.0.0.0 --tls-san 127.0.0.1 --write-kubeconfig /etc/rancher/k3s/k3s.yaml --write-kubeconfig-mode 644 --data-dir /var/lib/rancher/k3s >/var/log/k3s.log 2>&1 &' >> /start.sh && \
    echo 'echo "⏳ Waiting for kubeconfig..."' >> /start.sh && \
    echo 'for i in {1..120}; do if [ -f /etc/rancher/k3s/k3s.yaml ]; then break; fi; echo "waiting for kubeconfig..."; sleep 2; done' >> /start.sh && \
    echo '# Patch kubeconfig to container IP for external access' >> /start.sh && \
    echo 'CONTAINER_IP=$(hostname -i)' >> /start.sh && \
    echo 'sed -i "s/127.0.0.1/${CONTAINER_IP}/g" /etc/rancher/k3s/k3s.yaml' >> /start.sh && \
    echo 'mkdir -p /home/$SSH_USERNAME/.kube' >> /start.sh && \
    echo 'cp /etc/rancher/k3s/k3s.yaml /home/$SSH_USERNAME/.kube/config || true' >> /start.sh && \
    echo 'chown $SSH_USERNAME:$SSH_USERNAME /home/$SSH_USERNAME/.kube/config || true' >> /start.sh && \
    echo 'echo "export KUBECONFIG=$HOME/.kube/config" >> /home/$SSH_USERNAME/.bashrc' >> /start.sh && \
    echo 'echo "export KUBECONFIG=/etc/rancher/k3s/k3s.yaml" >> /etc/environment' >> /start.sh && \
    echo 'mkdir -p /run/sshd' >> /start.sh && \
    echo 'echo "✅ SSH is running; k3s API will be available on 6443 externally"' >> /start.sh && \
    echo 'exec /usr/sbin/sshd -D' >> /start.sh && \
    chmod +x /start.sh

EXPOSE 22 6443
CMD ["/start.sh"]
EOF

# Copy SSH public key for Docker build
cp "$KEY_DIR/id_rsa.pub" "$DOCKERFILE_DIR/"

# --- Build the image ---
echo "🐳 Building Kubernetes test server image..."
docker build \
  --build-arg SSH_USERNAME="$SSH_USERNAME" \
  --build-arg SSH_PASSWORD="$SSH_PASSWORD" \
  -t "$IMAGE_NAME" "$DOCKERFILE_DIR"

# --- Cleanup old container ---
if docker ps -aq -f name=$CONTAINER_NAME >/dev/null; then
    echo "🧹 Removing old container..."
    docker rm -f "$CONTAINER_NAME" || true
fi

# --- Run container like an EC2 node ---
echo "🚀 Starting Kubernetes test server (EC2-like)..."
docker run -d \
    --name "$CONTAINER_NAME" \
    --privileged \
    --tmpfs /run \
    --tmpfs /var/run \
    -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
    -p "$SSH_PORT:22" \
    -p "$API_PORT:6443" \
    -v "$K3S_DATA_DIR:/var/lib/rancher/k3s" \
    "$IMAGE_NAME"

# Wait for SSH daemon
echo "⏳ Waiting for SSH daemon..."
for i in {1..30}; do
    if nc -z localhost $SSH_PORT; then echo "✅ SSH is reachable"; break; fi
    echo "Waiting for SSH..."
    sleep 3
done

# Wait for Kubernetes API readiness inside the container
echo "⏳ Waiting for Kubernetes API to become ready (1-2min)..."
sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no -p "$SSH_PORT" "$SSH_USERNAME@localhost" \
'bash -c "
for i in {1..60}; do
  if kubectl --kubeconfig=/etc/rancher/k3s/k3s.yaml get nodes >/dev/null 2>&1; then
    echo \"✅ API is ready!\";
    exit 0;
  fi;
  echo \"Waiting for API...\";
  sleep 5;
done;
echo \"❌ API not ready in time\";
exit 1
"'

# Copy kubeconfig externally for host usage
echo "📄 Copying kubeconfig to host (ec2-like.yaml)"
docker cp "$CONTAINER_NAME:/etc/rancher/k3s/k3s.yaml" "$KEY_DIR/ec2-like.yaml"
# Patch kubeconfig on host to use localhost:6443
sed -i "s/127.0.0.1/localhost/g" "$KEY_DIR/ec2-like.yaml"

# Final verification
echo "🔍 Testing SSH + kubectl + Helm inside container..."
if sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no -p "$SSH_PORT" "$SSH_USERNAME@localhost" \
   "kubectl get nodes && helm version"; then
    echo "✅ SSH + Kubernetes + Helm working inside node!"
else
    echo "❌ SSH test failed! Check container logs with: docker logs $CONTAINER_NAME"
    exit 1
fi

# Verify external kubectl works from host
echo "🔍 Testing kubectl from HOST using kubeconfig..."
if kubectl --kubeconfig="$KEY_DIR/ec2-like.yaml" get nodes; then
    echo "✅ Host can access Kubernetes API like an EC2 node!"
else
    echo "⚠️  API not reachable externally yet. Check logs."
fi

# --- Output Info ---
echo "======================================"
echo "✅ Kubernetes Test Server (EC2-like) is running!"
echo "🔗 SSH (key): ssh -i $KEY_DIR/id_rsa $SSH_USERNAME@localhost -p $SSH_PORT"
echo "🔗 SSH (password): sshpass -p $SSH_PASSWORD ssh -o StrictHostKeyChecking=no -p $SSH_PORT $SSH_USERNAME@localhost"
echo "🔑 Password auth also works: $SSH_PASSWORD"
echo ""
echo "📂 kubeconfig inside container: /home/$SSH_USERNAME/.kube/config"
echo "📂 kubeconfig for host: $KEY_DIR/ec2-like.yaml"
echo "   You can run: kubectl --kubeconfig=$KEY_DIR/ec2-like.yaml get nodes"
echo ""
echo "🚀 k3s API is exposed on localhost:$API_PORT like an EC2 Kubernetes node"
echo "======================================"

