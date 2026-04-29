variable "kubeconfig_path" {
  description = "Path to the kubeconfig file for the target cluster."
  type        = string
  default     = "~/.kube/config"
}

variable "prometheus_url" {
  description = "In-cluster URL of the Prometheus server used by prometheus-adapter."
  type        = string
  default     = "http://prometheus-operated.monitoring.svc.cluster.local"
}

variable "namespace" {
  description = "Kubernetes namespace where the aframp-backend Deployment lives."
  type        = string
  default     = "default"
}
