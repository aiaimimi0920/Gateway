# High Availability Design - AI Gateway

## 目的

本文档定义 AI 网关的高可用架构设计，包括故障转移、数据备份、区域容灾、降级策略和恢复时间目标。

## 高可用目标

- **可用性目标**：99.99%（年停机时间 < 52.6 分钟）
- **RTO（恢复时间目标）**：< 5 分钟
- **RPO（恢复点目标）**：< 1 分钟
- **MTTR（平均恢复时间）**：< 15 分钟
- **MTBF（平均故障间隔）**：> 30 天

---

## 1. 架构设计

### 1.1 多层冗余架构

```
                    ┌─────────────────┐
                    │   Global CDN    │
                    │  (Cloudflare)   │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │  Load Balancer  │
                    │   (Active-Active)│
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
   ┌────▼────┐         ┌────▼────┐         ┌────▼────┐
   │ Gateway │         │ Gateway │         │ Gateway │
   │ Node 1  │         │ Node 2  │         │ Node 3  │
   └────┬────┘         └────┬────┘         └────┬────┘
        │                    │                    │
        └────────────────────┼────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
   ┌────▼────┐         ┌────▼────┐         ┌────▼────┐
   │ Redis   │◄────────┤ Redis   │────────►│ Redis   │
   │ Primary │         │ Replica │         │ Replica │
   └─────────┘         └─────────┘         └─────────┘
        │
   ┌────▼────────────────────────────────┐
   │     PostgreSQL Cluster              │
   │  (Primary + 2 Replicas + Failover)  │
   └─────────────────────────────────────┘
```

### 1.2 无单点故障设计

**组件冗余：**
- **负载均衡器**：双活配置，自动故障转移
- **网关节点**：至少 3 个节点，支持水平扩展
- **Redis**：主从复制 + Sentinel 自动故障转移
- **PostgreSQL**：主从复制 + 自动故障转移
- **对象存储**：S3 多区域复制

---

## 2. 故障转移

### 2.1 负载均衡器故障转移

**健康检查配置：**
```rust
pub struct HealthCheck {
    endpoint: String,        // /healthz
    interval: Duration,      // 10s
    timeout: Duration,       // 5s
    healthy_threshold: u32,  // 2 次成功
    unhealthy_threshold: u32,// 3 次失败
}

pub async fn health_check_handler() -> Result<StatusCode, StatusCode> {
    // 检查关键依赖
    if !check_redis().await {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    
    if !check_database().await {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    
    Ok(StatusCode::OK)
}
```

**故障转移策略：**
```yaml
# HAProxy 配置
backend gateway_cluster
  mode http
  balance roundrobin
  option httpchk GET /healthz
  http-check expect status 200
  
  server gateway1 10.0.1.10:4200 check inter 10s fall 3 rise 2
  server gateway2 10.0.1.11:4200 check inter 10s fall 3 rise 2
  server gateway3 10.0.1.12:4200 check inter 10s fall 3 rise 2
```

### 2.2 Redis 故障转移

**Redis Sentinel 配置：**
```conf
# sentinel.conf
sentinel monitor mymaster 10.0.2.10 6379 2
sentinel down-after-milliseconds mymaster 5000
sentinel parallel-syncs mymaster 1
sentinel failover-timeout mymaster 10000
```

**客户端自动重连：**
```rust
use redis::aio::ConnectionManager;

pub async fn create_redis_client() -> Result<ConnectionManager, Error> {
    let client = redis::Client::open("redis://sentinel1:26379,sentinel2:26379,sentinel3:26379")?;
    
    let manager = ConnectionManager::new(client).await?;
    
    // 自动重连配置
    manager.set_reconnect_policy(
        ExponentialBackoff::from_millis(100)
            .max_delay(Duration::from_secs(5))
            .max_elapsed_time(Some(Duration::from_secs(60)))
    );
    
    Ok(manager)
}
```

### 2.3 PostgreSQL 故障转移

**Patroni 自动故障转移：**
```yaml
# patroni.yml
scope: neuro-gateway
name: postgres1

restapi:
  listen: 0.0.0.0:8008
  connect_address: 10.0.3.10:8008

etcd:
  hosts: etcd1:2379,etcd2:2379,etcd3:2379

bootstrap:
  dcs:
    ttl: 30
    loop_wait: 10
    retry_timeout: 10
    maximum_lag_on_failover: 1048576
    
postgresql:
  listen: 0.0.0.0:5432
  connect_address: 10.0.3.10:5432
  data_dir: /var/lib/postgresql/data
  
  parameters:
    max_connections: 100
    shared_buffers: 256MB
    wal_level: replica
    max_wal_senders: 10
```

