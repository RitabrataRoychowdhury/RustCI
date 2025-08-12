#!/bin/bash

# Valkyrie Protocol Certificate Generation Script
# This script generates self-signed certificates for development and testing

set -euo pipefail

CERT_DIR="/certs"
DAYS=365
KEY_SIZE=4096

# Create certificate directory
mkdir -p "$CERT_DIR"

echo "🔐 Generating Valkyrie Protocol Certificates..."

# Generate CA private key
echo "📝 Generating CA private key..."
openssl genrsa -out "$CERT_DIR/ca-key.pem" $KEY_SIZE

# Generate CA certificate
echo "📝 Generating CA certificate..."
openssl req -new -x509 -days $DAYS -key "$CERT_DIR/ca-key.pem" -out "$CERT_DIR/ca.crt" -subj "/C=US/ST=CA/L=San Francisco/O=RustCI/OU=Valkyrie Protocol/CN=Valkyrie CA"

# Generate server private key
echo "📝 Generating server private key..."
openssl genrsa -out "$CERT_DIR/tls.key" $KEY_SIZE

# Generate server certificate signing request
echo "📝 Generating server certificate signing request..."
openssl req -new -key "$CERT_DIR/tls.key" -out "$CERT_DIR/server.csr" -subj "/C=US/ST=CA/L=San Francisco/O=RustCI/OU=Valkyrie Protocol/CN=valkyrie-server"

# Create certificate extensions file
cat > "$CERT_DIR/server.ext" << EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
subjectAltName = @alt_names

[alt_names]
DNS.1 = valkyrie-server
DNS.2 = valkyrie-protocol
DNS.3 = localhost
DNS.4 = valkyrie.local
DNS.5 = *.valkyrie.local
IP.1 = 127.0.0.1
IP.2 = ::1
IP.3 = 172.20.0.2
EOF

# Generate server certificate
echo "📝 Generating server certificate..."
openssl x509 -req -in "$CERT_DIR/server.csr" -CA "$CERT_DIR/ca.crt" -CAkey "$CERT_DIR/ca-key.pem" -CAcreateserial -out "$CERT_DIR/tls.crt" -days $DAYS -extensions v3_req -extfile "$CERT_DIR/server.ext"

# Generate client private key
echo "📝 Generating client private key..."
openssl genrsa -out "$CERT_DIR/client-key.pem" $KEY_SIZE

# Generate client certificate signing request
echo "📝 Generating client certificate signing request..."
openssl req -new -key "$CERT_DIR/client-key.pem" -out "$CERT_DIR/client.csr" -subj "/C=US/ST=CA/L=San Francisco/O=RustCI/OU=Valkyrie Protocol/CN=valkyrie-client"

# Generate client certificate
echo "📝 Generating client certificate..."
openssl x509 -req -in "$CERT_DIR/client.csr" -CA "$CERT_DIR/ca.crt" -CAkey "$CERT_DIR/ca-key.pem" -CAcreateserial -out "$CERT_DIR/client.crt" -days $DAYS

# Generate JWT signing keys
echo "📝 Generating JWT signing keys..."
openssl genrsa -out "$CERT_DIR/jwt-private.pem" 2048
openssl rsa -in "$CERT_DIR/jwt-private.pem" -pubout -out "$CERT_DIR/jwt-public.pem"

# Generate DH parameters for enhanced security
echo "📝 Generating DH parameters..."
openssl dhparam -out "$CERT_DIR/dhparam.pem" 2048

# Set appropriate permissions
chmod 600 "$CERT_DIR"/*.key "$CERT_DIR"/*.pem
chmod 644 "$CERT_DIR"/*.crt "$CERT_DIR"/*.ext

# Clean up temporary files
rm -f "$CERT_DIR"/*.csr "$CERT_DIR"/*.srl "$CERT_DIR"/*.ext

echo "✅ Certificate generation completed!"
echo "📁 Certificates saved to: $CERT_DIR"
echo ""
echo "📋 Generated files:"
echo "  - ca.crt (Certificate Authority)"
echo "  - tls.crt (Server Certificate)"
echo "  - tls.key (Server Private Key)"
echo "  - client.crt (Client Certificate)"
echo "  - client-key.pem (Client Private Key)"
echo "  - jwt-private.pem (JWT Private Key)"
echo "  - jwt-public.pem (JWT Public Key)"
echo "  - dhparam.pem (DH Parameters)"
echo ""
echo "🔒 Note: These are self-signed certificates for development/testing only."
echo "    For production, use certificates from a trusted CA."