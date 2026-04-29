# terraform/main.tf
# Provisions the Kubernetes infrastructure required for HPA (Issue #422):
#   1. metrics-server  — supplies CPU/memory to the native HPA
#   2. prometheus-adapter — bridges Prometheus metrics into the custom-metrics API
#   3. HPA resource (via kubectl manifest) — the autoscaling policy itself
#
# Prerequisites:
#   - A kubeconfig pointing at the target cluster (set via KUBECONFIG env var
#     or the kubeconfig_path variable).
#   - Helm provider >= 2.12, Kubernetes provider >= 2.27.
#   - Prometheus already running in the "monitoring" namespace.

terraform {
  required_version = ">= 1.6"
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.12"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.27"
    }
  }
}

# ---------------------------------------------------------------------------
# Providers
# ---------------------------------------------------------------------------

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path
  }
}

provider "kubernetes" {
  config_path = var.kubeconfig_path
}

# ---------------------------------------------------------------------------
# 1. metrics-server
# Provides CPU and memory metrics consumed by the native HPA.
# ---------------------------------------------------------------------------

resource "helm_release" "metrics_server" {
  name             = "metrics-server"
  repository       = "https://kubernetes-sigs.github.io/metrics-server/"
  chart            = "metrics-server"
  version          = "3.12.1"
  namespace        = "kube-system"
  create_namespace = false

  set {
    name  = "args[0]"
    value = "--kubelet-insecure-tls"   # required on most managed clusters
  }
}

# ---------------------------------------------------------------------------
# 2. prometheus-adapter
# Bridges aframp_http_requests_total → requests_per_second in the
# custom.metrics.k8s.io API so the HPA can consume it.
# ---------------------------------------------------------------------------

resource "helm_release" "prometheus_adapter" {
  name             = "prometheus-adapter"
  repository       = "https://prometheus-community.github.io/helm-charts"
  chart            = "prometheus-adapter"
  version          = "4.10.0"
  namespace        = "monitoring"
  create_namespace = true

  # Point the adapter at the in-cluster Prometheus service.
  set {
    name  = "prometheus.url"
    value = var.prometheus_url
  }
  set {
    name  = "prometheus.port"
    value = "9090"
  }

  # Mount our custom rules ConfigMap instead of the chart's default config.
  set {
    name  = "rules.existing"
    value = "prometheus-adapter-config"
  }

  depends_on = [helm_release.metrics_server]
}

# ---------------------------------------------------------------------------
# 3. HPA resource
# Applies the HPA manifest directly via the Kubernetes provider so the
# autoscaling policy is managed as IaC alongside the Helm releases.
# ---------------------------------------------------------------------------

resource "kubernetes_manifest" "aframp_hpa" {
  manifest = yamldecode(file("${path.module}/../k8s/hpa/hpa.yaml"))

  depends_on = [helm_release.prometheus_adapter]
}
