# Terraform Outputs

# EKS Cluster Outputs
output "eks_primary_cluster_id" {
  description = "EKS cluster ID - Primary"
  value       = module.eks_primary.cluster_id
}

output "eks_primary_cluster_endpoint" {
  description = "EKS cluster endpoint - Primary"
  value       = module.eks_primary.cluster_endpoint
}

output "eks_primary_cluster_certificate_authority_data" {
  description = "EKS cluster certificate authority data - Primary"
  value       = module.eks_primary.cluster_certificate_authority_data
  sensitive   = true
}

output "eks_lagos_cluster_id" {
  description = "EKS cluster ID - Lagos"
  value       = module.eks_lagos.cluster_id
}

output "eks_lagos_cluster_endpoint" {
  description = "EKS cluster endpoint - Lagos"
  value       = module.eks_lagos.cluster_endpoint
}

output "eks_nairobi_cluster_id" {
  description = "EKS cluster ID - Nairobi"
  value       = module.eks_nairobi.cluster_id
}

output "eks_nairobi_cluster_endpoint" {
  description = "EKS cluster endpoint - Nairobi"
  value       = module.eks_nairobi.cluster_endpoint
}

# Database Outputs
output "cockroachdb_endpoint" {
  description = "CockroachDB cluster endpoint"
  value       = aws_lb.cockroachdb_primary.dns_name
}

output "cockroachdb_internal_dns" {
  description = "CockroachDB internal DNS name"
  value       = aws_route53_record.cockroachdb.fqdn
}

# Redis Outputs
output "redis_primary_endpoint" {
  description = "Redis primary endpoint"
  value       = aws_elasticache_replication_group.redis_primary.primary_endpoint_address
  sensitive   = true
}

output "redis_lagos_endpoint" {
  description = "Redis Lagos endpoint"
  value       = aws_elasticache_replication_group.redis_lagos.primary_endpoint_address
  sensitive   = true
}

output "redis_nairobi_endpoint" {
  description = "Redis Nairobi endpoint"
  value       = aws_elasticache_replication_group.redis_nairobi.primary_endpoint_address
  sensitive   = true
}

output "redis_auth_token_secret_arn" {
  description = "ARN of Redis auth token in Secrets Manager"
  value       = aws_secretsmanager_secret.redis_auth.arn
}

# Networking Outputs
output "vpc_primary_id" {
  description = "VPC ID - Primary"
  value       = module.vpc_primary.vpc_id
}

output "vpc_lagos_id" {
  description = "VPC ID - Lagos"
  value       = module.vpc_lagos.vpc_id
}

output "vpc_nairobi_id" {
  description = "VPC ID - Nairobi"
  value       = module.vpc_nairobi.vpc_id
}

output "internal_hosted_zone_id" {
  description = "Route53 internal hosted zone ID"
  value       = aws_route53_zone.internal.zone_id
}

output "public_hosted_zone_id" {
  description = "Route53 public hosted zone ID"
  value       = aws_route53_zone.public.zone_id
}

output "public_hosted_zone_nameservers" {
  description = "Route53 public hosted zone nameservers"
  value       = aws_route53_zone.public.name_servers
}

# Security Outputs
output "kms_key_eks_primary_arn" {
  description = "KMS key ARN for EKS encryption - Primary"
  value       = aws_kms_key.eks_primary.arn
}

output "kms_key_cockroachdb_arn" {
  description = "KMS key ARN for CockroachDB encryption"
  value       = aws_kms_key.cockroachdb_primary.arn
}

# Monitoring Outputs
output "sns_alerts_topic_arn" {
  description = "SNS topic ARN for infrastructure alerts"
  value       = aws_sns_topic.alerts.arn
}

output "waf_web_acl_arn" {
  description = "WAF Web ACL ARN"
  value       = aws_wafv2_web_acl.main.arn
}

# Health Check Outputs
output "health_check_primary_id" {
  description = "Route53 health check ID - Primary"
  value       = aws_route53_health_check.primary.id
}

output "health_check_lagos_id" {
  description = "Route53 health check ID - Lagos"
  value       = aws_route53_health_check.lagos.id
}

output "health_check_nairobi_id" {
  description = "Route53 health check ID - Nairobi"
  value       = aws_route53_health_check.nairobi.id
}

# Kubectl Configuration Commands
output "kubectl_config_command_primary" {
  description = "Command to configure kubectl for primary cluster"
  value       = "aws eks update-kubeconfig --region ${var.primary_region} --name ${module.eks_primary.cluster_name}"
}

output "kubectl_config_command_lagos" {
  description = "Command to configure kubectl for Lagos cluster"
  value       = "aws eks update-kubeconfig --region ${var.lagos_region} --name ${module.eks_lagos.cluster_name}"
}

output "kubectl_config_command_nairobi" {
  description = "Command to configure kubectl for Nairobi cluster"
  value       = "aws eks update-kubeconfig --region ${var.nairobi_region} --name ${module.eks_nairobi.cluster_name}"
}
