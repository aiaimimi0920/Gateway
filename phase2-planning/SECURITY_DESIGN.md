# Security Design - AI Gateway

## 目的

本文档定义 AI 网关的安全设计，涵盖认证、授权、数据保护、审计和威胁防护等方面。

## 安全目标

1. **机密性**：保护 API 密钥、用户凭证、提供商凭证不被泄露
2. **完整性**：确保请求和响应数据不被篡改
3. **可用性**：防止 DoS 攻击，确保服务可用
4. **可审计性**：记录所有安全相关事件
5. **合规性**：满足 GDPR、SOC2 等合规要求

---

## 1. 认证安全

### 1.1 API Key 管理

**密钥生成：**
```rust
// 使用加密安全的随机数生成器
use rand::rngs::OsRng;
use rand::RngCore;

pub fn generate_api_key() -> String {
    let mut key_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut key_bytes);
    
    // 格式：gw-{version}-{base64url(random_bytes)}
    format!("gw-v1-{}", base64_url::encode(&key_bytes))
}
```

**密钥存储：**
- **数据库存储**：只存储密钥的 SHA-256 哈希
- **传输加密**：TLS 1.3 强制加密
- **内存保护**：使用 `secrecy` crate 防止密钥泄露到日志

```rust
use secrecy::{Secret, ExposeSecret};

pub struct ApiKey {
    key_hash: String,           // SHA-256 哈希（存储）
    key_value: Secret<String>,  // 原始值（仅在验证时使用）
}
```

**密钥轮换策略：**
- **自动轮换**：每 90 天强制轮换
- **手动轮换**：用户可随时轮换
- **宽限期**：旧密钥在轮换后 7 天内仍然有效
- **轮换通知**：提前 14 天通知用户

### 1.2 用户凭证安全

**凭证类型：**
1. **长期凭证**：用户 API 密钥（90 天有效期）
2. **短期凭证**：会话令牌（24 小时有效期）
3. **一次性凭证**：临时访问令牌（1 小时有效期）

**凭证验证流程：**
```rust
pub async fn verify_credential(
    credential: &str,
    redis: &RedisPool,
    db: &PgPool,
) -> Result<AuthenticatedSession, AuthError> {
    // 1. 检查黑名单（Redis）
    if is_revoked(credential, redis).await? {
        return Err(AuthError::CredentialRevoked);
    }
    
    // 2. 验证签名/哈希
    let credential_hash = sha256(credential);
    
    // 3. 查询数据库
    let cred = sqlx::query_as::<_, UserCredential>(
        "SELECT * FROM gateway_user_credentials 
         WHERE credential_key_hash = $1 
         AND status = 'active'
         AND (expires_at IS NULL OR expires_at > NOW())"
    )
    .bind(&credential_hash)
    .fetch_optional(db)
    .await?;
    
    // 4. 检查速率限制
    check_rate_limit(&cred.user_id, redis).await?;
    
    Ok(AuthenticatedSession::from(cred))
}
```

### 1.3 提供商凭证保护

**加密存储：**
```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

pub struct EncryptedCredential {
    encrypted_value: Vec<u8>,
    nonce: [u8; 12],
    tag: [u8; 16],
}

pub fn encrypt_provider_credential(
    plaintext: &str,
    master_key: &[u8; 32],
) -> EncryptedCredential {
    let cipher = Aes256Gcm::new(Key::from_slice(master_key));
    let nonce = Nonce::from_slice(&generate_nonce());
    
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
        .expect("encryption failure");
    
    EncryptedCredential {
        encrypted_value: ciphertext,
        nonce: nonce.as_slice().try_into().unwrap(),
        tag: extract_tag(&ciphertext),
    }
}
```

**密钥管理：**
- **主密钥**：存储在 AWS KMS / HashiCorp Vault
- **密钥轮换**：每年轮换一次主密钥
- **密钥分离**：不同环境使用不同的主密钥

---

## 2. 授权模型

### 2.1 基于角色的访问控制 (RBAC)

**角色定义：**
```rust
pub enum Role {
    Admin,           // 完全访问权限
    Developer,       // 开发者权限（读写 API 密钥）
    Viewer,          // 只读权限
    BillingManager,  // 计费管理权限
}

pub struct Permission {
    resource: Resource,
    action: Action,
}

pub enum Resource {
    ApiKey,
    UserCredential,
    ProviderCredential,
    RoutePolicy,
    RequestAudit,
    Billing,
}

pub enum Action {
    Create,
    Read,
    Update,
    Delete,
    List,
}
```