**应用层连接池故障转移：**
```rust
use sqlx::postgres::PgPoolOptions;

pub async fn create_db_pool() -> Result<PgPool, Error> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        // 多个数据库地址，自动故障转移
        .connect("postgresql://postgres1:5432,postgres2:5432,postgres3:5432/neuroloom")
        .await?;
    
    Ok(pool)
}
```

---

## 3. 数据备份

### 3.1 PostgreSQL 备份策略

**全量备份 + 增量备份：**
```bash
#!/bin/bash
# 每天凌晨 2 点全量备份
0 2 * * * /usr/bin/pg_basebackup -h localhost -U postgres -D /backup/full/$(date +\%Y\%m\%d) -Ft -z -P

# 每小时增量备份（WAL 归档）
*/60 * * * * /usr/bin/pg_receivewal -h localhost -U postgres -D /backup/wal -S backup_slot
```

**备份保留策略：**
- **每日全量备份**：保留 7 天
- **每周全量备份**：保留 4 周
- **每月全量备份**：保留 12 个月
- **WAL 归档**：保留 7 天

**备份验证：**
```rust
pub async fn verify_backup(backup_path: &str) -> Result<bool, Error> {
    // 1. 检查备份文件完整性
    let checksum = calculate_checksum(backup_path).await?;
    let expected_checksum = read_checksum_file(&format!("{}.sha256", backup_path)).await?;
    
    if checksum != expected_checksum {
        return Ok(false);
    }
    
    // 2. 尝试恢复到临时数据库
    let temp_db = create_temp_database().await?;
    restore_backup(backup_path, &temp_db).await?;
    
    // 3. 验证数据完整性
    let row_count = count_rows(&temp_db).await?;
    
    // 4. 清理临时数据库
    drop_temp_database(&temp_db).await?;
    
    Ok(row_count > 0)
}
```

### 3.2 Redis 备份策略

**RDB + AOF 双重持久化：**
```conf
# redis.conf
save 900 1      # 15分钟内至少1个key变化
save 300 10     # 5分钟内至少10个key变化
save 60 10000   # 1分钟内至少10000个key变化

appendonly yes
appendfsync everysec
```

**备份脚本：**
```bash
#!/bin/bash
# 每小时备份 RDB
0 * * * * redis-cli --rdb /backup/redis/dump-$(date +\%Y\%m\%d-\%H).rdb

# 每天备份 AOF
0 0 * * * cp /var/lib/redis/appendonly.aof /backup/redis/appendonly-$(date +\%Y\%m\%d).aof
```

### 3.3 配置和代码备份

**Git 版本控制：**
```bash
# 配置文件版本控制
/etc/neuro-gateway/
├── config.yaml
├── providers.yaml
├── routes.yaml
└── .git/

# 自动提交配置变更
*/15 * * * * cd /etc/neuro-gateway && git add -A && git commit -m "Auto backup $(date)" && git push
```

---

## 4. 区域容灾

### 4.1 多区域部署

**区域架构：**
```
Region 1 (Primary)          Region 2 (Standby)          Region 3 (Standby)
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│ Gateway Cluster │         │ Gateway Cluster │         │ Gateway Cluster │
│ (Active)        │         │ (Standby)       │         │ (Standby)       │
└────────┬────────┘         └────────┬────────┘         └────────┬────────┘
         │                           │                           │
         │                           │                           │
┌────────▼────────┐         ┌────────▼────────┐         ┌────────▼────────┐
│ PostgreSQL      │────────►│ PostgreSQL      │────────►│ PostgreSQL      │
│ (Primary)       │         │ (Replica)       │         │ (Replica)       │
└─────────────────┘         └─────────────────┘         └─────────────────┘
         │                           │                           │
         └───────────────────────────┴───────────────────────────┘
                              S3 Cross-Region Replication
```

**DNS 故障转移：**
```yaml
# Route53 健康检查和故障转移
HealthCheck:
  Type: HTTPS
  ResourcePath: /healthz
  FullyQualifiedDomainName: gateway-region1.neuroloom.ai
  Port: 443
  RequestInterval: 30
  FailureThreshold: 3

RecordSet:
  Name: gateway.neuroloom.ai
  Type: A
  SetIdentifier: region1
  Failover: PRIMARY
  HealthCheckId: !Ref HealthCheck
  AliasTarget:
    DNSName: gateway-region1.neuroloom.ai
```

### 4.2 数据同步

**跨区域数据库复制：**
```sql
-- 配置逻辑复制
CREATE PUBLICATION gateway_pub FOR ALL TABLES;

-- 在备用区域订阅
CREATE SUBSCRIPTION gateway_sub
    CONNECTION 'host=region1-db.neuroloom.ai port=5432 dbname=neuroloom'
    PUBLICATION gateway_pub;
```

