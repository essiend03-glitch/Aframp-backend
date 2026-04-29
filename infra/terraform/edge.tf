# infra/terraform/edge.tf — Issue #348
# CloudFront distribution with path-based cache policies.
# Aggressive TTL for /public/*, no-cache for /account/* and financial paths.

terraform {
  required_providers {
    aws = { source = "hashicorp/aws", version = "~> 5.0" }
  }
}

variable "origin_domain" {
  description = "ALB / origin domain name"
  type        = string
}

variable "certificate_arn" {
  description = "ACM certificate ARN (us-east-1) for the CloudFront distribution"
  type        = string
}

# ---------------------------------------------------------------------------
# Cache policies
# ---------------------------------------------------------------------------

resource "aws_cloudfront_cache_policy" "public_aggressive" {
  name        = "aframp-public-aggressive"
  min_ttl     = 60
  default_ttl = 300
  max_ttl     = 86400

  parameters_in_cache_key_and_forwarded_to_origin {
    cookies_config  { cookie_behavior = "none" }
    headers_config  { header_behavior = "none" }
    query_strings_config { query_string_behavior = "none" }
    enable_accept_encoding_gzip   = true
    enable_accept_encoding_brotli = true
  }
}

resource "aws_cloudfront_cache_policy" "no_cache" {
  name        = "aframp-no-cache"
  min_ttl     = 0
  default_ttl = 0
  max_ttl     = 0

  parameters_in_cache_key_and_forwarded_to_origin {
    cookies_config  { cookie_behavior = "all" }
    headers_config  {
      header_behavior = "whitelist"
      headers { items = ["Authorization", "X-Api-Key", "X-Consistency"] }
    }
    query_strings_config { query_string_behavior = "all" }
  }
}

# ---------------------------------------------------------------------------
# Origin request policy — forward consistency header to origin
# ---------------------------------------------------------------------------

resource "aws_cloudfront_origin_request_policy" "forward_consistency" {
  name = "aframp-forward-consistency"

  cookies_config  { cookie_behavior = "none" }
  headers_config  {
    header_behavior = "whitelist"
    headers { items = ["X-Consistency", "X-Forwarded-For", "X-Api-Key"] }
  }
  query_strings_config { query_string_behavior = "all" }
}

# ---------------------------------------------------------------------------
# CloudFront distribution
# ---------------------------------------------------------------------------

resource "aws_cloudfront_distribution" "aframp_edge" {
  enabled             = true
  is_ipv6_enabled     = true
  price_class         = "PriceClass_All"
  aliases             = ["api.aframp.io"]
  http_version        = "http2and3"

  origin {
    domain_name = var.origin_domain
    origin_id   = "aframp-alb"

    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }

    custom_header {
      name  = "X-Origin-Verify"
      value = "cloudfront-only"
    }
  }

  # Default behaviour — short TTL, private
  default_cache_behavior {
    target_origin_id         = "aframp-alb"
    viewer_protocol_policy   = "redirect-to-https"
    allowed_methods          = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods           = ["GET", "HEAD"]
    cache_policy_id          = aws_cloudfront_cache_policy.no_cache.id
    origin_request_policy_id = aws_cloudfront_origin_request_policy.forward_consistency.id
    compress                 = true
  }

  # /public/* — aggressive caching
  ordered_cache_behavior {
    path_pattern             = "/public/*"
    target_origin_id         = "aframp-alb"
    viewer_protocol_policy   = "redirect-to-https"
    allowed_methods          = ["GET", "HEAD", "OPTIONS"]
    cached_methods           = ["GET", "HEAD"]
    cache_policy_id          = aws_cloudfront_cache_policy.public_aggressive.id
    origin_request_policy_id = aws_cloudfront_origin_request_policy.forward_consistency.id
    compress                 = true
  }

  # /account/* — never cache
  ordered_cache_behavior {
    path_pattern             = "/account/*"
    target_origin_id         = "aframp-alb"
    viewer_protocol_policy   = "redirect-to-https"
    allowed_methods          = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods           = ["GET", "HEAD"]
    cache_policy_id          = aws_cloudfront_cache_policy.no_cache.id
    origin_request_policy_id = aws_cloudfront_origin_request_policy.forward_consistency.id
    compress                 = true
  }

  restrictions {
    geo_restriction { restriction_type = "none" }
  }

  viewer_certificate {
    acm_certificate_arn      = var.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }

  tags = { Project = "aframp", Issue = "348" }
}

output "cloudfront_domain" {
  value = aws_cloudfront_distribution.aframp_edge.domain_name
}