**权限检查：**
```rust
pub fn check_permission(
    session: &AuthenticatedSession,
    resource: Resource,
    action: Action,
) -> Result<(), AuthError> {
    let required_permission = Permission { resource, action };
    
    if !session.has_permission(&required_permission) {
        return Err(AuthError::Forbidden);
    }
    
    Ok(())
}
```

### 2.2 基于属性的访问控制 (ABAC)

**属性定义：**
```rust
pub struct AccessContext {
    user_id: String,
    project_id: String,
    tenant_id: String,
    ip_address: IpAddr,
    time: DateTime<Utc>,
    resource_owner: String,
}

pub struct Policy {
    effect: Effect,
    conditions: Vec<Condition>,
}

pub enum Effect {
    Allow,
    Deny,
}

pub enum Condition {
    UserIs(String),
    ProjectIs(String),
    IpInRange(IpNetwork),
    TimeInRange(TimeRange),
    ResourceOwnedBy(String),
}
```

**策略评估：**
```rust
pub fn evaluate_policy(
    context: &AccessContext,
    policies: &[Policy],
) -> bool {
    // 默认拒绝
    let mut allowed = false;
    
    for policy in policies {
        if policy.matches(context) {
            match policy.effect {
                Effect::Allow => allowed = true,
                Effect::Deny => return false, // 显式拒绝优先
            }
        }
    }
    
    allowed
}
```

---

## 3. 速率限制和 DoS 防护

### 3.1 多层速率限制

**层级 1：全局速率限制**
```rust
// 防止整个网关过载
pub struct GlobalRateLimiter {
    max_requests_per_second: u32,  // 10,000 req/s
    current_load: AtomicU32,
}
```

**层级 2：租户级速率限制**
```rust
// 防止单个租户占用过多资源
pub struct TenantRateLimiter {
    tenant_id: String,
    max_requests_per_minute: u32,  // 1,000 req/min
    max_concurrent_requests: u32,   // 100 concurrent
}
```

**层级 3：用户级速率限制**
```rust
// 防止单个用户滥用
pub struct UserRateLimiter {
    user_id: String,
    max_requests_per_minute: u32,  // 100 req/min
    max_tokens_per_day: u64,        // 1M tokens/day
}
```

**层级 4：IP 级速率限制**
```rust
// 防止 DDoS 攻击
pub struct IpRateLimiter {
    ip_address: IpAddr,
    max_requests_per_second: u32,  // 10 req/s
    max_failed_auth_per_hour: u32, // 10 failures/hour
}
```

**实现（使用 Token Bucket 算法）：**
```rust
use std::time::{Duration, Instant};

pub struct TokenBucket {
    capacity: u32,
    tokens: AtomicU32,
    refill_rate: u32,  // tokens per second
    last_refill: Mutex<Instant>,
}

impl TokenBucket {
    pub async fn acquire(&self, tokens: u32) -> Result<(), RateLimitError> {
        self.refill().await;
        
        let current = self.tokens.load(Ordering::Relaxed);
        if current >= tokens {
            self.tokens.fetch_sub(tokens, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RateLimitError::Exceeded {
                retry_after: self.calculate_retry_after(tokens),
            })
        }
    }
    
    async fn refill(&self) {
        let mut last_refill = self.last_refill.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill);
        
        let tokens_to_add = (elapsed.as_secs_f64() * self.refill_rate as f64) as u32;
        if tokens_to_add > 0 {
            let current = self.tokens.load(Ordering::Relaxed);
            let new_tokens = (current + tokens_to_add).min(self.capacity);
            self.tokens.store(new_tokens, Ordering::Relaxed);
            *last_refill = now;
        }
    }
}
```

### 3.2 DDoS 防护

**检测机制：**
```rust
pub struct DDoSDetector {
    // 异常流量检测
    baseline_rps: f64,
    current_rps: AtomicU64,
    
    // 异常 IP 检测
    suspicious_ips: DashMap<IpAddr, SuspicionScore>,
}

pub struct SuspicionScore {
    failed_auth_count: u32,
    rate_limit_violations: u32,
    unusual_patterns: u32,
    score: f64,  // 0.0 - 1.0
}

impl DDoSDetector {
    pub fn analyze_request(&self, req: &Request) -> ThreatLevel {
        let ip = req.client_ip();
        let score = self.calculate_suspicion_score(ip);
        
        if score > 0.9 {
            ThreatLevel::Critical
        } else if score > 0.7 {
            ThreatLevel::High
        } else if score > 0.5 {
            ThreatLevel::Medium
        } else {
            ThreatLevel::Low
        }
    }
}
```

