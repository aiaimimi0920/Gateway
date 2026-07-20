# Observability Design - AI Gateway

## 目的

本文档定义 AI 网关的可观测性设计，包括分布式追踪、结构化日志、实时指标、告警策略和调试工具。

## 可观测性三大支柱

1. **Metrics（指标）**：系统的量化测量
2. **Logs（日志）**：离散事件记录
3. **Traces（追踪）**：请求的完整生命周期

---

## 1. 分布式追踪

### 1.1 OpenTelemetry 集成

**技术栈：**
- OpenTelemetry SDK for Rust
- OTLP Exporter (发送到 Jaeger/Tempo/Honeycomb)
- tracing + tracing-opentelemetry

**追踪数据流：**
```
Gateway Request → Span Created → Context Propagated → Upstream Call → Span Ended → OTLP Export → Jaeger/Tempo
```

### 1.2 Span 层级设计

**请求生命周期追踪：**
```
http_request (root span)
├── pipeline_stage_auth
│   ├── redis_get_credential
│   └── validate_api_key
├── pipeline_stage_filter
│   ├── content_filter_check
│   └── quota_pre_deduct
├── pipeline_stage_route
│   ├── load_provider_candidates
│   ├── score_providers
│   └── build_candidate_queue
├── pipeline_stage_send
│   ├── aimd_acquire_permit
│   ├── upstream_http_call
│   │   ├── dns_lookup
│   │   ├── tcp_connect
│   │   ├── tls_handshake
│   │   └── http_request_send
│   └── parse_response
└── pipeline_stage_finalize
    ├── usage_report_enqueue
    └── response_cache_store
```

**Span 属性：**
```rust
pub struct SpanAttributes {
    // HTTP 属性
    http_method: String,
    http_url: String,
    http_status_code: u16,
    http_request_size: u64,
    http_response_size: u64,
    
    // 业务属性
    request_id: String,
    user_id: Option<String>,
    project_id: String,
    model: String,
    provider: String,
    
    // 性能属性
    cache_hit: bool,
    tokens_input: u64,
    tokens_output: u64,
    
    // 错误属性
    error: bool,
    error_type: Option<String>,
    error_message: Option<String>,
}
```

### 1.3 采样策略

**多层采样：**
```rust
pub enum SamplingStrategy {
    AlwaysOn,           // 100% 采样（开发环境）
    AlwaysOff,          // 0% 采样
    TraceIdRatio(f64),  // 基于 trace_id 的固定比例采样
    ParentBased,        // 跟随父 span 的采样决策
    RateLimiting {      // 速率限制采样
        max_traces_per_second: u32,
    },
    Adaptive {          // 自适应采样
        base_rate: f64,
        error_boost: f64,  // 错误请求提高采样率
        slow_boost: f64,   // 慢请求提高采样率
    },
}
```

**自适应采样实现：**
```rust
pub struct AdaptiveSampler {
    base_rate: f64,
    error_boost: f64,
    slow_boost: f64,
    slow_threshold_ms: u64,
}

impl AdaptiveSampler {
    pub fn should_sample(&self, ctx: &RequestContext) -> bool {
        let mut rate = self.base_rate;
        
        // 错误请求提高采样率
        if ctx.has_error {
            rate = (rate + self.error_boost).min(1.0);
        }
        
        // 慢请求提高采样率
        if ctx.duration_ms > self.slow_threshold_ms {
            rate = (rate + self.slow_boost).min(1.0);
        }
        
        rand::random::<f64>() < rate
    }
}
```

---

## 2. 结构化日志

### 2.1 日志格式标准

**JSON 格式：**
```json
{
  "timestamp": "2026-04-13T10:30:00.123456Z",
  "level": "INFO",
  "target": "neuro_gateway::pipeline::stage_auth",
  "message": "authentication successful",
  "request_id": "req_abc123",
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
  "span_id": "00f067aa0ba902b7",
  "user_id": "user_456",
  "project_id": "proj_789",
  "duration_ms": 12,
  "fields": {
    "credential_type": "api_key",
    "scopes": ["chat", "embeddings"]
  }
}
```

### 2.2 日志级别使用指南

**ERROR**：需要立即处理的错误
- 数据库连接失败
- 上游服务不可用
- 认证系统故障
- 数据损坏

**WARN**：需要关注但不影响服务的问题
- 速率限制触发
- 缓存未命中率高
- 提供商响应慢
- 配置即将过期

