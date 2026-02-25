# Query Execution Plans with EXPLAIN

## Overview

The `EXPLAIN` command allows you to analyze query execution plans and understand how MongoDB processes your queries. This feature is available in both SQL syntax and MongoDB native syntax, helping you optimize query performance and identify potential bottlenecks.

## Basic Usage

### SQL Syntax

```sql
EXPLAIN SELECT * FROM users;
```

### MongoDB Native Syntax

```javascript
// Method 1: Traditional approach
db.users.explain().find({ age: { $gt: 18 } });

// Method 2: Chained approach
db.users.find({ age: { $gt: 18 } }).explain();
```

All three syntaxes generate the same underlying MongoDB explain command and return equivalent results.

## Verbosity Levels

EXPLAIN supports three levels of detail:

### 1. queryPlanner (Default)

Returns the query plan selected by the query optimizer without executing the query.

```sql
EXPLAIN SELECT * FROM users WHERE age > 18;
```

```javascript
db.users.explain("queryPlanner").find({ age: { $gt: 18 } });
```

**Use when:** You want to understand which indexes will be used without running the query.

### 2. executionStats

Returns the query plan and execution statistics, including documents examined and execution time.

```sql
EXPLAIN executionStats SELECT * FROM users WHERE status = 'active';
```

```javascript
db.users.explain("executionStats").find({ status: "active" });
```

**Use when:** You need to measure actual query performance and resource usage.

### 3. allPlansExecution

Returns execution statistics for all candidate query plans considered by the optimizer.

```sql
EXPLAIN allPlansExecution SELECT * FROM orders WHERE customerId = 12345;
```

```javascript
db.orders.explain("allPlansExecution").find({ customerId: 12345 });
```

**Use when:** You want to compare multiple query plans and understand why a specific plan was chosen.

## Common Use Cases

### Analyzing Index Usage

Check if your query uses an index efficiently:

```sql
EXPLAIN executionStats 
SELECT name, email 
FROM users 
WHERE status = 'active' AND age > 25;
```

**Key metrics to check:**
- `executionStats.totalKeysExamined`: Number of index keys scanned
- `executionStats.totalDocsExamined`: Number of documents examined
- `winningPlan.stage`: Should show `IXSCAN` for indexed queries

### Optimizing Sort Operations

Identify if sorting requires an in-memory sort or uses an index:

```sql
EXPLAIN executionStats 
SELECT * 
FROM products 
ORDER BY price DESC 
LIMIT 20;
```

**Look for:**
- `SORT` stage: Indicates in-memory sorting (slower)
- `IXSCAN` stage: Sorting uses index (faster)

### Analyzing Aggregation Pipelines

```sql
EXPLAIN executionStats 
SELECT category, COUNT(*) as total, AVG(price) as avg_price
FROM products
WHERE inStock = true
GROUP BY category
HAVING total > 10;
```

```javascript
db.products.explain("executionStats").aggregate([
  { $match: { inStock: true } },
  { $group: { _id: "$category", total: { $sum: 1 }, avg_price: { $avg: "$price" } } },
  { $match: { total: { $gt: 10 } } }
]);
```

### Comparing Query Plans

Use `allPlansExecution` to see why the optimizer chose a specific plan:

```sql
EXPLAIN allPlansExecution
SELECT * 
FROM orders 
WHERE customerId = 12345 AND status IN ('pending', 'processing');
```

The output shows all candidate plans and their scores, helping you understand index selection.

## Understanding EXPLAIN Output

### Key Fields in Query Planner Output

```javascript
{
  "queryPlanner": {
    "winningPlan": {
      "stage": "IXSCAN",           // Index scan (good)
      "indexName": "status_1_age_1", // Index being used
      "direction": "forward"
    },
    "rejectedPlans": []             // Alternative plans considered
  }
}
```

### Key Fields in Execution Stats

```javascript
{
  "executionStats": {
    "executionTimeMillis": 15,      // Total execution time
    "totalKeysExamined": 100,       // Index keys scanned
    "totalDocsExamined": 100,       // Documents examined
    "nReturned": 50,                // Documents returned
    "executionStages": {
      "stage": "FETCH",
      "nReturned": 50,
      "docsExamined": 100
    }
  }
}
```

### Common Execution Stages

| Stage | Description | Performance |
|-------|-------------|-------------|
| `COLLSCAN` | Full collection scan | ⚠️ Slow for large collections |
| `IXSCAN` | Index scan | ✅ Fast |
| `FETCH` | Retrieve documents after index scan | ✅ Normal |
| `SORT` | In-memory sort | ⚠️ Limited by memory |
| `PROJECTION` | Field projection | ✅ Fast |
| `LIMIT` | Limit results | ✅ Fast |
| `SKIP` | Skip results | ⚠️ Still scans skipped docs |

## Performance Optimization Tips

### 1. Aim for Index-Only Queries

