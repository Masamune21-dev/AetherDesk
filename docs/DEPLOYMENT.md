# Deployment Architecture Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Topologi Deployment (Kubernetes)

Sistem di-deploy di cloud native Kubernetes (EKS / GKE / AKS) menggunakan Helm Charts.

```
                    Cloudflare (WAF / DNS / CDN)
                                 │
                                 ▼
                     HAProxy / Nginx Ingress
                                 │
         ┌───────────────────────┼───────────────────────┐
         │ (HTTP/gRPC/WS)        │ (STUN/TURN UDP)       │
         ▼                       ▼                       ▼
   [API Pods]             [Signal Pods]            [TURN Cluster]
   (Axum API)             (WS Signaling)            (coturn/Pion)
         │                       │                       │
         └───────────┬───────────┘                       │
                     ▼                                   │
              [NATS Cluster]                             │
                     │                                   │
         ┌───────────┴───────────┐                       │
         ▼                       ▼                       ▼
   [Redis Cluster]       [PostgreSQL HA]         [Object Storage (S3)]
   (Session State)       (Durable Metadata)      (Session Recordings)
```

---

## 2. High Availability (HA) Setup

### 2.1 Database (PostgreSQL HA via Patroni)
- 3-node cluster dengan satu Primary (Read-Write) dan dua Standby/Replica (Read-Only).
- Sinkronisasi data menggunakan PostgreSQL streaming replication.
- Sentinel (Consul/etcd) memantau kesehatan node. Jika Primary mati, Patroni secara otomatis mempromosikan Replica menjadi Primary baru dalam <10 detik.

### 2.2 Cache (Redis Cluster)
- 6-node Redis Cluster (3 Master, 3 Replicas).
- Data session presence di-sharding ke 3 Master.
- Redis Sentinel menangani automatic failover jika salah satu master mati.

### 2.3 WebRTC Relay (TURN Cluster)
- Di-deploy di luar K8s cluster langsung pada Virtual Machine (EC2/GCE) di 50+ PoP untuk latensi rendah.
- Setiap VM menjalankan **coturn** atau **Pion TURN service**.
- Load balancing menggunakan DNS Anycast.

---

## 3. Kubernetes Deployment Manifest (Helm Values Sample)

```yaml
# values.yaml for Helm deployment

global:
  environment: production
  domain: rdp.io

api:
  replicaCount: 5
  image:
    repository: registry.rdp.io/api
    tag: 1.0.0
    pullPolicy: IfNotPresent
  resources:
    limits:
      cpu: 2000m
      memory: 2Gi
    requests:
      cpu: 500m
      memory: 512Mi
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 20
    targetCPUUtilizationPercentage: 80

signal:
  replicaCount: 5
  image:
    repository: registry.rdp.io/signal
    tag: 1.0.0
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 20
    targetMemoryUtilizationPercentage: 80

nats:
  jetstream:
    enabled: true
  cluster:
    replicas: 3

postgresql:
  architecture: replication
  auth:
    database: rdp
  primary:
    resources:
      limits:
        cpu: 4000m
        memory: 8Gi
  readReplicas:
    replicaCount: 2
```

---

## 4. Disaster Recovery (DR) Plan

Sistem dirancang untuk menghadapi skenario bencana berskala regional (Recovery Point Objective / RPO = 5 menit, Recovery Time Objective / RTO = 30 menit).

- **Backup Otomatis**: PostgreSQL di-backup setiap 6 jam menggunakan `pg_backrest` ke AWS S3 dengan retensi 30 hari.
- **Cross-Region Replication**: Bucket S3 session recordings direplikasi secara asinkron ke regional backup S3 bucket di zona berbeda.
- **Infrastructure as Code (IaC)**: Seluruh topologi didefinisikan menggunakan **Terraform** sehingga seluruh stack infrastruktur dapat dibangun ulang di region baru secara otomatis dalam waktu < 20 menit.