**INFO**：正常业务操作
- 请求开始/完成
- 认证成功
- 路由决策
- 缓存命中

**DEBUG**：调试信息
- 详细的请求参数
- 中间计算结果
- 配置加载详情

**TRACE**：最详细的执行流程
- 函数进入/退出
- 循环迭代
- 条件分支

### 2.3 日志聚合和查询

**日志存储架构：**
```
Gateway → Fluentd/Vector → Elasticsearch/Loki → Kibana/Grafana
```

**常用查询：**
```
# 查询特定用户的所有错误
level:ERROR AND user_id:"user_456"

# 查询慢请求（>1秒）
duration_ms:>1000

# 查询特定提供商的失败请求
provider:"openai" AND (level:ERROR OR level:WARN)

# 查询缓存未命中的请求
cache_hit:false

# 按时间范围查询
timestamp:[2026-04-13T00:00:00 TO 2026-04-13T23:59:59]
```

---

## 3. 实时指标

### 3.1 核心指标定义

**RED 方法（Request, Error, Duration）：**
```rust
// Rate - 请求速率
gateway_requests_total
gateway_requests_per_second

// Errors - 错误率
gateway_errors_total
gateway_error_rate

// Duration - 请求延迟
gateway_request_duration_seconds (histogram)
  - P50, P95, P99, P99.9
```

**USE 方法（Utilization, Saturation, Errors）：**
```rust
// Utilization - 资源利用率
gateway_cpu_usage_percent
gateway_memory_usage_bytes
gateway_connection_pool_utilization

// Saturation - 资源饱和度
gateway_queue_length
gateway_pending_requests
gateway_connection_pool_wait_time

// Errors - 资源错误
gateway_connection_pool_errors_total
gateway_memory_allocation_failures_total
```

### 3.2 业务指标

**Token 使用：**
```rust
gateway_tokens_input_total
gateway_tokens_output_total
gateway_tokens_cached_total
gateway_tokens_per_request (histogram)
```

**成本指标：**
```rust
gateway_cost_usd_total
gateway_cost_per_request (histogram)
gateway_cost_savings_cache_usd_total
```

**提供商指标：**
```rust
gateway_provider_requests_total{provider="openai"}
gateway_provider_errors_total{provider="openai"}
gateway_provider_latency_seconds{provider="openai"}
gateway_provider_health_score{provider="openai"}
```

**缓存指标：**
```rust
gateway_cache_hits_total
gateway_cache_misses_total
gateway_cache_hit_rate
gateway_cache_size_bytes
gateway_cache_evictions_total
```

### 3.3 指标导出和可视化

**Prometheus 抓取配置：**
```yaml
scrape_configs:
  - job_name: 'neuro-gateway'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:9090']
    metric_relabel_configs:
      # 删除高基数标签
      - source_labels: [user_id]
        action: labeldrop
```

**Grafana 仪表板模板：**
```json
{
  "dashboard": {
    "title": "Neuro Gateway - Performance",
    "rows": [
      {
        "title": "Request Metrics",
        "panels": [
          {
            "title": "QPS",
            "targets": [{
              "expr": "rate(gateway_requests_total[1m])"
            }]
          },
          {
            "title": "Error Rate",
            "targets": [{
              "expr": "rate(gateway_errors_total[5m]) / rate(gateway_requests_total[5m])"
            }]
          },
          {
            "title": "Latency Percentiles",
            "targets": [
              {"expr": "histogram_quantile(0.50, rate(gateway_request_duration_seconds_bucket[5m]))"},
              {"expr": "histogram_quantile(0.95, rate(gateway_request_duration_seconds_bucket[5m]))"},
              {"expr": "histogram_quantile(0.99, rate(gateway_request_duration_seconds_bucket[5m]))"}
            ]
          }
        ]
      }
    ]
  }
}
```

---

## 4. SLI/SLO 定义

### 4.1 服务级别指标 (SLI)

**可用性 SLI：**
```
Availability = (Successful Requests / Total Requests) × 100%

成功请求定义：HTTP 状态码 2xx 或 3xx
```

**延迟 SLI：**
```
Latency SLI = (Requests under threshold / Total Requests) × 100%

阈值定义：
- P50 < 10ms
- P95 < 50ms
- P99 < 100ms
```

**质量 SLI：**
```
Quality SLI = (Requests without errors / Total Requests) × 100%

错误定义：
- 上游 5xx 错误
- 超时
- 连接失败
```

