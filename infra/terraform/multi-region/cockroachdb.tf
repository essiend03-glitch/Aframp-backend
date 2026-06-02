# CockroachDB Multi-Region Cluster Configuration

# Security group for CockroachDB
resource "aws_security_group" "cockroachdb_primary" {
  provider = aws.primary
  
  name_prefix = "${local.cluster_name}-cockroachdb-"
  description = "Security group for CockroachDB cluster"
  vpc_id      = module.vpc_primary.vpc_id
  
  ingress {
    description = "CockroachDB SQL"
    from_port   = 26257
    to_port     = 26257
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr_primary, var.vpc_cidr_lagos, var.vpc_cidr_nairobi]
  }
  
  ingress {
    description = "CockroachDB HTTP"
    from_port   = 8080
    to_port     = 8080
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr_primary]
  }
  
  ingress {
    description = "CockroachDB inter-node"
    from_port   = 26257
    to_port     = 26257
    protocol    = "tcp"
    self        = true
  }
  
  egress {
    description = "All outbound"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-cockroachdb"
  })
}

# IAM role for CockroachDB instances
resource "aws_iam_role" "cockroachdb" {
  provider = aws.primary
  
  name = "${local.cluster_name}-cockroachdb-role"
  
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "ec2.amazonaws.com"
      }
    }]
  })
  
  tags = local.common_tags
}

resource "aws_iam_role_policy_attachment" "cockroachdb_ssm" {
  provider = aws.primary
  
  role       = aws_iam_role.cockroachdb.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_instance_profile" "cockroachdb" {
  provider = aws.primary
  
  name = "${local.cluster_name}-cockroachdb-profile"
  role = aws_iam_role.cockroachdb.name
  
  tags = local.common_tags
}

# Launch template for CockroachDB nodes
resource "aws_launch_template" "cockroachdb_primary" {
  provider = aws.primary
  
  name_prefix   = "${local.cluster_name}-cockroachdb-"
  image_id      = data.aws_ami.ubuntu_primary.id
  instance_type = var.cockroachdb_instance_type
  
  iam_instance_profile {
    arn = aws_iam_instance_profile.cockroachdb.arn
  }
  
  vpc_security_group_ids = [aws_security_group.cockroachdb_primary.id]
  
  block_device_mappings {
    device_name = "/dev/sda1"
    
    ebs {
      volume_size           = 1000
      volume_type           = "gp3"
      iops                  = 16000
      throughput            = 1000
      encrypted             = true
      kms_key_id            = aws_kms_key.cockroachdb_primary.arn
      delete_on_termination = false
    }
  }
  
  user_data = base64encode(templatefile("${path.module}/templates/cockroachdb-init.sh", {
    cluster_name = local.cluster_name
    region       = "primary"
    join_list    = "cockroachdb-1,cockroachdb-2,cockroachdb-3"
  }))
  
  tag_specifications {
    resource_type = "instance"
    tags = merge(local.common_tags, {
      Name = "${local.cluster_name}-cockroachdb"
      Role = "database"
    })
  }
  
  tags = local.common_tags
}

# Auto Scaling Group for CockroachDB
resource "aws_autoscaling_group" "cockroachdb_primary" {
  provider = aws.primary
  
  name                = "${local.cluster_name}-cockroachdb-asg"
  vpc_zone_identifier = module.vpc_primary.database_subnets
  
  min_size         = 3
  max_size         = 5
  desired_capacity = 3
  
  health_check_type         = "EC2"
  health_check_grace_period = 300
  
  launch_template {
    id      = aws_launch_template.cockroachdb_primary.id
    version = "$Latest"
  }
  
  tag {
    key                 = "Name"
    value               = "${local.cluster_name}-cockroachdb"
    propagate_at_launch = true
  }
  
  tag {
    key                 = "Role"
    value               = "database"
    propagate_at_launch = true
  }
  
  dynamic "tag" {
    for_each = local.common_tags
    content {
      key                 = tag.key
      value               = tag.value
      propagate_at_launch = true
    }
  }
}

# KMS key for CockroachDB encryption
resource "aws_kms_key" "cockroachdb_primary" {
  provider = aws.primary
  
  description             = "CockroachDB Encryption Key - Primary"
  deletion_window_in_days = 7
  enable_key_rotation     = true
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-cockroachdb"
  })
}

resource "aws_kms_alias" "cockroachdb_primary" {
  provider = aws.primary
  
  name          = "alias/${local.cluster_name}-cockroachdb"
  target_key_id = aws_kms_key.cockroachdb_primary.key_id
}

# Data source for Ubuntu AMI
data "aws_ami" "ubuntu_primary" {
  provider = aws.primary
  
  most_recent = true
  owners      = ["099720109477"] # Canonical
  
  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*"]
  }
  
  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

# Network Load Balancer for CockroachDB
resource "aws_lb" "cockroachdb_primary" {
  provider = aws.primary
  
  name               = "${local.cluster_name}-cockroachdb-nlb"
  internal           = true
  load_balancer_type = "network"
  subnets            = module.vpc_primary.database_subnets
  
  enable_cross_zone_load_balancing = true
  
  tags = merge(local.common_tags, {
    Name = "${local.cluster_name}-cockroachdb-nlb"
  })
}

resource "aws_lb_target_group" "cockroachdb_sql" {
  provider = aws.primary
  
  name     = "${local.cluster_name}-cockroachdb-sql"
  port     = 26257
  protocol = "TCP"
  vpc_id   = module.vpc_primary.vpc_id
  
  health_check {
    enabled             = true
    healthy_threshold   = 2
    unhealthy_threshold = 2
    interval            = 10
    port                = 8080
    protocol            = "HTTP"
    path                = "/health?ready=1"
  }
  
  tags = local.common_tags
}

resource "aws_lb_listener" "cockroachdb_sql" {
  provider = aws.primary
  
  load_balancer_arn = aws_lb.cockroachdb_primary.arn
  port              = 26257
  protocol          = "TCP"
  
  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.cockroachdb_sql.arn
  }
}

resource "aws_autoscaling_attachment" "cockroachdb_sql" {
  provider = aws.primary
  
  autoscaling_group_name = aws_autoscaling_group.cockroachdb_primary.name
  lb_target_group_arn    = aws_lb_target_group.cockroachdb_sql.arn
}
