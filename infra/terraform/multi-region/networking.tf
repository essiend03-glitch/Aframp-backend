# Multi-Region Networking and VPN Tunnels

# VPC Peering: Primary <-> Lagos
resource "aws_vpc_peering_connection" "primary_lagos" {
  provider = aws.primary
  
  vpc_id      = module.vpc_primary.vpc_id
  peer_vpc_id = module.vpc_lagos.vpc_id
  peer_region = var.lagos_region
  auto_accept = false
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-primary-lagos-peering"
  })
}

resource "aws_vpc_peering_connection_accepter" "primary_lagos" {
  provider = aws.lagos
  
  vpc_peering_connection_id = aws_vpc_peering_connection.primary_lagos.id
  auto_accept               = true
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-primary-lagos-peering"
  })
}

# VPC Peering: Primary <-> Nairobi
resource "aws_vpc_peering_connection" "primary_nairobi" {
  provider = aws.primary
  
  vpc_id      = module.vpc_primary.vpc_id
  peer_vpc_id = module.vpc_nairobi.vpc_id
  peer_region = var.nairobi_region
  auto_accept = false
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-primary-nairobi-peering"
  })
}

resource "aws_vpc_peering_connection_accepter" "primary_nairobi" {
  provider = aws.nairobi
  
  vpc_peering_connection_id = aws_vpc_peering_connection.primary_nairobi.id
  auto_accept               = true
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-primary-nairobi-peering"
  })
}

# Route table entries for peering connections
resource "aws_route" "primary_to_lagos" {
  provider = aws.primary
  
  count = length(module.vpc_primary.private_route_table_ids)
  
  route_table_id            = module.vpc_primary.private_route_table_ids[count.index]
  destination_cidr_block    = var.vpc_cidr_lagos
  vpc_peering_connection_id = aws_vpc_peering_connection.primary_lagos.id
}

resource "aws_route" "lagos_to_primary" {
  provider = aws.lagos
  
  count = length(module.vpc_lagos.private_route_table_ids)
  
  route_table_id            = module.vpc_lagos.private_route_table_ids[count.index]
  destination_cidr_block    = var.vpc_cidr_primary
  vpc_peering_connection_id = aws_vpc_peering_connection.primary_lagos.id
}

resource "aws_route" "primary_to_nairobi" {
  provider = aws.primary
  
  count = length(module.vpc_primary.private_route_table_ids)
  
  route_table_id            = module.vpc_primary.private_route_table_ids[count.index]
  destination_cidr_block    = var.vpc_cidr_nairobi
  vpc_peering_connection_id = aws_vpc_peering_connection.primary_nairobi.id
}

resource "aws_route" "nairobi_to_primary" {
  provider = aws.nairobi
  
  count = length(module.vpc_nairobi.private_route_table_ids)
  
  route_table_id            = module.vpc_nairobi.private_route_table_ids[count.index]
  destination_cidr_block    = var.vpc_cidr_primary
  vpc_peering_connection_id = aws_vpc_peering_connection.primary_nairobi.id
}

# Transit Gateway for multi-region connectivity (optional, more scalable)
resource "aws_ec2_transit_gateway" "primary" {
  provider = aws.primary
  
  description                     = "Transit Gateway for ${local.cluster_name}"
  default_route_table_association = "enable"
  default_route_table_propagation = "enable"
  dns_support                     = "enable"
  vpn_ecmp_support                = "enable"
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-tgw-primary"
  })
}

# Attach VPCs to Transit Gateway
resource "aws_ec2_transit_gateway_vpc_attachment" "primary" {
  provider = aws.primary
  
  subnet_ids         = module.vpc_primary.private_subnets
  transit_gateway_id = aws_ec2_transit_gateway.primary.id
  vpc_id             = module.vpc_primary.vpc_id
  
  dns_support  = "enable"
  ipv6_support = "disable"
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-tgw-attachment-primary"
  })
}

# Route 53 Private Hosted Zone for internal DNS
resource "aws_route53_zone" "internal" {
  provider = aws.primary
  
  name = "aframp.internal"
  
  vpc {
    vpc_id = module.vpc_primary.vpc_id
  }
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-internal-zone"
  })
}

# Associate hosted zone with other VPCs
resource "aws_route53_zone_association" "lagos" {
  provider = aws.lagos
  
  zone_id = aws_route53_zone.internal.zone_id
  vpc_id  = module.vpc_lagos.vpc_id
}

resource "aws_route53_zone_association" "nairobi" {
  provider = aws.nairobi
  
  zone_id = aws_route53_zone.internal.zone_id
  vpc_id  = module.vpc_nairobi.vpc_id
}

# DNS records for CockroachDB
resource "aws_route53_record" "cockroachdb" {
  provider = aws.primary
  
  zone_id = aws_route53_zone.internal.zone_id
  name    = "cockroachdb.aframp.internal"
  type    = "A"
  
  alias {
    name                   = aws_lb.cockroachdb_primary.dns_name
    zone_id                = aws_lb.cockroachdb_primary.zone_id
    evaluate_target_health = true
  }
}

# DNS records for Redis
resource "aws_route53_record" "redis_primary" {
  provider = aws.primary
  
  zone_id = aws_route53_zone.internal.zone_id
  name    = "redis-primary.aframp.internal"
  type    = "CNAME"
  ttl     = 300
  records = [aws_elasticache_replication_group.redis_primary.primary_endpoint_address]
}

resource "aws_route53_record" "redis_lagos" {
  provider = aws.primary
  
  zone_id = aws_route53_zone.internal.zone_id
  name    = "redis-lagos.aframp.internal"
  type    = "CNAME"
  ttl     = 300
  records = [aws_elasticache_replication_group.redis_lagos.primary_endpoint_address]
}

resource "aws_route53_record" "redis_nairobi" {
  provider = aws.primary
  
  zone_id = aws_route53_zone.internal.zone_id
  name    = "redis-nairobi.aframp.internal"
  type    = "CNAME"
  ttl     = 300
  records = [aws_elasticache_replication_group.redis_nairobi.primary_endpoint_address]
}

# Network ACLs for additional security
resource "aws_network_acl" "database_primary" {
  provider = aws.primary
  
  vpc_id     = module.vpc_primary.vpc_id
  subnet_ids = module.vpc_primary.database_subnets
  
  # Allow inbound from private subnets
  ingress {
    protocol   = "tcp"
    rule_no    = 100
    action     = "allow"
    cidr_block = var.vpc_cidr_primary
    from_port  = 26257
    to_port    = 26257
  }
  
  ingress {
    protocol   = "tcp"
    rule_no    = 110
    action     = "allow"
    cidr_block = var.vpc_cidr_lagos
    from_port  = 26257
    to_port    = 26257
  }
  
  ingress {
    protocol   = "tcp"
    rule_no    = 120
    action     = "allow"
    cidr_block = var.vpc_cidr_nairobi
    from_port  = 26257
    to_port    = 26257
  }
  
  # Allow return traffic
  ingress {
    protocol   = "tcp"
    rule_no    = 200
    action     = "allow"
    cidr_block = "0.0.0.0/0"
    from_port  = 1024
    to_port    = 65535
  }
  
  # Allow all outbound
  egress {
    protocol   = "-1"
    rule_no    = 100
    action     = "allow"
    cidr_block = "0.0.0.0/0"
    from_port  = 0
    to_port    = 0
  }
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-database-nacl"
  })
}