### 4.2 服务级别目标 (SLO)

**SLO 定义表：**

| SLI | SLO Target | 时间窗口 | 错误预算 |
|-----|-----------|---------|---------|
| 可用性 | 99.99% | 30 天 | 0.01% (43 分钟) |
| P99 延迟 < 100ms | 95% | 7 天 | 5% |
| 错误率 < 1% | 99% | 24 小时 | 1% |

**错误预算计算：**
```rust
pub struct ErrorBudget {
    slo_target: f64,      // 例如 0.9999
    window_seconds: u64,  // 例如 30 * 24 * 3600
}

impl ErrorBudget {
    pub fn total_budget_seconds(&self) -> u64 {
        ((1.0 - self.slo_target) * self.window_seconds as f64) as u64
    }
    
    pub fn consumed_seconds(&self, current_sli: f64) -> u64 {
        ((1.0 - current_sli) * self.window_seconds as f64) as u64
    }
    
    pub fn remaining_seconds(&self, current_sli: f64) -> i64 {
        self.total_budget_seconds() as i64 - self.consumed_seconds(current_sli) as i64
    }
    
    pub fn burn_rate(&self, error_rate_5m: f64) -> f64 {
        // 当前 5 分钟错误率相对于 SLO 的倍数
        error_rate_5m / (1.0 - self.slo_target)
    }
}
```

### 4.3 SLO 告警

**多窗口多燃烧率告警：**
```yaml
# 快速燃烧（1小时内消耗 5% 错误预算）
- alert: ErrorBudgetFastBurn
  expr: |
    (1 - (sum(rate(gateway_requests_total{status=~"2.."}[1h])) / 
          sum(rate(gateway_requests_total[1h])))) > 0.0005
  labels:
    severity: critical
  annotations:
    summary: "Error budget burning at 14.4x rate"

# 慢速燃烧（6小时内消耗 10% 错误预算）
- alert: ErrorBudgetSlowBurn
  expr: |
    (1 - (sum(rate(gateway_requests_total{status=~"2.."}[6h])) / 
          sum(rate(gateway_requests_total[6h])))) > 0.00017
  labels:
    severity: high
  annotations:
    summary: "Error budget burning at 2.4x rate"
```

---

## 5. 告警策略

### 5.1 告警分级

**Critical（P0）**：立即响应，影响服务
- 可用性 < 99.9%
- 错误率 > 5%
- P99 延迟 > 1 秒
- 数据库连接失败
- 所有提供商不可用

**High（P1）**：1 小时内响应
- 可用性 < 99.95%
- 错误率 > 2%
- P99 延迟 > 500ms
- 错误预算快速燃烧
- 单个提供商完全不可用

**Medium（P2）**：4 小时内响应
- 可用性 < 99.99%
- 错误率 > 1%
- P99 延迟 > 200ms
- 缓存命中率 < 50%
- 内存使用 > 80%

**Low（P3）**：下一个工作日响应
- 慢查询检测
- 配置即将过期
- 磁盘使用 > 70%

### 5.2 告警抑制和分组

**抑制规则：**
```yaml
# 如果整个网关不可用，抑制单个提供商的告警
inhibit_rules:
  - source_match:
      alertname: GatewayDown
    target_match_re:
      alertname: Provider.*Down
    equal: ['cluster']
```

**分组规则：**
```yaml
# 按服务和严重性分组
route:
  group_by: ['alertname', 'severity', 'cluster']
  group_wait: 30s
  group_interval: 5m
  repeat_interval: 4h
```

### 5.3 通知渠道

**Slack 通知：**
```rust
pub async fn send_slack_alert(webhook_url: &str, alert: &Alert) -> Result<(), Error> {
    let payload = json!({
        "text": format!("[{}] {}", alert.severity, alert.name),
        "attachments": [{
            "color": match alert.severity {
                AlertSeverity::Critical => "danger",
                AlertSeverity::High => "warning",
                _ => "good",
            },
            "fields": [
                {"title": "Description", "value": alert.description, "short": false},
                {"title": "Threshold", "value": alert.threshold, "short": true},
                {"title": "Current Value", "value": alert.current_value, "short": true},
            ]
        }]
    });
    
    reqwest::Client::new()
        .post(webhook_url)
        .json(&payload)
        .send()
        .await?;
    
    Ok(())
}
```

---

## 6. 调试工具

### 6.1 请求重放

**用途：**
- 重现生产问题
- 性能回归测试
- A/B 测试不同版本

