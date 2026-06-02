# Redis Enterprise Cluster for Multi-Region Caching

# Primary Region Redis
resource "aws_elasticache_replication_group" "redis_primary" {
  provider = aws.primary
  
  replication_group_id       = "${local.cluster_name}-redis-primary"
  replication_group_description = "Redis Enterprise for Aframp - Primary"
  
  engine               = "redis"
  engine_version       = "7.0"
  node_type            = var.redis_node_type
  num_cache_clusters   = 3
  parameter_group_name = aws_elasticache_parameter_group.redis_primary.name
  port                 = 6379
  
  subnet_group_name  = aws_elasticache_subnet_group.redis_primary.name
  security_group_ids = [aws_security_group.redis_primary.id]
  
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  auth_token_enabled         = true
  auth_token                 = random_password.redis_auth.result
  
  automatic_failover_enabled = true
  multi_az_enabled           = true
  
  snapshot_retention_limit = 7
  snapshot_window          = "03:00-05:00"
  maintenance_window       = "sun:05:00-sun:07:00"
  
  notification_topic_arn = aws_sns_topic.redis_alerts.arn
  
  log_delivery_configuration {
    destination      = aws_cloudwatch_log_group.redis_slow_log.name
    destination_type = "cloudwatch-logs"
    log_format       = "json"
    log_type         = "slow-log"
  }
  
  log_delivery_configuration {
    destination      = aws_cloudwatch_log_group.redis_engine_log.name
    destination_type = "cloudwatch-logs"
    log_format       = "json"
    log_type         = "engine-log"
  }
  
  tags = merge(local.common_tags, {
    Name   = "${local.cluster_name}-redis-primary"
    Region = "primary"
  })
}

resource "aws_elasticache_subnet_group" "redis_primary" {
  provider = aws.primary
  
  name       = "${local.cluster_name}-redis-subnet-group"
  subnet_ids = module.vpc_primary.private_subnets
  
  tags = local.common_tags
}

resource "aws_elasticache_parameter_group" "redis_primary" {
  provider = aws.primary
  
  name   = "${local.cluster_name}-redis-params"
  family = "redis7"
  
  # Optimize for high-performance caching
  parameter {
    name  = "maxmemory-policy"
    value = "allkeys-lru"
  }
  
  parameter {
    name  = "timeout"
    value = "300"
  }
  
  parameter {
    name  = "tcp-keepalive"
    value = "300"
  }
  
  parameter {
    name  = "maxmemory-samples"
    value = "10"
  }
  
  tags = local.common_tags
}

resource "aws_security_group" "redis_primary" {
  provider = aws.primary
  
  name_prefix = "${local.cluster_name}-redis-"
  description = "Security group for Redis cluster"
  vpc_id      = module.vpc_primary.vpc_id
  
  ingress {
    description = "Redis from EKS"
    from_port   = 6379
    to_port     = 6379
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr_primary]
  }
  
  egress {
    description = "All outbound"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-redis"
  })
}

# Random password for Redis auth
resource "random_password" "redis_auth" {
  length  = 32
  special = true
}

# Store Redis password in Secrets Manager
resource "aws_secretsmanager_secret" "redis_auth" {
  provider = aws.primary
  
  name_prefix             = "${local.cluster_name}-redis-auth-"
  description             = "Redis authentication token"
  recovery_window_in_days = 7
  
  tags = local.common_tags
}

resource "aws_secretsmanager_secret_version" "redis_auth" {
  provider = aws.primary
  
  secret_id     = aws_secretsmanager_secret.redis_auth.id
  secret_string = random_password.redis_auth.result
}

# CloudWatch Log Groups
resource "aws_cloudwatch_log_group" "redis_slow_log" {
  provider = aws.primary
  
  name              = "/aws/elasticache/${local.cluster_name}-redis/slow-log"
  retention_in_days = 7
  
  tags = local.common_tags
}

resource "aws_cloudwatch_log_group" "redis_engine_log" {
  provider = aws.primary
  
  name              = "/aws/elasticache/${local.cluster_name}-redis/engine-log"
  retention_in_days = 7
  
  tags = local.common_tags
}

# SNS Topic for Redis alerts
resource "aws_sns_topic" "redis_alerts" {
  provider = aws.primary
  
  name = "${local.cluster_name}-redis-alerts"
  
  tags = local.common_tags
}

resource "aws_sns_topic_subscription" "redis_alerts_email" {
  provider = aws.primary
  
  topic_arn = aws_sns_topic.redis_alerts.arn
  protocol  = "email"
  endpoint  = var.alert_email
}

# CloudWatch Alarms for Redis
resource "aws_cloudwatch_metric_alarm" "redis_cpu" {
  provider = aws.primary
  
  alarm_name          = "${local.cluster_name}-redis-high-cpu"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "CPUUtilization"
  namespace           = "AWS/ElastiCache"
  period              = 300
  statistic           = "Average"
  threshold           = 75
  alarm_description   = "Redis CPU utilization is too high"
  alarm_actions       = [aws_sns_topic.redis_alerts.arn]
  
  dimensions = {
    ReplicationGroupId = aws_elasticache_replication_group.redis_primary.id
  }
  
  tags = local.common_tags
}

