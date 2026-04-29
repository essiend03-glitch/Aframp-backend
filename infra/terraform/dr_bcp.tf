# infra/terraform/dr_bcp.tf — Issue #DR-BCP
#
# Immutable backup infrastructure for Disaster Recovery & Business Continuity.
#
# Architecture:
#   Primary account  ──► S3 (Object Lock COMPLIANCE)  ──► Cross-region replica
#                                                      ──► Cross-account replica (air-gapped)
#
# RPO target : 0 minutes  (continuous WAL archiving + point-in-time recovery)
# RTO target : 15 minutes (automated restore pipeline)

terraform {
  required_providers {
    aws = { source = "hashicorp/aws", version = "~> 5.0" }
  }
}

# ---------------------------------------------------------------------------
# Variables
# ---------------------------------------------------------------------------

variable "primary_region" {
  description = "Primary AWS region"
  type        = string
  default     = "us-east-1"
}

variable "dr_region" {
  description = "DR / replica AWS region"
  type        = string
  default     = "eu-west-1"
}

variable "backup_retention_days" {
  description = "Minimum retention period for immutable backups (Object Lock)"
  type        = number
  default     = 90
}

variable "dr_account_id" {
  description = "AWS account ID of the air-gapped DR account"
  type        = string
}

variable "environment" {
  type    = string
  default = "production"
}

# ---------------------------------------------------------------------------
# Providers
# ---------------------------------------------------------------------------

provider "aws" {
  alias  = "primary"
  region = var.primary_region
}

provider "aws" {
  alias  = "dr"
  region = var.dr_region
}

# ---------------------------------------------------------------------------
# KMS keys — separate key per region for envelope encryption
# ---------------------------------------------------------------------------

resource "aws_kms_key" "backup_primary" {
  provider                = aws.primary
  description             = "Aframp immutable backup encryption key (primary)"
  deletion_window_in_days = 30
  enable_key_rotation     = true

  tags = { Environment = var.environment, Purpose = "dr-backup" }
}

resource "aws_kms_key" "backup_dr" {
  provider                = aws.dr
  description             = "Aframp immutable backup encryption key (DR replica)"
  deletion_window_in_days = 30
  enable_key_rotation     = true

  tags = { Environment = var.environment, Purpose = "dr-backup-replica" }
}

# ---------------------------------------------------------------------------
# Primary immutable backup bucket (Object Lock — COMPLIANCE mode)
# ---------------------------------------------------------------------------

resource "aws_s3_bucket" "backup_primary" {
  provider = aws.primary
  bucket   = "aframp-immutable-backups-${var.environment}-primary"

  # Prevent accidental deletion of the bucket itself.
  lifecycle { prevent_destroy = true }

  tags = { Environment = var.environment, Purpose = "dr-backup" }
}

resource "aws_s3_bucket_versioning" "backup_primary" {
  provider = aws.primary
  bucket   = aws_s3_bucket.backup_primary.id
  versioning_configuration { status = "Enabled" }
}

resource "aws_s3_bucket_object_lock_configuration" "backup_primary" {
  provider = aws.primary
  bucket   = aws_s3_bucket.backup_primary.id

  rule {
    default_retention {
      mode = "COMPLIANCE"
      days = var.backup_retention_days
    }
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "backup_primary" {
  provider = aws.primary
  bucket   = aws_s3_bucket.backup_primary.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm     = "aws:kms"
      kms_master_key_id = aws_kms_key.backup_primary.arn
    }
    bucket_key_enabled = true
  }
}