Best case: `totalKeysExamined` ≈ `nReturned`

```sql
-- Create covering index
CREATE INDEX idx_user_status_name ON users(status, name);

-- Query that can use index-only scan
EXPLAIN executionStats 
SELECT name 
FROM users 
WHERE status = 'active';
```

### 2. Avoid Collection Scans

If you see `COLLSCAN` frequently:

```sql
-- Before: Collection scan
EXPLAIN SELECT * FROM orders WHERE customerId = 12345;
-- Stage: COLLSCAN

-- Create index
CREATE INDEX idx_customer ON orders(customerId);

-- After: Index scan
EXPLAIN SELECT * FROM orders WHERE customerId = 12345;
-- Stage: IXSCAN
```

### 3. Optimize Compound Indexes

Order matters in compound indexes:

```sql
-- Index: (status, createdAt)
-- Good: Uses entire index
SELECT * FROM orders WHERE status = 'pending' AND createdAt > '2024-01-01';

-- Partial: Only uses status
SELECT * FROM orders WHERE status = 'pending';

-- Bad: Cannot use index efficiently
SELECT * FROM orders WHERE createdAt > '2024-01-01';
```

### 4. Use Projection to Reduce Data Transfer

```sql
-- More efficient: Only fetch needed fields
EXPLAIN executionStats 
SELECT name, email 
FROM users 
WHERE status = 'active';

-- Less efficient: Fetches all fields
EXPLAIN executionStats 
SELECT * 
FROM users 
WHERE status = 'active';
```

## Advanced Examples

### Chained Operations with MongoDB Syntax

```javascript
// Complex query with multiple operations
db.users
  .find({ status: "active" })
  .sort({ lastLoginAt: -1 })
  .limit(100)
  .skip(50)
  .explain("executionStats");
```

### Aggregation with EXPLAIN

```javascript
db.sales
  .aggregate([
    { $match: { date: { $gte: ISODate("2024-01-01") } } },
    { $group: { _id: "$product", total: { $sum: "$amount" } } },
    { $sort: { total: -1 } },
    { $limit: 10 }
  ])
  .explain("executionStats");
```

### Explain with Hint

Force a specific index and compare performance:

```javascript
// Use default index selection
db.orders.explain("executionStats").find({ customerId: 12345, status: "pending" });

// Force specific index
db.orders.explain("executionStats").find({ customerId: 12345, status: "pending" }).hint({ customerId: 1 });
```

## Troubleshooting Common Issues

### Issue: High `totalDocsExamined` vs `nReturned`

**Problem:** Query examines many more documents than it returns.

**Solution:**
```sql
-- Check current performance
EXPLAIN executionStats SELECT * FROM users WHERE age > 18 AND status = 'active';

-- Create compound index to improve selectivity
CREATE INDEX idx_status_age ON users(status, age);
```

### Issue: `SORT_KEY_GENERATOR` Stage Present

**Problem:** Sort operation requires in-memory sorting.

**Solution:**
```sql
-- Create index that supports both filter and sort
CREATE INDEX idx_status_createdAt ON orders(status, createdAt);

-- Now this query can use index for both
SELECT * FROM orders WHERE status = 'pending' ORDER BY createdAt DESC;
```

### Issue: Query Takes Too Long

**Steps to diagnose:**

1. Check execution time:
   ```sql
   EXPLAIN executionStats SELECT * FROM large_collection WHERE field = 'value';
   ```

2. Look at `executionTimeMillis` in output

3. Check for:
   - `COLLSCAN` → Need index
   - High `totalDocsExamined` → Index not selective enough
   - `SORT` stage → Need index for sorting

## Best Practices

1. **Always test with production-like data**: Query performance varies significantly with data size and distribution.

2. **Start with `queryPlanner`**: Use this for quick checks without executing the query.

3. **Use `executionStats` for optimization**: Get real performance metrics when tuning queries.

4. **Compare before and after**: Run EXPLAIN before and after creating indexes to measure improvement.

5. **Monitor in production**: Use MongoDB's profiling tools alongside EXPLAIN for production optimization.

6. **Consider data growth**: A query that performs well with 1,000 documents might fail with 1,000,000.

## Limitations

- `EXPLAIN` does not execute write operations (INSERT, UPDATE, DELETE)
- Some operations cannot be fully simulated without execution
- Performance can vary based on server load and cache state
- `allPlansExecution` executes trial runs which may take significant time

## Related Resources

- [MongoDB EXPLAIN Documentation](https://docs.mongodb.com/manual/reference/method/db.collection.explain/)
- [Query Performance Optimization](https://docs.mongodb.com/manual/tutorial/optimize-query-performance-with-indexes-and-projections/)
- [Analyze Query Performance](https://docs.mongodb.com/manual/tutorial/analyze-query-plan/)
- [Index Strategies](https://docs.mongodb.com/manual/applications/indexes/)
