output "metrics_server_status" {
  description = "Helm release status for metrics-server."
  value       = helm_release.metrics_server.status
}

output "prometheus_adapter_status" {
  description = "Helm release status for prometheus-adapter."
  value       = helm_release.prometheus_adapter.status
}
