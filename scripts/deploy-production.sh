#!/bin/bash
set -euo pipefail

# Production Deployment Script for Multi-Region Aframp Infrastructure
# This script orchestrates the complete deployment process

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    local missing_tools=()
    
    command -v terraform >/dev/null 2>&1 || missing_tools+=("terraform")
    command -v kubectl >/dev/null 2>&1 || missing_tools+=("kubectl")
    command -v aws >/dev/null 2>&1 || missing_tools+=("aws")
    command -v helm >/dev/null 2>&1 || missing_tools+=("helm")
    command -v istioctl >/dev/null 2>&1 || missing_tools+=("istioctl")
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        exit 1
    fi
    
    log_info "All prerequisites satisfied"
}

# Phase 1: Infrastructure Provisioning
deploy_infrastructure() {
    log_info "Phase 1: Deploying infrastructure with Terraform..."
    
    cd "${PROJECT_ROOT}/infra/terraform/multi-region"
    
    # Initialize Terraform
    log_info "Initializing Terraform..."
    terraform init
    
    # Validate configuration
    log_info "Validating Terraform configuration..."
    terraform validate
    
    # Plan deployment
    log_info "Planning infrastructure deployment..."
    terraform plan -out=tfplan
    
    # Confirm deployment
    read -p "Review the plan above. Continue with deployment? (yes/no): " confirm
    if [ "$confirm" != "yes" ]; then
        log_warn "Deployment cancelled by user"
        exit 0
    fi
    
    # Apply infrastructure
    log_info "Applying infrastructure changes..."
    terraform apply tfplan
    
    # Save outputs
    terraform output -json > "${PROJECT_ROOT}/terraform-outputs.json"
    
    log_info "Infrastructure deployment complete"
}

# Phase 2: Configure kubectl contexts
configure_kubectl() {
    log_info "Phase 2: Configuring kubectl contexts..."
    
    # Extract cluster names from Terraform outputs
    PRIMARY_CLUSTER=$(terraform output -raw eks_primary_cluster_id)
    LAGOS_CLUSTER=$(terraform output -raw eks_lagos_cluster_id)
    NAIROBI_CLUSTER=$(terraform output -raw eks_nairobi_cluster_id)
    
    # Configure kubectl for each cluster
    log_info "Configuring kubectl for primary cluster..."
    aws eks update-kubeconfig --region af-south-1 --name "$PRIMARY_CLUSTER" --alias aframp-primary
    
    log_info "Configuring kubectl for Lagos cluster..."
    aws eks update-kubeconfig --region eu-west-1 --name "$LAGOS_CLUSTER" --alias aframp-lagos
    
    log_info "Configuring kubectl for Nairobi cluster..."
    aws eks update-kubeconfig --region eu-central-1 --name "$NAIROBI_CLUSTER" --alias aframp-nairobi
    
    log_info "Kubectl contexts configured"
}

# Phase 3: Install Istio Service Mesh
install_istio() {
    log_info "Phase 3: Installing Istio service mesh..."
    
    for context in aframp-primary aframp-lagos aframp-nairobi; do
        log_info "Installing Istio on $context..."
        
        kubectl config use-context "$context"
        
        # Install Istio with production profile
        istioctl install --set profile=production -y
        
        # Verify installation
        kubectl wait --for=condition=available --timeout=300s \
            deployment/istiod -n istio-system
        
        log_info "Istio installed on $context"
    done
}

