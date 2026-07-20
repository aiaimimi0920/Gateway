# Performance Benchmarks for Rust Gateway

## Purpose

This document defines clear performance targets for the Rust gateway and how to measure them. It also provides baseline metrics from the TypeScript gateway for comparison.

## Performance Targets

### Latency (Gateway Overhead Only)

Gateway overhead = time from receiving request to sending upstream request + time from receiving upstream response to sending client response.

| Metric | Target | Rationale |
|--------|--------|-----------|
| P50 latency | < 5ms | Most requests should have minimal overhead |
| P95 latency | < 15ms | 95% of requests should be fast |
| P99 latency | < 30ms | Even tail latency should be acceptable |
| P99.9 latency | < 100ms | Extreme outliers should still be reasonable |

### Throughput

| Metric | Target | Rationale |
|--------|--------|-----------|
| Requests/second (single instance) | > 10,000 | High throughput for cost efficiency |
| Concurrent SSE connections | > 50,000 | Support many streaming clients |
| Requests/second (streaming) | > 5,000 | Streaming has higher overhead |

### Resource Usage

| Metric | Target | Rationale |
|--------|--------|-----------|
| Resident memory (idle) | < 50MB | Low baseline memory usage |
| Resident memory (1000 req/s) | < 100MB | Efficient under load |
| Peak memory (10000 req/s) | < 500MB | Bounded memory growth |
| CPU usage (1000 req/s, single core) | < 30% | Leave headroom for spikes |

### Database Performance

| Metric | Target | Rationale |
|--------|--------|-----------|
| Credential lookup (cache hit) | < 1ms | Redis should be very fast |
| Credential lookup (cache miss) | < 10ms | PostgreSQL query should be fast |
| Credential issuance | < 50ms | Write + cache update |
| Connection pool exhaustion | Never | Proper pool sizing |

## Baseline Metrics (TypeScript Gateway)

These are measured metrics from the legacy TypeScript gateway that was replaced by Rust `gateway/`.

### Latency

| Metric | TypeScript Baseline | Rust Target | Improvement |
|--------|---------------------|-------------|-------------|
| P50 latency | ~50ms | < 5ms | **10x faster** |
| P95 latency | ~150ms | < 15ms | **10x faster** |
| P99 latency | ~300ms | < 30ms | **10x faster** |

### Throughput

| Metric | TypeScript Baseline | Rust Target | Improvement |
|--------|---------------------|-------------|-------------|
| Requests/second | ~2,000 | > 10,000 | **5x higher** |
| Concurrent connections | ~5,000 | > 50,000 | **10x higher** |

### Resource Usage

| Metric | TypeScript Baseline | Rust Target | Improvement |
|--------|---------------------|-------------|-------------|
| Resident memory | ~200MB | < 50MB | **4x lower** |
| Peak memory | ~500MB | < 500MB | Same |
| CPU (1000 req/s) | ~80% (single core) | < 30% | **2.5x more efficient** |

## How to Measure

### 1. Latency Benchmarks

Use `wrk` for HTTP load testing:

```bash
# Install wrk
# macOS: brew install wrk
# Linux: apt-get install wrk

# Basic latency test (non-streaming)
wrk -t12 -c100 -d30s \
  -H "Authorization: Bearer test-key" \
  -H "Content-Type: application/json" \
  -s scripts/chat-completion.lua \
  http://localhost:4200/v1/chat/completions

# High concurrency test
wrk -t12 -c1000 -d30s \
  -H "Authorization: Bearer test-key" \
  -H "Content-Type: application/json" \
  -s scripts/chat-completion.lua \
  http://localhost:4200/v1/chat/completions
```

**Lua script** (`scripts/chat-completion.lua`):
```lua
wrk.method = "POST"
wrk.body = [[{
  "model": "gpt-4",
  "messages": [{"role": "user", "content": "Hello"}],
  "stream": false
}]]
wrk.headers["Content-Type"] = "application/json"
```

### 2. Streaming Benchmarks

Use custom streaming test tool:

```bash
# Build streaming test tool
cd gateway/tools/stream-bench
cargo build --release

# Run streaming benchmark
./target/release/stream-bench \
  --url http://localhost:4200/v1/chat/completions \
  --auth "Bearer test-key" \
  --concurrent 1000 \
  --duration 30s
```

