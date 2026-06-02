# Runbook: Regional Failover

## Overview

This runbook covers the procedure for handling a complete regional failure and failing over to edge regions.

## Severity: CRITICAL

**Response Time**: Immediate  
**Escalation**: Page on-call engineer + notify CTO

## Symptoms

- Primary region (Cape Town) health checks failing
- High error rates from primary region
- Database replication lag increasing
- CloudWatch alarms firing for primary region
- Route53 health check failures

## Impact

- Increased latency for some users
- Potential service degradation
- Automatic failover to edge regions (Lagos/Nairobi)

## Prerequisites

- Access to AWS Console
- kubectl configured for all clusters
- PagerDuty access
- Slack access to #incidents channel

## Detection

### Automated Alerts

1. **Route53 Health Check Failure**
   - Alert: `health-check-primary-failed`
   - Threshold: 2 consecutive failures

2. **EKS Cluster Unavailable**
   - Alert: `eks-cluster-unreachable`
   - Threshold: 5 minutes

3. **Database Replication Lag**
   - Alert: `database-replication-lag-high`
   - Threshold: > 10 seconds

### Manual Verification

```bash
# Check primary cluster health
kubectl get nodes --context aframp-primary

# Check pod status
kubectl get pods -n aframp-production --context aframp-primary

# Check Route53 health
aws route53 get-health-check-status --health-check-id <PRIMARY_HEALTH_CHECK_ID>
```

## Response Procedure

### Phase 1: Immediate Response (0-5 minutes)

#### 1. Acknowledge Incident
```bash
# Post to Slack
/incident create "Primary region failure - Cape Town"

# Update status page
# https://status.aframp.com
```

#### 2. Verify Edge Regions
```bash
# Check Lagos cluster
kubectl get nodes --context aframp-lagos
kubectl get pods -n aframp-production --context aframp-lagos

# Check Nairobi cluster
kubectl get nodes --context aframp-nairobi
kubectl get pods -n aframp-production --context aframp-nairobi
```

#### 3. Verify Traffic Routing
```bash
# Check Route53 routing
aws route53 list-resource-record-sets \
  --hosted-zone-id <ZONE_ID> \
  --query "ResourceRecordSets[?Name=='api.aframp.com']"

# Verify traffic is routing to edge regions
# Check CloudWatch metrics for Lagos/Nairobi request counts
```

#### 4. Check Database Status
```bash
# Connect to CockroachDB from edge region
cockroach sql --host=cockroachdb.aframp.internal --certs-dir=/certs

# Check cluster status
SHOW CLUSTER SETTING cluster.organization;
SELECT * FROM crdb_internal.gossip_liveness;

# Verify replication
SHOW RANGES;
```

### Phase 2: Stabilization (5-30 minutes)

#### 1. Scale Edge Regions
```bash
# Increase capacity in Lagos
kubectl config use-context aframp-lagos
kubectl scale deployment aframp-api -n aframp-production --replicas=10

# Increase capacity in Nairobi
kubectl config use-context aframp-nairobi
kubectl scale deployment aframp-api -n aframp-production --replicas=10

# Wait for pods to be ready
kubectl wait --for=condition=Ready pods -l app=aframp-api \
  -n aframp-production --timeout=300s --context aframp-lagos

kubectl wait --for=condition=Ready pods -l app=aframp-api \
  -n aframp-production --timeout=300s --context aframp-nairobi
```

#### 2. Verify Application Health
```bash
# Check API health from edge regions
curl -f https://api-lagos.aframp.com/health
curl -f https://api-nairobi.aframp.com/health

# Check metrics
kubectl top pods -n aframp-production --context aframp-lagos
kubectl top pods -n aframp-production --context aframp-nairobi
```

#### 3. Monitor Database Performance
```bash
# Check replication lag
cockroach sql --host=cockroachdb.aframp.internal --execute="
  SELECT
    range_id,
    start_key,
    end_key,
    replicas,
    lease_holder
  FROM crdb_internal.ranges
  WHERE database_name = 'aframp'
  LIMIT 10;
"

# Monitor query performance
cockroach sql --host=cockroachdb.aframp.internal --execute="
  SELECT * FROM crdb_internal.node_statement_statistics
  ORDER BY mean_latency DESC
  LIMIT 10;
"
```

#### 4. Update Monitoring
```bash
# Add annotation to Grafana
curl -X POST https://grafana.aframp.com/api/annotations \
  -H "Authorization: Bearer $GRAFANA_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "text": "Primary region failure - failover to edge regions",
    "tags": ["incident", "failover"],
    "time": '$(date +%s000)'
  }'
```

### Phase 3: Investigation (30-60 minutes)

#### 1. Identify Root Cause
```bash
# Check AWS Service Health Dashboard
# https://health.aws.amazon.com/health/status

# Check CloudWatch logs
aws logs tail /aws/eks/aframp-primary/cluster --follow

# Check VPC Flow Logs
aws ec2 describe-flow-logs --filter "Name=resource-id,Values=<VPC_ID>"

# Check EKS control plane logs
aws eks describe-cluster --name aframp-primary --region af-south-1
```

#### 2. Document Findings
```markdown
# Incident Report Template

## Incident Details
- **Start Time**: 
- **Detection Time**: 
- **Response Time**: 
- **Resolution Time**: 

## Root Cause
[Describe the root cause]

## Impact
- **Users Affected**: 
- **Services Affected**: 
- **Data Loss**: None (RPO = 0)

## Timeline
- HH:MM - Event occurred
- HH:MM - Alert fired
- HH:MM - Engineer responded
- HH:MM - Failover completed
- HH:MM - Service restored

## Actions Taken
1. Verified edge regions healthy
2. Scaled edge regions
3. Monitored traffic routing
4. Verified database consistency
```