**防护措施：**
```rust
pub enum DefenseAction {
    Allow,
    Challenge,      // CAPTCHA 验证
    Throttle,       // 降低速率
    Block,          // 临时封禁
    Blacklist,      // 永久封禁
}

pub fn apply_defense(threat_level: ThreatLevel) -> DefenseAction {
    match threat_level {
        ThreatLevel::Low => DefenseAction::Allow,
        ThreatLevel::Medium => DefenseAction::Throttle,
        ThreatLevel::High => DefenseAction::Challenge,
        ThreatLevel::Critical => DefenseAction::Block,
    }
}
```

---

## 4. 数据安全

### 4.1 传输加密

**TLS 配置：**
```rust
use rustls::{ServerConfig, NoClientAuth};

pub fn create_tls_config() -> ServerConfig {
    let mut config = ServerConfig::new(NoClientAuth::new());
    
    // 只允许 TLS 1.3
    config.versions = vec![ProtocolVersion::TLSv1_3];
    
    // 强加密套件
    config.ciphersuites = vec![
        CipherSuite::TLS13_AES_256_GCM_SHA384,
        CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
    ];
    
    config
}
```

**HSTS 头：**
```rust
// 强制 HTTPS
response.headers_mut().insert(
    "Strict-Transport-Security",
    "max-age=31536000; includeSubDomains; preload".parse().unwrap()
);
```

### 4.2 静态数据加密

**数据库加密：**
```sql
-- PostgreSQL 透明数据加密 (TDE)
ALTER DATABASE neuroloom SET encryption = 'AES256';

-- 列级加密（敏感字段）
CREATE TABLE gateway_user_credentials (
    id TEXT PRIMARY KEY,
    credential_key_hash TEXT NOT NULL,  -- SHA-256 哈希
    api_key_encrypted BYTEA,            -- AES-256-GCM 加密
    encryption_key_id TEXT NOT NULL,    -- KMS 密钥 ID
    ...
);
```

**Redis 加密：**
```rust
// Redis 传输加密
let redis_config = RedisConfig {
    url: "rediss://localhost:6379",  // TLS
    tls: Some(TlsConfig {
        ca_cert: "/path/to/ca.crt",
        client_cert: "/path/to/client.crt",
        client_key: "/path/to/client.key",
    }),
};
```

### 4.3 敏感信息脱敏

**日志脱敏：**
```rust
use regex::Regex;

pub fn sanitize_log(message: &str) -> String {
    let patterns = vec![
        (r"gw-v1-[A-Za-z0-9_-]{43}", "gw-v1-***"),           // API keys
        (r"sk-[A-Za-z0-9]{48}", "sk-***"),                   // OpenAI keys
        (r"Bearer [A-Za-z0-9_-]+", "Bearer ***"),            // Bearer tokens
        (r"\b\d{16}\b", "****-****-****-****"),              // Credit cards
        (r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b", "***@***.com"), // Emails
    ];
    
    let mut sanitized = message.to_string();
    for (pattern, replacement) in patterns {
        let re = Regex::new(pattern).unwrap();
        sanitized = re.replace_all(&sanitized, replacement).to_string();
    }
    
    sanitized
}
```

**请求/响应脱敏：**
```rust
pub fn sanitize_request_body(body: &mut serde_json::Value) {
    // 移除敏感字段
    if let Some(obj) = body.as_object_mut() {
        obj.remove("api_key");
        obj.remove("password");
        obj.remove("secret");
        obj.remove("token");
        
        // 截断长文本
        if let Some(messages) = obj.get_mut("messages") {
            if let Some(arr) = messages.as_array_mut() {
                for msg in arr {
                    if let Some(content) = msg.get_mut("content") {
                        if let Some(text) = content.as_str() {
                            if text.len() > 1000 {
                                *content = json!(format!("{}... (truncated)", &text[..1000]));
                            }
                        }
                    }
                }
            }
        }
    }
}
```

---

## 5. 审计日志

### 5.1 审计事件定义