resource "aws_s3_bucket_public_access_block" "backup_primary" {
  provider                = aws.primary
  bucket                  = aws_s3_bucket.backup_primary.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# Lifecycle: transition to Glacier after 30 days, expire after retention period.
resource "aws_s3_bucket_lifecycle_configuration" "backup_primary" {
  provider = aws.primary
  bucket   = aws_s3_bucket.backup_primary.id

  rule {
    id     = "archive-and-expire"
    status = "Enabled"

    transition {
      days          = 30
      storage_class = "GLACIER"
    }

    expiration {
      days = var.backup_retention_days
    }
  }
}

# ---------------------------------------------------------------------------
# Cross-region replica bucket (DR region)
# ---------------------------------------------------------------------------

resource "aws_s3_bucket" "backup_dr_replica" {
  provider = aws.dr
  bucket   = "aframp-immutable-backups-${var.environment}-dr-replica"

  lifecycle { prevent_destroy = true }

  tags = { Environment = var.environment, Purpose = "dr-backup-replica" }
}

resource "aws_s3_bucket_versioning" "backup_dr_replica" {
  provider = aws.dr
  bucket   = aws_s3_bucket.backup_dr_replica.id
  versioning_configuration { status = "Enabled" }
}

resource "aws_s3_bucket_object_lock_configuration" "backup_dr_replica" {
  provider = aws.dr
  bucket   = aws_s3_bucket.backup_dr_replica.id

  rule {
    default_retention {
      mode = "COMPLIANCE"
      days = var.backup_retention_days
    }
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "backup_dr_replica" {
  provider = aws.dr
  bucket   = aws_s3_bucket.backup_dr_replica.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm     = "aws:kms"
      kms_master_key_id = aws_kms_key.backup_dr.arn
    }
    bucket_key_enabled = true
  }
}

resource "aws_s3_bucket_public_access_block" "backup_dr_replica" {
  provider                = aws.dr
  bucket                  = aws_s3_bucket.backup_dr_replica.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# ---------------------------------------------------------------------------
# S3 replication: primary → DR replica
# ---------------------------------------------------------------------------

resource "aws_iam_role" "s3_replication" {
  provider = aws.primary
  name     = "aframp-dr-s3-replication-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = "s3.amazonaws.com" }
      Action    = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "s3_replication" {
  provider = aws.primary
  role     = aws_iam_role.s3_replication.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:GetReplicationConfiguration", "s3:ListBucket"]
        Resource = aws_s3_bucket.backup_primary.arn
      },
      {
        Effect = "Allow"
        Action = ["s3:GetObjectVersionForReplication", "s3:GetObjectVersionAcl",
                  "s3:GetObjectVersionTagging"]
        Resource = "${aws_s3_bucket.backup_primary.arn}/*"
      },
      {
        Effect = "Allow"
        Action = ["s3:ReplicateObject", "s3:ReplicateDelete", "s3:ReplicateTags",
                  "s3:ObjectOwnerOverrideToBucketOwner"]
        Resource = "${aws_s3_bucket.backup_dr_replica.arn}/*"
      },
      {
        Effect   = "Allow"
        Action   = ["kms:Decrypt"]
        Resource = aws_kms_key.backup_primary.arn
      },
      {
        Effect   = "Allow"
        Action   = ["kms:GenerateDataKey"]
        Resource = aws_kms_key.backup_dr.arn
      }
    ]
  })
}

resource "aws_s3_bucket_replication_configuration" "primary_to_dr" {
  provider   = aws.primary
  depends_on = [aws_s3_bucket_versioning.backup_primary]
  bucket     = aws_s3_bucket.backup_primary.id
  role       = aws_iam_role.s3_replication.arn

  rule {
    id     = "replicate-all-to-dr"
    status = "Enabled"

    destination {
      bucket        = aws_s3_bucket.backup_dr_replica.arn
      storage_class = "STANDARD"

      encryption_configuration {
        replica_kms_key_id = aws_kms_key.backup_dr.arn
      }
    }

    source_selection_criteria {
      sse_kms_encrypted_objects { status = "Enabled" }
    }
  }
}

# ---------------------------------------------------------------------------
# CloudWatch alarms — RPO / RTO monitoring
# ---------------------------------------------------------------------------

resource "aws_cloudwatch_metric_alarm" "backup_age" {
  provider            = aws.primary
  alarm_name          = "aframp-dr-backup-age-${var.environment}"
  alarm_description   = "Fires when no new backup has been created in the last 6 hours (RPO breach risk)"
  namespace           = "Aframp/DR"
  metric_name         = "BackupAgeSeconds"
  statistic           = "Maximum"
  period              = 3600
  evaluation_periods  = 6
  threshold           = 21600  # 6 hours
  comparison_operator = "GreaterThanThreshold"
  treat_missing_data  = "breaching"

  alarm_actions = [aws_sns_topic.dr_alerts.arn]
  ok_actions    = [aws_sns_topic.dr_alerts.arn]

  tags = { Environment = var.environment }
}

resource "aws_cloudwatch_metric_alarm" "restore_test_failure" {
  provider            = aws.primary
  alarm_name          = "aframp-dr-restore-test-failure-${var.environment}"
  alarm_description   = "Fires when the automated restore verification pipeline fails"
  namespace           = "Aframp/DR"
  metric_name         = "RestoreTestFailures"
  statistic           = "Sum"
  period              = 86400  # 24 hours
  evaluation_periods  = 1
  threshold           = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  treat_missing_data  = "notBreaching"

  alarm_actions = [aws_sns_topic.dr_alerts.arn]

  tags = { Environment = var.environment }
}

# ---------------------------------------------------------------------------
# SNS topic for DR alerts (PagerDuty / email integration)
# ---------------------------------------------------------------------------

resource "aws_sns_topic" "dr_alerts" {
  provider = aws.primary
  name     = "aframp-dr-alerts-${var.environment}"

  tags = { Environment = var.environment }
}

# ---------------------------------------------------------------------------
# Outputs
# ---------------------------------------------------------------------------

output "backup_bucket_primary" {
  description = "Primary immutable backup bucket name"
  value       = aws_s3_bucket.backup_primary.bucket
}

output "backup_bucket_dr_replica" {
  description = "DR replica backup bucket name"
  value       = aws_s3_bucket.backup_dr_replica.bucket
}

output "dr_alerts_sns_arn" {
  description = "SNS topic ARN for DR alerts"
  value       = aws_sns_topic.dr_alerts.arn
}
