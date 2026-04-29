# infra/terraform/global_lb.tf — Issue #348
# Route 53 latency-based routing (Anycast-style) across three regions.
# Each region has its own ALB; Route 53 routes to the lowest-latency endpoint.
# Health checks use /health/edge — returns 503 when replication lag > 5 s,
# triggering automatic DNS failover to the next closest region.

variable "hosted_zone_id" {
  description = "Route 53 hosted zone ID for aframp.io"
  type        = string
}

variable "regions" {
  description = "Map of region → ALB DNS name"
  type        = map(string)
  default = {
    "us-east-1"    = "aframp-alb-use1.example.com"
    "eu-west-1"    = "aframp-alb-euw1.example.com"
    "ap-southeast-1" = "aframp-alb-apse1.example.com"
  }
}

# ---------------------------------------------------------------------------
# Route 53 health checks — one per region, targeting /health/edge
# ---------------------------------------------------------------------------

resource "aws_route53_health_check" "edge" {
  for_each = var.regions

  fqdn              = each.value
  port              = 443
  type              = "HTTPS"
  resource_path     = "/health/edge"
  failure_threshold = 2
  request_interval  = 10

  tags = { Region = each.key, Project = "aframp", Issue = "348" }
}

# ---------------------------------------------------------------------------
# Latency-based DNS records — api.aframp.io
# ---------------------------------------------------------------------------

resource "aws_route53_record" "api_latency" {
  for_each = var.regions

  zone_id        = var.hosted_zone_id
  name           = "api.aframp.io"
  type           = "CNAME"
  ttl            = 60
  set_identifier = each.key
  records        = [each.value]

  latency_routing_policy {
    region = each.key
  }

  health_check_id = aws_route53_health_check.edge[each.key].id
}

# ---------------------------------------------------------------------------
# Outputs
# ---------------------------------------------------------------------------

output "health_check_ids" {
  value = { for k, v in aws_route53_health_check.edge : k => v.id }
}