### Phase 4: Recovery (1-4 hours)

#### 1. Wait for Primary Region Recovery
```bash
# Monitor AWS Service Health
# Wait for AWS to resolve regional issues

# Check primary cluster periodically
watch -n 60 'kubectl get nodes --context aframp-primary'
```

#### 2. Verify Primary Region Health
```bash
# Once primary is back, verify all components
kubectl get nodes --context aframp-primary
kubectl get pods -n aframp-production --context aframp-primary

# Check database connectivity
cockroach sql --host=primary-cockroachdb.aframp.internal --execute="SELECT 1;"

# Check Redis
redis-cli -h redis-primary.aframp.internal ping
```

#### 3. Gradual Traffic Restoration
```bash
# Update Route53 health check to re-enable primary
aws route53 change-resource-record-sets \
  --hosted-zone-id <ZONE_ID> \
  --change-batch file://restore-primary.json

# restore-primary.json
{
  "Changes": [{
    "Action": "UPSERT",
    "ResourceRecordSet": {
      "Name": "api.aframp.com",
      "Type": "A",
      "SetIdentifier": "primary",
      "HealthCheckId": "<PRIMARY_HEALTH_CHECK_ID>",
      "AliasTarget": {
        "HostedZoneId": "<ELB_ZONE_ID>",
        "DNSName": "<PRIMARY_ELB_DNS>",
        "EvaluateTargetHealth": true
      }
    }
  }]
}
```

#### 4. Monitor Traffic Distribution
```bash
# Watch CloudWatch metrics for traffic distribution
aws cloudwatch get-metric-statistics \
  --namespace AWS/ApplicationELB \
  --metric-name RequestCount \
  --dimensions Name=LoadBalancer,Value=<PRIMARY_LB> \
  --start-time $(date -u -d '10 minutes ago' +%Y-%m-%dT%H:%M:%S) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%S) \
  --period 60 \
  --statistics Sum
```

#### 5. Scale Down Edge Regions
```bash
# Once primary is stable, scale down edge regions
kubectl scale deployment aframp-api -n aframp-production \
  --replicas=3 --context aframp-lagos

kubectl scale deployment aframp-api -n aframp-production \
  --replicas=3 --context aframp-nairobi
```

### Phase 5: Post-Incident (4-24 hours)

#### 1. Verify Data Consistency
```bash
# Run data consistency checks
cockroach sql --host=cockroachdb.aframp.internal --execute="
  SELECT
    table_name,
    row_count,
    total_bytes
  FROM crdb_internal.table_row_statistics
  WHERE database_name = 'aframp'
  ORDER BY table_name;
"

# Compare with pre-incident snapshot
# Verify no data loss (RPO = 0)
```

#### 2. Review Metrics
```bash
# Generate incident report
# - Total downtime
# - Users affected
# - Error rate during incident
# - Recovery time (RTO)
# - Data loss (RPO)
```

#### 3. Update Documentation
- Update this runbook with lessons learned
- Document any new failure modes discovered
- Update monitoring thresholds if needed

#### 4. Schedule Post-Mortem
- Schedule within 48 hours
- Invite all stakeholders
- Use blameless post-mortem format

## Rollback Procedure

If failover causes issues:

```bash
# Disable edge regions in Route53
aws route53 change-resource-record-sets \
  --hosted-zone-id <ZONE_ID> \
  --change-batch file://disable-edge.json

# Scale down edge regions
kubectl scale deployment aframp-api -n aframp-production \
  --replicas=0 --context aframp-lagos

kubectl scale deployment aframp-api -n aframp-production \
  --replicas=0 --context aframp-nairobi

# Wait for primary region recovery
```

## Success Criteria

- [ ] Edge regions serving traffic successfully
- [ ] Error rate < 0.1%
- [ ] Latency within acceptable range (< 200ms P99)
- [ ] Database replication lag < 5 seconds
- [ ] No data loss (RPO = 0)
- [ ] Recovery time < 5 minutes (RTO)

## Communication Templates

### Initial Notification
```
🚨 INCIDENT: Primary region failure detected

Status: Investigating
Impact: Automatic failover to edge regions in progress
ETA: 5 minutes

We are investigating a failure in our primary region (Cape Town).
Traffic is automatically routing to our edge regions (Lagos/Nairobi).
No data loss expected.

Updates: Every 15 minutes
```

### Update Template
```
📊 UPDATE: Regional failover

Status: Mitigated
Impact: Service operating normally from edge regions
Next: Monitoring primary region recovery

Edge regions (Lagos/Nairobi) are serving all traffic.
Performance is within normal parameters.
Waiting for primary region to recover.

Next update: 15 minutes
```

### Resolution Template
```
✅ RESOLVED: Regional failover complete

Status: Resolved
Duration: XX minutes
Impact: Minimal service degradation
Data Loss: None

Primary region has recovered and is serving traffic.
All systems operating normally.
Post-mortem scheduled for [DATE/TIME].

Thank you for your patience.
```

## Contacts

- **On-Call Engineer**: PagerDuty
- **Infrastructure Lead**: [Name] - [Phone]
- **CTO**: [Name] - [Phone]
- **AWS Support**: Premium Support Case

## Related Runbooks

- [Database Failover](./database-failover.md)
- [Redis Cluster Failure](./redis-failure.md)
- [Network Partition](./network-partition.md)

## Revision History

- 2026-06-01: Initial version
- [Date]: [Changes]
