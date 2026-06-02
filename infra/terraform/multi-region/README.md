# Multi-Region Production Infrastructure

Terraform configuration for deploying Aframp's production infrastructure across multiple AWS regions optimized for sub-Saharan Africa.

## Architecture

### Regions
- **Primary**: Cape Town (af-south-1) - Full stack with write operations
- **Edge**: Lagos (eu-west-1) - Read replicas and API gateway
- **Edge**: Nairobi (eu-central-1) - Read replicas and API gateway

### Components
- **EKS Clusters**: Kubernetes 1.28 with auto-scaling node groups
- **CockroachDB**: Multi-region distributed SQL database
- **Redis Enterprise**: Distributed caching layer
- **Networking**: VPC peering, Transit Gateway, Route53
- **Security**: KMS encryption, WAF, mTLS via Istio
- **Monitoring**: CloudWatch, Prometheus, Grafana

## Prerequisites

### Required Tools
```bash
# Terraform
terraform --version  # >= 1.5.0

# AWS CLI
aws --version  # >= 2.0

# kubectl
kubectl version  # >= 1.28

# Helm
helm version  # >= 3.0

# Istio
istioctl version  # >= 1.19
```

### AWS Credentials
Configure AWS credentials with appropriate permissions:
```bash
aws configure --profile aframp-production

# Or use environment variables
export AWS_ACCESS_KEY_ID="your-key"
export AWS_SECRET_ACCESS_KEY="your-secret"
export AWS_DEFAULT_REGION="af-south-1"
```

### Required IAM Permissions
- EC2 (VPC, Subnets, Security Groups)
- EKS (Clusters, Node Groups)
- ElastiCache (Redis)
- RDS/EC2 (CockroachDB instances)
- Route53 (Hosted Zones, Health Checks)
- KMS (Key management)
- Secrets Manager
- CloudWatch (Logs, Alarms)
- SNS (Notifications)

## Quick Start

### 1. Clone and Navigate
```bash
cd infra/terraform/multi-region
```

### 2. Create terraform.tfvars
```hcl
# terraform.tfvars
environment = "production"

# Alert configuration
alert_email              = "ops@aframp.com"
slack_webhook_url        = "https://hooks.slack.com/services/YOUR/WEBHOOK/URL"
pagerduty_integration_key = "your-pagerduty-key"

# Domain configuration
domain_name = "api.aframp.com"

# Cluster configuration
cluster_version = "1.28"
min_nodes       = 3
max_nodes       = 10
desired_nodes   = 5

# Instance types
node_instance_types       = ["t3.xlarge", "t3.2xlarge"]
cockroachdb_instance_type = "r6i.2xlarge"
redis_node_type           = "cache.r6g.xlarge"

# Feature flags
enable_vpn_tunnels = true
enable_istio       = true
cloudflare_enabled = false
```

### 3. Initialize Terraform
```bash
terraform init
```

### 4. Plan Deployment
```bash
terraform plan -out=tfplan
```

### 5. Apply Infrastructure
```bash
terraform apply tfplan
```

### 6. Save Outputs
```bash
terraform output -json > outputs.json
```

## Configuration

### State Management

This configuration uses S3 backend for state storage:

```hcl
# backend.tf (already configured in main.tf)
terraform {
  backend "s3" {
    bucket         = "aframp-terraform-state"
    key            = "production/multi-region/terraform.tfstate"
    region         = "af-south-1"
    encrypt        = true
    dynamodb_table = "terraform-state-lock"
  }
}
```

**Setup S3 Backend**:
```bash
# Create S3 bucket
aws s3 mb s3://aframp-terraform-state --region af-south-1

# Enable versioning
aws s3api put-bucket-versioning \
  --bucket aframp-terraform-state \
  --versioning-configuration Status=Enabled

# Create DynamoDB table for locking
aws dynamodb create-table \
  --table-name terraform-state-lock \
  --attribute-definitions AttributeName=LockID,AttributeType=S \
  --key-schema AttributeName=LockID,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST \
  --region af-south-1
```

### Variables

#### Required Variables
- `alert_email` - Email for critical alerts
- `slack_webhook_url` - Slack webhook for notifications
- `pagerduty_integration_key` - PagerDuty integration key

#### Optional Variables
See `variables.tf` for complete list with defaults.

### Outputs

After deployment, access outputs:
```bash
# All outputs
terraform output

# Specific output
terraform output eks_primary_cluster_endpoint

# Configure kubectl
$(terraform output -raw kubectl_config_command_primary)
```

## Post-Deployment

### 1. Configure kubectl
```bash
# Primary cluster
aws eks update-kubeconfig --region af-south-1 --name $(terraform output -raw eks_primary_cluster_id) --alias aframp-primary

# Lagos cluster
aws eks update-kubeconfig --region eu-west-1 --name $(terraform output -raw eks_lagos_cluster_id) --alias aframp-lagos

# Nairobi cluster
aws eks update-kubeconfig --region eu-central-1 --name $(terraform output -raw eks_nairobi_cluster_id) --alias aframp-nairobi
```