**S3 跨区域复制：**
```json
{
  "Role": "arn:aws:iam::123456789:role/s3-replication",
  "Rules": [{
    "Status": "Enabled",
    "Priority": 1,
    "Filter": {},
    "Destination": {
      "Bucket": "arn:aws:s3:::neuroloom-backup-region2",
      "ReplicationTime": {
        "Status": "Enabled",
        "Time": {
          "Minutes": 15
        }
      }
    }
  }]
}
```

### 4.3 区域切换流程

**自动切换触发条件：**
- 主区域健康检查连续失败 > 3 次
- 主区域延迟 > 5 秒
- 主区域错误率 > 10%

**切换步骤：**
```rust
pub async fn failover_to_region(target_region: &str) -> Result<(), Error> {
    // 1. 验证目标区域健康
    if !check_region_health(target_region).await? {
        return Err(Error::TargetRegionUnhealthy);
    }
    
    // 2. 停止主区域写入
    set_region_readonly("primary").await?;
    
    // 3. 等待数据同步完成
    wait_for_replication_lag(target_region, Duration::from_secs(60)).await?;
    
    // 4. 提升目标区域为主区域
    promote_region_to_primary(target_region).await?;
    
    // 5. 更新 DNS
    update_dns_to_region(target_region).await?;
    
    // 6. 验证切换成功
    verify_failover(target_region).await?;
    
    Ok(())
}
```

---

## 5. 降级策略

### 5.1 功能降级

**降级级别：**
```rust
pub enum DegradationLevel {
    Normal,      // 正常运行
    Level1,      // 关闭非关键功能
    Level2,      // 只保留核心功能
    Level3,      // 只读模式
    Emergency,   // 完全停止服务
}

pub struct FeatureFlags {
    response_cache: bool,      // Level 1: 关闭响应缓存
    prompt_cache: bool,        // Level 1: 关闭提示缓存
    request_audit: bool,       // Level 1: 关闭详细审计
    usage_tracking: bool,      // Level 2: 关闭使用量追踪
    provider_failover: bool,   // Level 2: 关闭提供商故障转移
    new_requests: bool,        // Level 3: 拒绝新请求
}
```

**自动降级触发：**
```rust
pub async fn check_and_apply_degradation() -> Result<(), Error> {
    let metrics = collect_system_metrics().await?;
    
    let level = if metrics.error_rate > 0.5 {
        DegradationLevel::Emergency
    } else if metrics.cpu_usage > 0.95 || metrics.memory_usage > 0.95 {
        DegradationLevel::Level3
    } else if metrics.error_rate > 0.1 {
        DegradationLevel::Level2
    } else if metrics.latency_p99 > 5.0 {
        DegradationLevel::Level1
    } else {
        DegradationLevel::Normal
    };
    
    apply_degradation(level).await?;
    
    Ok(())
}
```

### 5.2 熔断机制

**熔断器实现：**
```rust
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
}

pub enum CircuitState {
    Closed,      // 正常状态
    Open,        // 熔断状态
    HalfOpen,    // 半开状态（尝试恢复）
}

impl CircuitBreaker {
    pub async fn call<F, T>(&self, f: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, Error>>,
    {
        let state = self.state.lock().await.clone();
        
        match state {
            CircuitState::Open => {
                // 熔断状态，直接返回错误
                Err(Error::CircuitBreakerOpen)
            }
            CircuitState::HalfOpen => {
                // 半开状态，尝试调用
                match f.await {
                    Ok(result) => {
                        self.on_success().await;
                        Ok(result)
                    }
                    Err(e) => {
                        self.on_failure().await;
                        Err(e)
                    }
                }
            }
            CircuitState::Closed => {
                // 正常状态
                match f.await {
                    Ok(result) => {
                        self.on_success().await;
                        Ok(result)
                    }
                    Err(e) => {
                        self.on_failure().await;
                        Err(e)
                    }
                }
            }
        }
    }
    
    async fn on_failure(&self) {
        let mut state = self.state.lock().await;
        // 失败次数超过阈值，打开熔断器
        if self.failure_count.fetch_add(1, Ordering::Relaxed) >= self.failure_threshold {
            *state = CircuitState::Open;
            
            // 设置定时器，超时后进入半开状态
            let state_clone = self.state.clone();
            let timeout = self.timeout;
            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                *state_clone.lock().await = CircuitState::HalfOpen;
            });
        }
    }
}
```

### 5.3 限流保护

**令牌桶限流：**
```rust
pub struct RateLimiter {
    capacity: u32,
    refill_rate: u32,
    tokens: Arc<AtomicU32>,
}

impl RateLimiter {
    pub async fn acquire(&self) -> Result<(), Error> {
        loop {
            let current = self.tokens.load(Ordering::Relaxed);
            
            if current > 0 {
                if self.tokens.compare_exchange(
                    current,
                    current - 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed
                ).is_ok() {
                    return Ok(());
                }
            } else {
                return Err(Error::RateLimitExceeded);
            }
        }
    }
}
```