**安全事件：**
```rust
pub enum SecurityEvent {
    // 认证事件
    AuthSuccess { user_id: String, ip: IpAddr },
    AuthFailure { credential: String, ip: IpAddr, reason: String },
    CredentialCreated { credential_id: String, created_by: String },
    CredentialRevoked { credential_id: String, revoked_by: String, reason: String },
    
    // 授权事件
    AccessGranted { user_id: String, resource: String, action: String },
    AccessDenied { user_id: String, resource: String, action: String, reason: String },
    
    // 速率限制事件
    RateLimitExceeded { user_id: String, limit_type: String, ip: IpAddr },
    
    // 异常事件
    SuspiciousActivity { ip: IpAddr, pattern: String, severity: String },
    DataBreach { resource: String, accessed_by: String },
    
    // 配置变更
    ConfigChanged { changed_by: String, resource: String, old_value: String, new_value: String },
}
```

**审计日志格式：**
```rust
pub struct AuditLog {
    id: String,
    timestamp: DateTime<Utc>,
    event_type: SecurityEvent,
    user_id: Option<String>,
    ip_address: IpAddr,
    user_agent: String,
    request_id: String,
    session_id: Option<String>,
    metadata: serde_json::Value,
}
```

### 5.2 审计日志存储

**数据库表：**
```sql
CREATE TABLE gateway_audit_logs (
    id TEXT PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    event_type TEXT NOT NULL,
    user_id TEXT,
    ip_address INET NOT NULL,
    user_agent TEXT,
    request_id TEXT,
    session_id TEXT,
    metadata JSONB,
    
    INDEX idx_audit_timestamp (timestamp DESC),
    INDEX idx_audit_user (user_id, timestamp DESC),
    INDEX idx_audit_event_type (event_type, timestamp DESC),
    INDEX idx_audit_ip (ip_address, timestamp DESC)
);

-- 分区表（按月分区）
CREATE TABLE gateway_audit_logs_2026_04 PARTITION OF gateway_audit_logs
    FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');
```

**日志保留策略：**
- **热数据**：最近 30 天，存储在 PostgreSQL
- **温数据**：31-365 天，存储在 PostgreSQL（压缩）
- **冷数据**：365 天以上，归档到 S3（Parquet 格式）

### 5.3 审计日志查询

**查询 API：**
```rust
pub async fn query_audit_logs(
    filters: AuditLogFilters,
    db: &PgPool,
) -> Result<Vec<AuditLog>, DbError> {
    let mut query = String::from("SELECT * FROM gateway_audit_logs WHERE 1=1");
    
    if let Some(user_id) = filters.user_id {
        query.push_str(&format!(" AND user_id = '{}'", user_id));
    }
    
    if let Some(event_type) = filters.event_type {
        query.push_str(&format!(" AND event_type = '{}'", event_type));
    }
    
    if let Some(start_time) = filters.start_time {
        query.push_str(&format!(" AND timestamp >= '{}'", start_time));
    }
    
    query.push_str(" ORDER BY timestamp DESC LIMIT 1000");
    
    sqlx::query_as(&query).fetch_all(db).await
}
```

---

## 6. 漏洞响应

### 6.1 漏洞分类

**严重性级别：**
```rust
pub enum Severity {
    Critical,  // CVSS 9.0-10.0
    High,      // CVSS 7.0-8.9
    Medium,    // CVSS 4.0-6.9
    Low,       // CVSS 0.1-3.9
}

pub struct Vulnerability {
    id: String,
    severity: Severity,
    cve_id: Option<String>,
    description: String,
    affected_versions: Vec<String>,
    fixed_version: Option<String>,
    workaround: Option<String>,
}
```

### 6.2 响应流程

**Critical/High 漏洞：**
1. **发现后 1 小时内**：评估影响范围
2. **发现后 4 小时内**：制定修复计划
3. **发现后 24 小时内**：发布补丁或临时缓解措施
4. **发现后 48 小时内**：通知所有受影响用户

**Medium/Low 漏洞：**
1. **发现后 1 周内**：评估和修复
2. **发现后 2 周内**：发布补丁
3. **下一个版本**：包含在常规更新中

### 6.3 安全更新机制

**自动更新检查：**
```rust
pub async fn check_security_updates() -> Result<Vec<SecurityUpdate>, Error> {
    let response = reqwest::get("https://security.neuroloom.ai/updates.json").await?;
    let updates: Vec<SecurityUpdate> = response.json().await?;
    
    // 过滤适用于当前版本的更新
    let current_version = env!("CARGO_PKG_VERSION");
    let applicable_updates = updates.into_iter()
        .filter(|u| u.affects_version(current_version))
        .collect();
    
    Ok(applicable_updates)
}
```

**紧急补丁部署：**
```bash
# 热补丁（无需重启）
curl -X POST https://gateway.neuroloom.ai/v1/internal/security/hotpatch \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -d '{"patch_id": "CVE-2026-12345", "action": "apply"}'

# 滚动重启（逐个实例重启）
kubectl rollout restart deployment/neuro-gateway
```

