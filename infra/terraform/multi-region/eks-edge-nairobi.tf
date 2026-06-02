# Edge EKS Cluster - Nairobi (eu-central-1 with edge routing)

module "vpc_nairobi" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "~> 5.0"
  
  providers = {
    aws = aws.nairobi
  }
  
  name = "${local.cluster_name}-nairobi-vpc"
  cidr = var.vpc_cidr_nairobi
  
  azs             = ["${var.nairobi_region}a", "${var.nairobi_region}b", "${var.nairobi_region}c"]
  private_subnets = ["10.2.1.0/24", "10.2.2.0/24", "10.2.3.0/24"]
  public_subnets  = ["10.2.101.0/24", "10.2.102.0/24", "10.2.103.0/24"]
  
  enable_nat_gateway   = true
  single_nat_gateway   = false
  enable_dns_hostnames = true
  enable_dns_support   = true
  
  enable_flow_log                      = true
  create_flow_log_cloudwatch_iam_role  = true
  create_flow_log_cloudwatch_log_group = true
  
  public_subnet_tags = {
    "kubernetes.io/role/elb" = "1"
    "kubernetes.io/cluster/${local.cluster_name}-nairobi" = "shared"
  }
  
  private_subnet_tags = {
    "kubernetes.io/role/internal-elb" = "1"
    "kubernetes.io/cluster/${local.cluster_name}-nairobi" = "shared"
  }
  
  tags = merge(local.common_tags, {
    Region = "nairobi-edge"
  })
}

module "eks_nairobi" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 19.0"
  
  providers = {
    aws = aws.nairobi
  }
  
  cluster_name    = "${local.cluster_name}-nairobi"
  cluster_version = var.cluster_version
  
  vpc_id     = module.vpc_nairobi.vpc_id
  subnet_ids = module.vpc_nairobi.private_subnets
  
  cluster_endpoint_public_access  = true
  cluster_endpoint_private_access = true
  
  enable_irsa = true
  
  cluster_encryption_config = {
    resources        = ["secrets"]
    provider_key_arn = aws_kms_key.eks_nairobi.arn
  }
  
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
  
  eks_managed_node_groups = {
    api_gateway = {
      name = "api-gateway"
      
      instance_types = ["c6i.2xlarge"]
      capacity_type  = "ON_DEMAND"
      
      min_size     = 2
      max_size     = 8
      desired_size = 3
      
      labels = {
        role = "api-gateway"
        region = "nairobi"
      }
      
      block_device_mappings = {
        xvda = {
          device_name = "/dev/xvda"
          ebs = {
            volume_size           = 100
            volume_type           = "gp3"
            iops                  = 3000
            throughput            = 150
            encrypted             = true
            kms_key_id            = aws_kms_key.eks_nairobi.arn
            delete_on_termination = true
          }
        }
      }
    }
  }
  
  tags = merge(local.common_tags, {
    Region = "nairobi-edge"
  })
}

resource "aws_kms_key" "eks_nairobi" {
  provider = aws.nairobi
  
  description             = "EKS Secret Encryption Key - Nairobi"
  deletion_window_in_days = 7
  enable_key_rotation     = true
  
  tags = merge(local.common_tags, {
    Name   = "${local.cluster_name}-nairobi-eks"
    Region = "nairobi-edge"
  })
}

resource "aws_kms_alias" "eks_nairobi" {
  provider = aws.nairobi
  
  name          = "alias/${local.cluster_name}-nairobi-eks"
  target_key_id = aws_kms_key.eks_nairobi.key_id
}