---

## 6. 监控和告警

### 6.1 关键指标监控

**可用性指标：**
```yaml
# Prometheus 规则
- alert: GatewayDown
  expr: up{job="neuro-gateway"} == 0
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "Gateway instance is down"

- alert: HighErrorRate
  expr: rate(gateway_errors_total[5m]) / rate(gateway_requests_total[5m]) > 0.05
  for: 5m
  labels:
    severity: critical
```

**性能指标：**
```yaml
- alert: HighLatency
  expr: histogram_quantile(0.99, rate(gateway_request_duration_seconds_bucket[5m])) > 1.0
  for: 5m
  labels:
    severity: high

- alert: HighMemoryUsage
  expr: process_resident_memory_bytes / node_memory_MemTotal_bytes > 0.9
  for: 10m
  labels:
    severity: high
```

### 6.2 依赖健康监控

**Redis 健康检查：**
```rust
pub async fn check_redis_health(redis: &RedisPool) -> HealthStatus {
    match redis.get::<_, String>("health_check").await {
        Ok(_) => HealthStatus::Healthy,
        Err(e) if is_timeout(&e) => HealthStatus::Degraded,
        Err(_) => HealthStatus::Unhealthy,
    }
}
```

**数据库健康检查：**
```rust
pub async fn check_db_health(db: &PgPool) -> HealthStatus {
    match sqlx::query("SELECT 1").fetch_one(db).await {
        Ok(_) => {
            // 检查复制延迟
            let lag = check_replication_lag(db).await?;
            if lag > Duration::from_secs(60) {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            }
        }
        Err(_) => HealthStatus::Unhealthy,
    }
}
```

---

## 7. 灾难恢复演练

### 7.1 演练计划

**季度演练：**
- **Q1**：数据库故障转移演练
- **Q2**：区域切换演练
- **Q3**：全系统灾难恢复演练
- **Q4**：数据恢复演练

**演练步骤：**
```markdown
1. 准备阶段（T-1天）
   - 通知所有相关人员
   - 准备演练环境
   - 备份当前配置

2. 执行阶段（T+0）
   - 模拟故障场景
   - 执行恢复流程
   - 记录每个步骤的时间

3. 验证阶段（T+1小时）
   - 验证服务恢复
   - 验证数据完整性
   - 验证性能指标

4. 总结阶段（T+1天）
   - 分析演练结果
   - 识别改进点
   - 更新恢复文档
```

### 7.2 恢复流程文档

**数据库恢复 Runbook：**
```markdown
# 数据库故障恢复流程

## 场景：主数据库不可用

### 步骤 1：确认故障（预计 2 分钟）
- 检查数据库健康检查状态
- 检查 Patroni 集群状态
- 确认是否需要手动干预

### 步骤 2：触发故障转移（预计 1 分钟）
```bash
# 手动触发故障转移
patronictl failover neuro-gateway --candidate postgres2
```

### 步骤 3：验证恢复（预计 2 分钟）
```bash
# 检查新主库状态
psql -h postgres2 -c "SELECT pg_is_in_recovery();"

# 验证应用连接
curl https://gateway.neuroloom.ai/healthz
```

### 步骤 4：通知和记录
- 通知团队故障已恢复
- 记录故障原因和恢复时间
- 创建事后分析报告
```

---

## 8. 实施计划

### Phase 1: 基础高可用（Week 1-2）
- [ ] 部署多节点网关集群
- [ ] 配置负载均衡器
- [ ] 实现健康检查
- [ ] 配置 Redis 主从复制

### Phase 2: 数据库高可用（Week 3-4）
- [ ] 部署 Patroni 集群
- [ ] 配置自动故障转移
- [ ] 实现备份策略
- [ ] 验证恢复流程

### Phase 3: 区域容灾（Week 5-6）
- [ ] 部署多区域集群
- [ ] 配置跨区域复制
- [ ] 实现 DNS 故障转移
- [ ] 测试区域切换

### Phase 4: 降级和熔断（Week 7-8）
- [ ] 实现功能降级
- [ ] 实现熔断机制
- [ ] 配置限流保护
- [ ] 进行灾难恢复演练

---

## 9. 成功指标

- **可用性**：> 99.99%
- **RTO**：< 5 分钟
- **RPO**：< 1 分钟
- **MTTR**：< 15 分钟
- **备份成功率**：100%
- **演练通过率**：100%

---

**创建日期**: 2026-04-13  
**负责团队**: SRE Team + Infrastructure Team  
**状态**: 设计阶段