---

## 7. 合规性

### 7.1 GDPR 合规

**数据主体权利：**
```rust
pub enum DataSubjectRight {
    AccessRight,        // 访问权
    RectificationRight, // 更正权
    ErasureRight,       // 删除权（被遗忘权）
    PortabilityRight,   // 数据可携权
    ObjectionRight,     // 反对权
}

pub async fn handle_data_subject_request(
    user_id: &str,
    right: DataSubjectRight,
    db: &PgPool,
) -> Result<DataSubjectResponse, Error> {
    match right {
        DataSubjectRight::AccessRight => {
            // 导出用户所有数据
            export_user_data(user_id, db).await
        }
        DataSubjectRight::ErasureRight => {
            // 删除用户所有数据（保留审计日志）
            anonymize_user_data(user_id, db).await
        }
        // ... 其他权利
    }
}
```

**数据最小化：**
- 只收集必要的数据
- 定期清理过期数据
- 匿名化历史数据

### 7.2 SOC 2 合规

**控制措施：**
1. **访问控制**：RBAC + MFA
2. **加密**：传输加密 + 静态加密
3. **监控**：实时监控 + 告警
4. **审计**：完整的审计日志
5. **变更管理**：所有变更需审批和记录
6. **事件响应**：24/7 安全事件响应

**合规检查清单：**
```rust
pub struct ComplianceChecklist {
    access_control: bool,
    encryption_at_rest: bool,
    encryption_in_transit: bool,
    audit_logging: bool,
    incident_response_plan: bool,
    vulnerability_management: bool,
    change_management: bool,
    backup_and_recovery: bool,
}

pub async fn run_compliance_check() -> ComplianceChecklist {
    ComplianceChecklist {
        access_control: check_rbac_enabled().await,
        encryption_at_rest: check_database_encryption().await,
        encryption_in_transit: check_tls_enabled().await,
        audit_logging: check_audit_logs_enabled().await,
        incident_response_plan: check_incident_plan_exists().await,
        vulnerability_management: check_security_updates().await.is_ok(),
        change_management: check_change_approval_process().await,
        backup_and_recovery: check_backup_status().await,
    }
}
```

---

## 8. 安全测试

### 8.1 渗透测试

**测试范围：**
- 认证绕过
- 授权漏洞
- SQL 注入
- XSS 攻击
- CSRF 攻击
- API 滥用
- 速率限制绕过

**测试频率：**
- 每季度进行一次完整渗透测试
- 每次重大更新后进行安全测试

### 8.2 漏洞扫描

**自动化扫描：**
```yaml
# .github/workflows/security-scan.yml
name: Security Scan

on:
  push:
    branches: [main]
  schedule:
    - cron: '0 0 * * *'  # 每天运行

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run cargo audit
        run: cargo audit
      
      - name: Run SAST
        run: cargo clippy -- -D warnings
      
      - name: Run dependency check
        uses: actions-rs/audit-check@v1
```

### 8.3 安全代码审查

**审查清单：**
- [ ] 输入验证
- [ ] 输出编码
- [ ] 认证检查
- [ ] 授权检查
- [ ] 敏感数据处理
- [ ] 错误处理
- [ ] 日志记录
- [ ] 加密使用

---

## 9. 实施计划

### Phase 1: 基础安全（Week 1-2）
- [ ] 实现 API Key 安全存储
- [ ] 实现基础 RBAC
- [ ] 实现速率限制
- [ ] 启用 TLS 1.3

### Phase 2: 高级防护（Week 3-4）
- [ ] 实现 DDoS 防护
- [ ] 实现数据加密
- [ ] 实现审计日志
- [ ] 实现敏感信息脱敏

### Phase 3: 合规性（Week 5-6）
- [ ] GDPR 合规实施
- [ ] SOC 2 控制措施
- [ ] 安全测试
- [ ] 文档完善

### Phase 4: 持续改进（Week 7+）
- [ ] 定期渗透测试
- [ ] 漏洞扫描自动化
- [ ] 安全培训
- [ ] 事件响应演练

---

## 10. 成功指标

- **安全事件数量**：< 1 次/月
- **漏洞修复时间**：Critical < 24h, High < 7d
- **审计日志完整性**：100%
- **合规性检查通过率**：100%
- **渗透测试通过率**：> 95%

---

**创建日期**: 2026-04-13  
**负责团队**: Security Team + Rust Gateway Team  
**状态**: 设计阶段
