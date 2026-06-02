# Global Load Balancing with Route 53 and Cloudflare

# Route 53 Public Hosted Zone
resource "aws_route53_zone" "public" {
  provider = aws.primary
  
  name = var.domain_name
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-public-zone"
  })
}

# Health checks for each region
resource "aws_route53_health_check" "primary" {
  provider = aws.primary
  
  fqdn              = "api-primary.${var.domain_name}"
  port              = 443
  type              = "HTTPS"
  resource_path     = "/health"
  failure_threshold = 3
  request_interval  = 30
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-health-primary"
  })
}

resource "aws_route53_health_check" "lagos" {
  provider = aws.primary
  
  fqdn              = "api-lagos.${var.domain_name}"
  port              = 443
  type              = "HTTPS"
  resource_path     = "/health"
  failure_threshold = 3
  request_interval  = 30
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-health-lagos"
  })
}

resource "aws_route53_health_check" "nairobi" {
  provider = aws.primary
  
  fqdn              = "api-nairobi.${var.domain_name}"
  port              = 443
  type              = "HTTPS"
  resource_path     = "/health"
  failure_threshold = 3
  request_interval  = 30
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-health-nairobi"
  })
}

# Latency-based routing records
resource "aws_route53_record" "api_primary" {
  provider = aws.primary
  
  zone_id = aws_route53_zone.public.zone_id
  name    = var.domain_name
  type    = "A"
  
  set_identifier = "primary"
  latency_routing_policy {
    region = var.primary_region
  }
  
  health_check_id = aws_route53_health_check.primary.id
  
  alias {
    name                   = module.eks_primary.cluster_endpoint
    zone_id                = module.eks_primary.cluster_arn
    evaluate_target_health = true
  }
}

resource "aws_route53_record" "api_lagos" {
  provider = aws.primary
  
  zone_id = aws_route53_zone.public.zone_id
  name    = var.domain_name
  type    = "A"
  
  set_identifier = "lagos"
  latency_routing_policy {
    region = var.lagos_region
  }
  
  health_check_id = aws_route53_health_check.lagos.id
  
  alias {
    name                   = module.eks_lagos.cluster_endpoint
    zone_id                = module.eks_lagos.cluster_arn
    evaluate_target_health = true
  }
}

resource "aws_route53_record" "api_nairobi" {
  provider = aws.primary
  
  zone_id = aws_route53_zone.public.zone_id
  name    = var.domain_name
  type    = "A"
  
  set_identifier = "nairobi"
  latency_routing_policy {
    region = var.nairobi_region
  }
  
  health_check_id = aws_route53_health_check.nairobi.id
  
  alias {
    name                   = module.eks_nairobi.cluster_endpoint
    zone_id                = module.eks_nairobi.cluster_arn
    evaluate_target_health = true
  }
}

# CloudWatch alarms for health checks
resource "aws_cloudwatch_metric_alarm" "health_check_primary" {
  provider = aws.primary
  
  alarm_name          = "${local.cluster_name}-health-check-primary-failed"
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = 2
  metric_name         = "HealthCheckStatus"
  namespace           = "AWS/Route53"
  period              = 60
  statistic           = "Minimum"
  threshold           = 1
  alarm_description   = "Primary region health check failed"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    HealthCheckId = aws_route53_health_check.primary.id
  }
  
  tags = local.common_tags
}

resource "aws_cloudwatch_metric_alarm" "health_check_lagos" {
  provider = aws.primary
  
  alarm_name          = "${local.cluster_name}-health-check-lagos-failed"
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = 2
  metric_name         = "HealthCheckStatus"
  namespace           = "AWS/Route53"
  period              = 60
  statistic           = "Minimum"
  threshold           = 1
  alarm_description   = "Lagos region health check failed"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    HealthCheckId = aws_route53_health_check.lagos.id
  }
  
  tags = local.common_tags
}

resource "aws_cloudwatch_metric_alarm" "health_check_nairobi" {
  provider = aws.primary
  
  alarm_name          = "${local.cluster_name}-health-check-nairobi-failed"
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = 2
  metric_name         = "HealthCheckStatus"
  namespace           = "AWS/Route53"
  period              = 60
  statistic           = "Minimum"
  threshold           = 1
  alarm_description   = "Nairobi region health check failed"
  alarm_actions       = [aws_sns_topic.alerts.arn]
  
  dimensions = {
    HealthCheckId = aws_route53_health_check.nairobi.id
  }
  
  tags = local.common_tags
}

# SNS Topic for alerts
resource "aws_sns_topic" "alerts" {
  provider = aws.primary
  
  name = "${local.cluster_name}-infrastructure-alerts"
  
  tags = local.common_tags
}

resource "aws_sns_topic_subscription" "alerts_email" {
  provider = aws.primary
  
  topic_arn = aws_sns_topic.alerts.arn
  protocol  = "email"
  endpoint  = var.alert_email
}

# WAF for DDoS protection
resource "aws_wafv2_web_acl" "main" {
  provider = aws.primary
  
  name  = "${local.cluster_name}-waf"
  scope = "REGIONAL"
  
  default_action {
    allow {}
  }
  
  # Rate limiting rule
  rule {
    name     = "RateLimitRule"
    priority = 1
    
    action {
      block {}
    }
    
    statement {
      rate_based_statement {
        limit              = 2000
        aggregate_key_type = "IP"
      }
    }
    
    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "${local.cluster_name}-rate-limit"
      sampled_requests_enabled   = true
    }
  }
  
  # AWS Managed Rules - Core Rule Set
  rule {
    name     = "AWSManagedRulesCommonRuleSet"
    priority = 2
    
    override_action {
      none {}
    }
    
    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesCommonRuleSet"
        vendor_name = "AWS"
      }
    }
    
    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "${local.cluster_name}-common-rules"
      sampled_requests_enabled   = true
    }
  }
  
  # AWS Managed Rules - Known Bad Inputs
  rule {
    name     = "AWSManagedRulesKnownBadInputsRuleSet"
    priority = 3
    
    override_action {
      none {}
    }
    
    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesKnownBadInputsRuleSet"
        vendor_name = "AWS"
      }
    }
    
    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "${local.cluster_name}-bad-inputs"
      sampled_requests_enabled   = true
    }
  }
  
  visibility_config {
    cloudwatch_metrics_enabled = true
    metric_name                = "${local.cluster_name}-waf"
    sampled_requests_enabled   = true
  }
  
  tags = local.common_tags
}