resource "aws_cloudwatch_metric_alarm" "redis_memory" {
  provider = aws.primary
  
  alarm_name          = "${local.cluster_name}-redis-high-memory"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "DatabaseMemoryUsagePercentage"
  namespace           = "AWS/ElastiCache"
  period              = 300
  statistic           = "Average"
  threshold           = 80
  alarm_description   = "Redis memory utilization is too high"
  alarm_actions       = [aws_sns_topic.redis_alerts.arn]
  
  dimensions = {
    ReplicationGroupId = aws_elasticache_replication_group.redis_primary.id
  }
  
  tags = local.common_tags
}

resource "aws_cloudwatch_metric_alarm" "redis_evictions" {
  provider = aws.primary
  
  alarm_name          = "${local.cluster_name}-redis-evictions"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "Evictions"
  namespace           = "AWS/ElastiCache"
  period              = 300
  statistic           = "Sum"
  threshold           = 1000
  alarm_description   = "Redis is evicting too many keys"
  alarm_actions       = [aws_sns_topic.redis_alerts.arn]
  
  dimensions = {
    ReplicationGroupId = aws_elasticache_replication_group.redis_primary.id
  }
  
  tags = local.common_tags
}

# Lagos Edge Redis
resource "aws_elasticache_replication_group" "redis_lagos" {
  provider = aws.lagos
  
  replication_group_id          = "${local.cluster_name}-redis-lagos"
  replication_group_description = "Redis Enterprise for Aframp - Lagos Edge"
  
  engine               = "redis"
  engine_version       = "7.0"
  node_type            = var.redis_node_type
  num_cache_clusters   = 2
  parameter_group_name = aws_elasticache_parameter_group.redis_lagos.name
  port                 = 6379
  
  subnet_group_name  = aws_elasticache_subnet_group.redis_lagos.name
  security_group_ids = [aws_security_group.redis_lagos.id]
  
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  auth_token_enabled         = true
  auth_token                 = random_password.redis_auth.result
  
  automatic_failover_enabled = true
  multi_az_enabled           = true
  
  snapshot_retention_limit = 3
  
  tags = merge(local.common_tags, {
    Name   = "${local.cluster_name}-redis-lagos"
    Region = "lagos-edge"
  })
}

resource "aws_elasticache_subnet_group" "redis_lagos" {
  provider = aws.lagos
  
  name       = "${local.cluster_name}-redis-lagos-subnet-group"
  subnet_ids = module.vpc_lagos.private_subnets
  
  tags = local.common_tags
}

resource "aws_elasticache_parameter_group" "redis_lagos" {
  provider = aws.lagos
  
  name   = "${local.cluster_name}-redis-lagos-params"
  family = "redis7"
  
  parameter {
    name  = "maxmemory-policy"
    value = "allkeys-lru"
  }
  
  tags = local.common_tags
}

resource "aws_security_group" "redis_lagos" {
  provider = aws.lagos
  
  name_prefix = "${local.cluster_name}-redis-lagos-"
  description = "Security group for Redis cluster - Lagos"
  vpc_id      = module.vpc_lagos.vpc_id
  
  ingress {
    description = "Redis from EKS"
    from_port   = 6379
    to_port     = 6379
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr_lagos]
  }
  
  egress {
    description = "All outbound"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-redis-lagos"
  })
}

# Nairobi Edge Redis
resource "aws_elasticache_replication_group" "redis_nairobi" {
  provider = aws.nairobi
  
  replication_group_id          = "${local.cluster_name}-redis-nairobi"
  replication_group_description = "Redis Enterprise for Aframp - Nairobi Edge"
  
  engine               = "redis"
  engine_version       = "7.0"
  node_type            = var.redis_node_type
  num_cache_clusters   = 2
  parameter_group_name = aws_elasticache_parameter_group.redis_nairobi.name
  port                 = 6379
  
  subnet_group_name  = aws_elasticache_subnet_group.redis_nairobi.name
  security_group_ids = [aws_security_group.redis_nairobi.id]
  
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  auth_token_enabled         = true
  auth_token                 = random_password.redis_auth.result
  
  automatic_failover_enabled = true
  multi_az_enabled           = true
  
  snapshot_retention_limit = 3
  
  tags = merge(local.common_tags, {
    Name   = "${local.cluster_name}-redis-nairobi"
    Region = "nairobi-edge"
  })
}

resource "aws_elasticache_subnet_group" "redis_nairobi" {
  provider = aws.nairobi
  
  name       = "${local.cluster_name}-redis-nairobi-subnet-group"
  subnet_ids = module.vpc_nairobi.private_subnets
  
  tags = local.common_tags
}

resource "aws_elasticache_parameter_group" "redis_nairobi" {
  provider = aws.nairobi
  
  name   = "${local.cluster_name}-redis-nairobi-params"
  family = "redis7"
  
  parameter {
    name  = "maxmemory-policy"
    value = "allkeys-lru"
  }
  
  tags = local.common_tags
}

resource "aws_security_group" "redis_nairobi" {
  provider = aws.nairobi
  
  name_prefix = "${local.cluster_name}-redis-nairobi-"
  description = "Security group for Redis cluster - Nairobi"
  vpc_id      = module.vpc_nairobi.vpc_id
  
  ingress {
    description = "Redis from EKS"
    from_port   = 6379
    to_port     = 6379
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr_nairobi]
  }
  
  egress {
    description = "All outbound"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-redis-nairobi"
  })
}