# Phase 4: Deploy application
deploy_application() {
    log_info "Phase 4: Deploying application..."
    
    cd "${PROJECT_ROOT}/k8s/production"
    
    for context in aframp-primary aframp-lagos aframp-nairobi; do
        log_info "Deploying to $context..."
        
        kubectl config use-context "$context"
        
        # Create namespaces
        kubectl apply -f namespace.yaml
        
        # Create secrets (ensure these are properly configured)
        log_warn "Ensure secrets are configured in secrets-template.yaml before proceeding"
        read -p "Have you configured secrets? (yes/no): " secrets_confirm
        if [ "$secrets_confirm" != "yes" ]; then
            log_error "Please configure secrets before deployment"
            exit 1
        fi
        
        kubectl apply -f secrets-template.yaml
        
        # Deploy Istio gateway and policies
        kubectl apply -f istio-gateway.yaml
        
        # Deploy application
        kubectl apply -f deployment.yaml
        
        # Deploy HPA and PDB
        kubectl apply -f hpa.yaml
        
        # Deploy network policies
        kubectl apply -f network-policy.yaml
        
        # Wait for deployments
        log_info "Waiting for deployments to be ready..."
        kubectl wait --for=condition=available --timeout=600s \
            deployment/aframp-api -n aframp-production
        
        log_info "Application deployed to $context"
    done
}

# Phase 5: Deploy monitoring
deploy_monitoring() {
    log_info "Phase 5: Deploying monitoring stack..."
    
    kubectl config use-context aframp-primary
    
    cd "${PROJECT_ROOT}/k8s/production/monitoring"
    
    # Deploy Prometheus
    kubectl apply -f prometheus-config.yaml
    
    # Install Prometheus using Helm
    helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
    helm repo update
    
    helm upgrade --install prometheus prometheus-community/kube-prometheus-stack \
        --namespace aframp-monitoring \
        --create-namespace \
        --values - <<EOF
prometheus:
  prometheusSpec:
    additionalScrapeConfigs:
      - job_name: 'aframp-api'
        kubernetes_sd_configs:
        - role: pod
          namespaces:
            names:
            - aframp-production
grafana:
  adminPassword: "CHANGE_ME"
  ingress:
    enabled: true
    hosts:
    - grafana.aframp.com
EOF
    
    log_info "Monitoring stack deployed"
}

# Phase 6: Verification
verify_deployment() {
    log_info "Phase 6: Verifying deployment..."
    
    kubectl config use-context aframp-primary
    
    # Check pod status
    log_info "Checking pod status..."
    kubectl get pods -n aframp-production
    
    # Check service endpoints
    log_info "Checking service endpoints..."
    kubectl get svc -n aframp-production
    
    # Check Istio gateway
    log_info "Checking Istio gateway..."
    kubectl get gateway -n aframp-production
    
    # Run health checks
    log_info "Running health checks..."
    API_ENDPOINT=$(kubectl get svc istio-ingressgateway -n istio-system -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')
    
    if curl -f "http://${API_ENDPOINT}/health" >/dev/null 2>&1; then
        log_info "Health check passed"
    else
        log_warn "Health check failed - manual verification required"
    fi
    
    log_info "Deployment verification complete"
}

# Main deployment flow
main() {
    log_info "Starting production deployment..."
    log_info "Target: Multi-region Kubernetes (Cape Town, Lagos, Nairobi)"
    
    check_prerequisites
    
    # Confirm production deployment
    read -p "This will deploy to PRODUCTION. Are you sure? (yes/no): " prod_confirm
    if [ "$prod_confirm" != "yes" ]; then
        log_warn "Deployment cancelled"
        exit 0
    fi
    
    deploy_infrastructure
    configure_kubectl
    install_istio
    deploy_application
    deploy_monitoring
    verify_deployment
    
    log_info "=========================================="
    log_info "Production deployment complete!"
    log_info "=========================================="
    log_info ""
    log_info "Next steps:"
    log_info "1. Configure DNS records for your domain"
    log_info "2. Set up SSL certificates"
    log_info "3. Configure monitoring alerts"
    log_info "4. Run chaos engineering tests"
    log_info "5. Perform load testing"
    log_info ""
    log_info "Cluster contexts:"
    log_info "  - aframp-primary (Cape Town)"
    log_info "  - aframp-lagos (Lagos Edge)"
    log_info "  - aframp-nairobi (Nairobi Edge)"
}

# Run main function
main "$@"
