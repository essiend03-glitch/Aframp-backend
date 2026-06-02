# Variable definitions for multi-region deployment

variable "environment" {
  description = "Environment name"
  type        = string
  default     = "production"
}

variable "primary_region" {
  description = "Primary AWS region (Cape Town)"
  type        = string
  default     = "af-south-1"
}

variable "lagos_region" {
  description = "Lagos edge region (EU West closest to West Africa)"
  type        = string
  default     = "eu-west-1"
}

variable "nairobi_region" {
  description = "Nairobi edge region (EU Central closest to East Africa)"
  type        = string
  default     = "eu-central-1"
}

variable "cluster_version" {
  description = "Kubernetes cluster version"
  type        = string
  default     = "1.28"
}

variable "node_instance_types" {
  description = "EC2 instance types for worker nodes"
  type        = list(string)
  default     = ["t3.xlarge", "t3.2xlarge"]
}

variable "min_nodes" {
  description = "Minimum number of nodes per cluster"
  type        = number
  default     = 3
}

variable "max_nodes" {
  description = "Maximum number of nodes per cluster"
  type        = number
  default     = 10
}

variable "desired_nodes" {
  description = "Desired number of nodes per cluster"
  type        = number
  default     = 5
}

variable "vpc_cidr_primary" {
  description = "CIDR block for primary VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "vpc_cidr_lagos" {
  description = "CIDR block for Lagos VPC"
  type        = string
  default     = "10.1.0.0/16"
}

variable "vpc_cidr_nairobi" {
  description = "CIDR block for Nairobi VPC"
  type        = string
  default     = "10.2.0.0/16"
}

variable "cockroachdb_instance_type" {
  description = "Instance type for CockroachDB nodes"
  type        = string
  default     = "r6i.2xlarge"
}

variable "redis_node_type" {
  description = "Node type for Redis Enterprise"
  type        = string
  default     = "cache.r6g.xlarge"
}

variable "enable_vpn_tunnels" {
  description = "Enable WireGuard VPN tunnels between regions"
  type        = bool
  default     = true
}

variable "enable_istio" {
  description = "Enable Istio service mesh for zero-trust"
  type        = bool
  default     = true
}

variable "cloudflare_enabled" {
  description = "Use Cloudflare for global load balancing"
  type        = bool
  default     = true
}

variable "domain_name" {
  description = "Primary domain name"
  type        = string
  default     = "api.aframp.com"
}

variable "alert_email" {
  description = "Email for critical alerts"
  type        = string
}

variable "slack_webhook_url" {
  description = "Slack webhook for alerts"
  type        = string
  sensitive   = true
}

variable "pagerduty_integration_key" {
  description = "PagerDuty integration key"
  type        = string
  sensitive   = true
}