**实现：**
```rust
pub struct RequestRecorder {
    storage: Arc<dyn Storage>,
    sample_rate: f64,
}

impl RequestRecorder {
    pub async fn record(&self, req: &Request, resp: &Response) -> Result<(), Error> {
        if rand::random::<f64>() > self.sample_rate {
            return Ok(());
        }
        
        let record = RequestRecord {
            id: generate_id(),
            timestamp: Utc::now(),
            method: req.method().to_string(),
            uri: req.uri().to_string(),
            headers: serialize_headers(req.headers()),
            body: read_body(req).await?,
            response_status: resp.status().as_u16(),
            response_headers: serialize_headers(resp.headers()),
            response_body: read_body(resp).await?,
        };
        
        self.storage.save(&record).await?;
        Ok(())
    }
    
    pub async fn replay(&self, record_id: &str) -> Result<ReplayResult, Error> {
        let record = self.storage.load(record_id).await?;
        
        // 重建请求
        let req = rebuild_request(&record)?;
        
        // 执行请求
        let start = Instant::now();
        let resp = execute_request(req).await?;
        let duration = start.elapsed();
        
        // 比较结果
        let diff = compare_responses(&record.response_body, &resp)?;
        
        Ok(ReplayResult {
            original_status: record.response_status,
            replay_status: resp.status().as_u16(),
            original_duration: record.duration,
            replay_duration: duration,
            diff,
        })
    }
}
```

### 6.2 流量镜像

**用途：**
- 测试新版本
- 压力测试
- 数据收集

**实现：**
```rust
pub struct TrafficMirror {
    target_url: String,
    mirror_rate: f64,
    async_send: bool,
}

impl TrafficMirror {
    pub async fn mirror(&self, req: &Request) -> Result<(), Error> {
        if rand::random::<f64>() > self.mirror_rate {
            return Ok(());
        }
        
        let mirror_req = req.clone();
        let target_url = self.target_url.clone();
        
        if self.async_send {
            // 异步发送，不等待响应
            tokio::spawn(async move {
                let _ = send_to_target(&target_url, mirror_req).await;
            });
        } else {
            // 同步发送，等待响应
            send_to_target(&self.target_url, mirror_req).await?;
        }
        
        Ok(())
    }
}
```

### 6.3 实时性能分析

**CPU Profiling：**
```rust
// 使用 pprof
#[cfg(feature = "profiling")]
pub async fn start_profiling() -> Result<(), Error> {
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread"])
        .build()?;
    
    // 运行 60 秒
    tokio::time::sleep(Duration::from_secs(60)).await;
    
    // 生成火焰图
    if let Ok(report) = guard.report().build() {
        let file = File::create("flamegraph.svg")?;
        report.flamegraph(file)?;
    }
    
    Ok(())
}
```

**内存分析：**
```rust
// 使用 jemalloc profiling
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

pub fn dump_memory_profile() -> Result<(), Error> {
    let mut prof_ctl = jemalloc_ctl::prof::ctl()?;
    prof_ctl.dump("memory_profile.heap")?;
    Ok(())
}
```

---

## 7. 实施计划

### Phase 1: 基础设施（Week 1-2）
- [ ] 部署 Prometheus + Grafana
- [ ] 部署 Jaeger/Tempo
- [ ] 部署 Elasticsearch/Loki
- [ ] 配置日志收集管道

### Phase 2: 集成（Week 3-4）
- [ ] 集成 OpenTelemetry
- [ ] 实现结构化日志
- [ ] 导出 Prometheus 指标
- [ ] 创建基础仪表板

### Phase 3: SLO 和告警（Week 5-6）
- [ ] 定义 SLI/SLO
- [ ] 配置告警规则
- [ ] 集成通知渠道
- [ ] 实现错误预算追踪

### Phase 4: 高级功能（Week 7-8）
- [ ] 实现请求重放
- [ ] 配置流量镜像
- [ ] 添加性能分析工具
- [ ] 创建调试仪表板

---

## 8. 成功指标

- **追踪覆盖率**：100% 的请求被追踪
- **日志完整性**：所有错误都有详细日志
- **指标准确性**：指标与实际业务一致
- **告警准确率**：误报率 < 5%
- **MTTR**：平均恢复时间 < 15 分钟
- **SLO 达成率**：> 99.9%

---

**创建日期**: 2026-04-13  
**负责团队**: SRE Team + Rust Gateway Team  
**状态**: 设计阶段
