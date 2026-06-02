# Primary EKS Cluster - Cape Town (af-south-1)

module "vpc_primary" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "~> 5.0"
  
  providers = {
    aws = aws.primary
  }
  
  name = "${local.cluster_name}-primary-vpc"
  cidr = var.vpc_cidr_primary
  
  azs             = ["${var.primary_region}a", "${var.primary_region}b", "${var.primary_region}c"]
  private_subnets = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
  public_subnets  = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]
  database_subnets = ["10.0.201.0/24", "10.0.202.0/24", "10.0.203.0/24"]
  
  enable_nat_gateway   = true
  single_nat_gateway   = false
  enable_dns_hostnames = true
  enable_dns_support   = true
  
  enable_flow_log                      = true
  create_flow_log_cloudwatch_iam_role  = true
  create_flow_log_cloudwatch_log_group = true
  
  public_subnet_tags = {
    "kubernetes.io/role/elb" = "1"
    "kubernetes.io/cluster/${local.cluster_name}-primary" = "shared"
  }
  
  private_subnet_tags = {
    "kubernetes.io/role/internal-elb" = "1"
    "kubernetes.io/cluster/${local.cluster_name}-primary" = "shared"
  }
  
  tags = merge(local.common_tags, {
    Region = "primary"
  })
}

module "eks_primary" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 19.0"
  
  providers = {
    aws = aws.primary
  }
  
  cluster_name    = "${local.cluster_name}-primary"
  cluster_version = var.cluster_version
  
  vpc_id     = module.vpc_primary.vpc_id
  subnet_ids = module.vpc_primary.private_subnets
  
  cluster_endpoint_public_access  = true
  cluster_endpoint_private_access = true
  
  # Enable IRSA for pod-level IAM
  enable_irsa = true
  
  # Cluster encryption
  cluster_encryption_config = {
    resources        = ["secrets"]
    provider_key_arn = aws_kms_key.eks_primary.arn
  }
  
  # Cluster addons
  cluster_addons = {
    coredns = {
      most_recent = true
    }
    kube-proxy = {
      most_recent = true
    }
    vpc-cni = {
      most_recent = true
    }
    aws-ebs-csi-driver = {
      most_recent = true
    }
  }
  
  # Node groups
  eks_managed_node_groups = {
    # General purpose nodes
    general = {
      name = "general-purpose"
      
      instance_types = var.node_instance_types
      capacity_type  = "ON_DEMAND"
      
      min_size     = var.min_nodes
      max_size     = var.max_nodes
      desired_size = var.desired_nodes
      
      labels = {
        role = "general"
      }
      
      taints = []
      
      block_device_mappings = {
        xvda = {
          device_name = "/dev/xvda"
          ebs = {
            volume_size           = 100
            volume_type           = "gp3"
            iops                  = 3000
            throughput            = 150
            encrypted             = true
            kms_key_id            = aws_kms_key.eks_primary.arn
            delete_on_termination = true
          }
        }
      }
    }
    
    # Database workload nodes
    database = {
      name = "database-workload"
      
      instance_types = ["r6i.2xlarge"]
      capacity_type  = "ON_DEMAND"
      
      min_size     = 3
      max_size     = 6
      desired_size = 3
      
      labels = {
        role = "database"
        workload = "cockroachdb"
      }
      
      taints = [{
        key    = "workload"
        value  = "database"
        effect = "NO_SCHEDULE"
      }]
      
      block_device_mappings = {
        xvda = {
          device_name = "/dev/xvda"
          ebs = {
            volume_size           = 500
            volume_type           = "gp3"
            iops                  = 16000
            throughput            = 1000
            encrypted             = true
            kms_key_id            = aws_kms_key.eks_primary.arn
            delete_on_termination = true
          }
        }
      }
    }
  }
  
  # Cluster security group rules
  cluster_security_group_additional_rules = {
    ingress_nodes_ephemeral_ports_tcp = {
      description                = "Nodes on ephemeral ports"
      protocol                   = "tcp"
      from_port                  = 1025
      to_port                    = 65535
      type                       = "ingress"
      source_node_security_group = true
    }
  }
  
  # Node security group rules
  node_security_group_additional_rules = {
    ingress_self_all = {
      description = "Node to node all ports/protocols"
      protocol    = "-1"
      from_port   = 0
      to_port     = 0
      type        = "ingress"
      self        = true
    }
    
    ingress_cluster_all = {
      description                   = "Cluster to node all ports/protocols"
      protocol                      = "-1"
      from_port                     = 0
      to_port                       = 0
      type                          = "ingress"
      source_cluster_security_group = true
    }
    
    egress_all = {
      description      = "Node all egress"
      protocol         = "-1"
      from_port        = 0
      to_port          = 0
      type             = "egress"
      cidr_blocks      = ["0.0.0.0/0"]
      ipv6_cidr_blocks = ["::/0"]
    }
  }
  
  tags = merge(local.common_tags, {
    Region = "primary"
  })
}

# KMS key for EKS encryption
resource "aws_kms_key" "eks_primary" {
  provider = aws.primary
  
  description             = "EKS Secret Encryption Key - Primary"
  deletion_window_in_days = 7
  enable_key_rotation     = true
  
  tags = merge(local.common_tags, {
    Name   = "${local.cluster_name}-primary-eks"
    Region = "primary"
  })
}

resource "aws_kms_alias" "eks_primary" {
  provider = aws.primary
  
  name          = "alias/${local.cluster_name}-primary-eks"
  target_key_id = aws_kms_key.eks_primary.key_id
}