### 3. Memory Profiling

Use `valgrind` and `heaptrack` for memory analysis:

```bash
# Install heaptrack
# Linux: apt-get install heaptrack
# macOS: brew install heaptrack

# Profile memory usage
heaptrack ./target/release/neuro-gateway

# Generate report
heaptrack_print heaptrack.neuro-gateway.*.gz
```

### 4. CPU Profiling

Use `perf` and `flamegraph` for CPU analysis:

```bash
# Install flamegraph
cargo install flamegraph

# Profile CPU usage
cargo flamegraph --bin neuro-gateway

# Open flamegraph.svg in browser
```

### 5. Database Performance

Use custom database benchmark:

```bash
# Run database benchmark
cd gateway/tools/db-bench
cargo run --release -- \
  --database-url postgresql://neuroloom:neuroloom@localhost:5432/neuroloom \
  --operations 10000 \
  --concurrent 100
```

## Benchmark Scenarios

### Scenario 1: Credential Verification (Hot Path)

**Setup**:
- 1000 pre-issued user credentials in database
- Redis cache warmed up
- 100 concurrent clients

**Test**:
```bash
wrk -t12 -c100 -d30s \
  -H "Authorization: Bearer gw-user-test-credential" \
  -s scripts/verify-credential.lua \
  http://localhost:4200/v1/chat/completions
```

**Expected Results**:
- P50 latency: < 5ms
- P99 latency: < 30ms
- Throughput: > 10,000 req/s
- Memory: < 100MB

### Scenario 2: Provider Failover

**Setup**:
- 3 provider credentials configured
- First credential returns 429 (rate limit)
- Second credential succeeds

**Test**:
```bash
# Simulate provider rate limit
curl -X POST http://localhost:4200/v1/internal/providers/test-provider/simulate-error \
  -H "Authorization: Bearer admin-token" \
  -d '{"error_type": "rate_limit", "credential_index": 0}'

# Run benchmark
wrk -t12 -c100 -d30s \
  -H "Authorization: Bearer test-key" \
  -s scripts/chat-completion.lua \
  http://localhost:4200/v1/chat/completions
```

**Expected Results**:
- Failover latency: < 50ms (includes retry)
- Success rate: > 99%
- No memory leaks

### Scenario 3: Cache Miss (Cold Start)

**Setup**:
- Empty Redis cache
- 1000 credentials in PostgreSQL
- 100 concurrent clients

**Test**:
```bash
# Flush Redis cache
redis-cli FLUSHALL

# Run benchmark
wrk -t12 -c100 -d30s \
  -H "Authorization: Bearer gw-user-test-credential" \
  -s scripts/verify-credential.lua \
  http://localhost:4200/v1/chat/completions
```

**Expected Results**:
- First request latency: < 50ms (includes DB query)
- Subsequent requests: < 5ms (cache hit)
- Cache population rate: > 1000 credentials/second

### Scenario 4: Streaming SSE

**Setup**:
- 1000 concurrent SSE connections
- Each connection receives 100 chunks
- Total duration: 30 seconds

**Test**:
```bash
./target/release/stream-bench \
  --url http://localhost:4200/v1/chat/completions \
  --auth "Bearer test-key" \
  --concurrent 1000 \
  --duration 30s \
  --stream true
```

**Expected Results**:
- Concurrent connections: > 1000
- TTFT (Time To First Token): < 100ms
- Chunk latency: < 10ms
- Memory per connection: < 100KB

## Continuous Benchmarking

### CI/CD Integration

Add performance regression tests to CI:

```yaml
# .github/workflows/benchmark.yml
name: Performance Benchmarks

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build gateway
        run: cd gateway && cargo build --release
      
      - name: Start services
        run: |
          docker-compose up -d postgres redis
          sleep 10
      
      - name: Run benchmarks
        run: |
          cd gateway
          cargo bench --bench latency
          cargo bench --bench throughput
          cargo bench --bench memory
      
      - name: Check regression
        run: |
          # Fail if P99 latency > 50ms
          # Fail if throughput < 8000 req/s
          # Fail if memory > 600MB
          ./scripts/check-benchmark-regression.sh
```

### Benchmark Dashboard

Track performance over time:

```bash
# Store benchmark results
./scripts/store-benchmark-results.sh \
  --commit $(git rev-parse HEAD) \
  --p50 5.2 \
  --p99 28.1 \
  --throughput 12500 \
  --memory 85

# Generate dashboard
./scripts/generate-benchmark-dashboard.sh > docs/benchmark-dashboard.html
```

## Performance Optimization Checklist

When optimizing performance, check these areas:

### 1. Hot Path Optimization
- [ ] Minimize allocations in request path
- [ ] Use `&str` instead of `String` where possible
- [ ] Avoid cloning large structures
- [ ] Use `Arc` for shared immutable data
- [ ] Use `DashMap` instead of `Mutex<HashMap>`

### 2. Database Optimization
- [ ] Use connection pooling (20 max connections)
- [ ] Add indexes on frequently queried columns
- [ ] Use `EXPLAIN ANALYZE` to check query plans
- [ ] Batch operations where possible
- [ ] Use prepared statements

### 3. Redis Optimization
- [ ] Use pipelining for multiple commands
- [ ] Set appropriate TTLs
- [ ] Use Redis Cluster for horizontal scaling
- [ ] Monitor cache hit rate (target > 95%)

### 4. Network Optimization
- [ ] Use HTTP/2 for multiplexing
- [ ] Enable TCP_NODELAY
- [ ] Use connection pooling for upstream providers
- [ ] Implement request coalescing for duplicate requests

### 5. Concurrency Optimization
- [ ] Use `tokio::spawn` for CPU-bound tasks
- [ ] Avoid blocking operations in async context
- [ ] Use bounded channels to prevent memory growth
- [ ] Implement backpressure mechanisms

## Comparison with Other Gateways

For context, here are performance metrics from other AI gateways:

| Gateway | Language | P99 Latency | Throughput | Memory |
|---------|----------|-------------|------------|--------|
| **Rust Gateway (Target)** | Rust | < 30ms | > 10,000 req/s | < 100MB |
| TypeScript Gateway (Current) | TypeScript | ~300ms | ~2,000 req/s | ~200MB |
| Kong | C/Lua | ~50ms | ~5,000 req/s | ~150MB |
| Envoy | C++ | ~20ms | ~15,000 req/s | ~80MB |
| Nginx | C | ~10ms | ~20,000 req/s | ~50MB |

**Note**: These are approximate values and depend heavily on configuration and workload.

## Troubleshooting Performance Issues

### High Latency

1. **Check database query time**:
   ```sql
   SELECT query, mean_exec_time, calls
   FROM pg_stat_statements
   ORDER BY mean_exec_time DESC
   LIMIT 10;
   ```

2. **Check Redis latency**:
   ```bash
   redis-cli --latency
   ```

3. **Profile CPU usage**:
   ```bash
   cargo flamegraph --bin neuro-gateway
   ```

### Low Throughput

1. **Check connection pool exhaustion**:
   ```rust
   tracing::info!(
       pool_size = pool.size(),
       pool_idle = pool.num_idle(),
       "Database connection pool status"
   );
   ```

2. **Check for blocking operations**:
   ```bash
   # Use tokio-console for async debugging
   cargo install tokio-console
   RUSTFLAGS="--cfg tokio_unstable" cargo run
   ```

3. **Check for lock contention**:
   ```bash
   # Use perf to find lock contention
   perf record -g ./target/release/neuro-gateway
   perf report
   ```

### High Memory Usage

1. **Check for memory leaks**:
   ```bash
   heaptrack ./target/release/neuro-gateway
   heaptrack_print heaptrack.neuro-gateway.*.gz
   ```

2. **Check connection pool size**:
   ```rust
   // Reduce max connections if memory is high
   PgPoolOptions::new()
       .max_connections(10)  // Reduce from 20
   ```

3. **Check cache size**:
   ```bash
   redis-cli INFO memory
   ```

## Acceptance Criteria

The Rust gateway is considered performant enough for production when:

1. **Latency**: P99 < 30ms for 95% of benchmark runs
2. **Throughput**: > 10,000 req/s sustained for 5 minutes
3. **Memory**: < 100MB resident under 1000 req/s load
4. **Stability**: No memory leaks after 24 hours of load testing
5. **Regression**: No more than 10% performance degradation between releases

---

**Last Updated**: 2026-04-13
**Owner**: Rust Gateway Team