### 2. Verify Clusters
```bash
kubectl get nodes --context aframp-primary
kubectl get nodes --context aframp-lagos
kubectl get nodes --context aframp-nairobi
```

### 3. Install Istio
```bash
# Install on each cluster
for context in aframp-primary aframp-lagos aframp-nairobi; do
  kubectl config use-context $context
  istioctl install --set profile=production -y
done
```

### 4. Deploy Application
```bash
cd ../../../k8s/production
kubectl apply -f . --context aframp-primary
```

### 5. Configure DNS
Update your domain's nameservers to Route53:
```bash
terraform output public_hosted_zone_nameservers
```

## Maintenance

### Updating Infrastructure

1. **Update variables or configuration**
2. **Plan changes**:
   ```bash
   terraform plan
   ```
3. **Review changes carefully**
4. **Apply updates**:
   ```bash
   terraform apply
   ```

### Scaling Nodes

Update `terraform.tfvars`:
```hcl
min_nodes     = 5
max_nodes     = 15
desired_nodes = 8
```

Then apply:
```bash
terraform apply
```

### Upgrading Kubernetes

Update `cluster_version` in `terraform.tfvars`:
```hcl
cluster_version = "1.29"
```

Apply with caution:
```bash
terraform plan
terraform apply
```

### Rotating Secrets

Redis auth token rotation:
```bash
# Trigger rotation in Secrets Manager
aws secretsmanager rotate-secret \
  --secret-id $(terraform output -raw redis_auth_token_secret_arn)
```

## Disaster Recovery

### Regional Failover

If primary region fails:

1. **Verify edge regions are healthy**:
   ```bash
   kubectl get nodes --context aframp-lagos
   kubectl get nodes --context aframp-nairobi
   ```

2. **Route53 automatically fails over** based on health checks

3. **Promote Lagos or Nairobi to primary** (manual):
   ```bash
   # Update CockroachDB to promote replica
   cockroach sql --host=lagos-cockroachdb.aframp.internal
   ```

### Backup and Restore

**CockroachDB Backups**:
```bash
# Automated daily backups configured
# Manual backup:
cockroach sql --execute="BACKUP TO 's3://aframp-backups/$(date +%Y%m%d)?AWS_ACCESS_KEY_ID=xxx&AWS_SECRET_ACCESS_KEY=xxx'"
```

**Terraform State Backup**:
```bash
# State is versioned in S3
aws s3api list-object-versions --bucket aframp-terraform-state
```

## Monitoring

### CloudWatch Dashboards

Access via AWS Console:
- EKS Cluster Metrics
- CockroachDB Performance
- Redis Cache Metrics
- Network Traffic

### Prometheus/Grafana

After deploying monitoring stack:
```bash
kubectl port-forward -n aframp-monitoring svc/grafana 3000:80
# Access: http://localhost:3000
```

### Alerts

Configured alerts:
- Health check failures
- High CPU/Memory usage
- Database replication lag
- Redis evictions
- Pod restart rate

## Cost Optimization

### Current Costs
Estimated monthly: ~$4,300

### Optimization Strategies

1. **Use Spot Instances** for non-critical workloads:
   ```hcl
   capacity_type = "SPOT"
   ```

2. **Reserved Instances** for stable workloads (40% savings)

3. **Right-size instances** based on actual usage

4. **Enable cluster autoscaler** to scale down during low traffic

5. **Use S3 lifecycle policies** for log retention

## Troubleshooting

### Terraform Errors

**State Lock Error**:
```bash
# Force unlock (use with caution)
terraform force-unlock LOCK_ID
```

**Provider Authentication**:
```bash
# Verify AWS credentials
aws sts get-caller-identity
```

### EKS Access Issues

**Update kubeconfig**:
```bash
aws eks update-kubeconfig --region af-south-1 --name CLUSTER_NAME
```

**Check IAM permissions**:
```bash
aws eks describe-cluster --name CLUSTER_NAME --region af-south-1
```

### Network Connectivity

**Test VPC peering**:
```bash
# From primary VPC
ping 10.1.1.10  # Lagos VPC
ping 10.2.1.10  # Nairobi VPC
```

**Check security groups**:
```bash
aws ec2 describe-security-groups --filters "Name=tag:Name,Values=*aframp*"
```

## Security Best Practices

1. **Never commit secrets** to version control
2. **Use AWS Secrets Manager** for sensitive data
3. **Enable MFA** for AWS accounts
4. **Rotate credentials** regularly
5. **Review IAM policies** quarterly
6. **Enable CloudTrail** for audit logging
7. **Use KMS** for encryption at rest
8. **Implement least privilege** access

## Support

For issues or questions:
- **Documentation**: `docs/`
- **Runbooks**: `docs/runbooks/`
- **Team**: ops@aframp.com

## License

Proprietary - Aframp Platform
