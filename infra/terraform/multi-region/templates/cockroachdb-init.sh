#!/bin/bash
set -euo pipefail

# CockroachDB initialization script
# Variables injected by Terraform
CLUSTER_NAME="${cluster_name}"
REGION="${region}"
JOIN_LIST="${join_list}"

# Install CockroachDB
curl https://binaries.cockroachdb.com/cockroach-v23.1.11.linux-amd64.tgz | tar -xz
cp -i cockroach-v23.1.11.linux-amd64/cockroach /usr/local/bin/
mkdir -p /var/lib/cockroach
mkdir -p /var/log/cockroach

# Create systemd service
cat > /etc/systemd/system/cockroachdb.service <<EOF
[Unit]
Description=CockroachDB
After=network.target

[Service]
Type=notify
User=root
ExecStart=/usr/local/bin/cockroach start \
  --certs-dir=/var/lib/cockroach/certs \
  --store=/var/lib/cockroach/data \
  --listen-addr=0.0.0.0:26257 \
  --http-addr=0.0.0.0:8080 \
  --join=$JOIN_LIST \
  --locality=region=$REGION \
  --cache=.25 \
  --max-sql-memory=.25
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Generate certificates (in production, use proper CA)
/usr/local/bin/cockroach cert create-ca \
  --certs-dir=/var/lib/cockroach/certs \
  --ca-key=/var/lib/cockroach/ca.key

/usr/local/bin/cockroach cert create-node \
  localhost \
  $(hostname) \
  $(hostname -f) \
  $(hostname -I | awk '{print $1}') \
  --certs-dir=/var/lib/cockroach/certs \
  --ca-key=/var/lib/cockroach/ca.key

/usr/local/bin/cockroach cert create-client root \
  --certs-dir=/var/lib/cockroach/certs \
  --ca-key=/var/lib/cockroach/ca.key

# Set permissions
chmod 700 /var/lib/cockroach/certs
chmod 600 /var/lib/cockroach/certs/*

# Enable and start service
systemctl daemon-reload
systemctl enable cockroachdb
systemctl start cockroachdb

# Wait for cluster to be ready
sleep 30

# Initialize cluster (only on first node)
if [ "$(hostname)" == "cockroachdb-1" ]; then
  /usr/local/bin/cockroach init \
    --certs-dir=/var/lib/cockroach/certs \
    --host=localhost:26257 || true
fi

echo "CockroachDB node initialized successfully"
