# Multi-Region Production Infrastructure
# Optimized for sub-Saharan Africa deployment

terraform {
  required_version = ">= 1.5.0"
  
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.11"
    }
  }
  
  backend "s3" {
    bucket         = "aframp-terraform-state"
    key            = "production/multi-region/terraform.tfstate"
    region         = "af-south-1"
    encrypt        = true
    dynamodb_table = "terraform-state-lock"
  }
}

# Primary Region: Cape Town
provider "aws" {
  alias  = "primary"
  region = var.primary_region
  
  default_tags {
    tags = {
      Environment = "production"
      Project     = "aframp"
      ManagedBy   = "terraform"
      Region      = "primary"
    }
  }
}

# Edge Region: Lagos (simulated via eu-west-1 with edge routing)
provider "aws" {
  alias  = "lagos"
  region = var.lagos_region
  
  default_tags {
    tags = {
      Environment = "production"
      Project     = "aframp"
      ManagedBy   = "terraform"
      Region      = "lagos-edge"
    }
  }
}

# Edge Region: Nairobi (simulated via eu-central-1 with edge routing)
provider "aws" {
  alias  = "nairobi"
  region = var.nairobi_region
  
  default_tags {
    tags = {
      Environment = "production"
      Project     = "aframp"
      ManagedBy   = "terraform"
      Region      = "nairobi-edge"
    }
  }
}

# Local variables
locals {
  cluster_name = "aframp-${var.environment}"
  
  common_tags = {
    Environment = var.environment
    Project     = "aframp"
    ManagedBy   = "terraform"
  }
}
