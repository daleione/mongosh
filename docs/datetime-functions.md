# DateTime Functions and Literals

## Overview

mongosh SQL mode supports standard SQL datetime syntax, providing a more intuitive way to work with dates and times in your queries. This feature bridges the gap between traditional SQL databases and MongoDB, making it easier for SQL users to work with temporal data.

## Date and Time Literals

### DATE Literals

Use the `DATE` keyword to define date values in standard SQL format.

```sql
-- Standard ISO format
SELECT * FROM orders WHERE order_date > DATE '2024-01-15'

-- Automatically sets time to 00:00:00 UTC
-- Equivalent to: TIMESTAMP '2024-01-15T00:00:00Z'
```

### TIMESTAMP Literals

Use the `TIMESTAMP` keyword for precise date and time values.

```sql
-- Full ISO 8601 format (recommended)
SELECT * FROM events WHERE event_time > TIMESTAMP '2024-01-15T14:30:00Z'

-- Space-separated format (auto-converted to ISO)
SELECT * FROM logs WHERE created_at > TIMESTAMP '2024-01-15 14:30:00'

-- Without timezone (automatically adds UTC)
SELECT * FROM records WHERE updated_at > TIMESTAMP '2024-01-15T14:30:00'

-- Without milliseconds (auto-completes to .000)
SELECT * FROM tasks WHERE deadline < TIMESTAMP '2024-12-31T23:59:59Z'
```

### Supported Date Formats

| Format | Example | Description |
|--------|---------|-------------|
| ISO 8601 full | `2024-01-15T14:30:00.000Z` | Standard format, recommended |
| ISO 8601 no ms | `2024-01-15T14:30:00Z` | Auto-completes milliseconds to .000 |
| Space-separated | `2024-01-15 14:30:00` | Auto-converts to ISO, adds UTC |
| Date only | `2024-01-15` | Auto-adds time as 00:00:00 UTC |
| Slash-separated | `2024/01/15` | Auto-converts to standard format |

## Current Time Functions

### CURRENT_TIMESTAMP

Returns the current date and time.

```sql
-- Standard SQL syntax (no parentheses)
SELECT * FROM sessions WHERE last_active > CURRENT_TIMESTAMP

-- Find records created in the last hour
SELECT * FROM notifications 
WHERE created_at > CURRENT_TIMESTAMP - INTERVAL '1' HOUR
```

### CURRENT_DATE

Returns the current date with time set to 00:00:00.

```sql
-- Get today's orders
SELECT * FROM orders WHERE order_date = CURRENT_DATE

-- Get orders from the last 7 days
SELECT * FROM orders WHERE order_date >= CURRENT_DATE - INTERVAL '7' DAY
```

### NOW()

Alternative syntax for getting the current timestamp.

```sql
-- With parentheses (common extension syntax)
SELECT * FROM events WHERE event_time > NOW()

-- Without parentheses (also supported)
SELECT * FROM events WHERE event_time > NOW
```

## MongoDB Compatibility

The legacy `ISODate()` function continues to work for backward compatibility.

```sql
-- MongoDB native syntax
SELECT * FROM orders WHERE order_date > ISODate('2024-01-15T14:30:00.000Z')

-- Simplified format also supported
SELECT * FROM users WHERE registered_at > ISODate('2024-01-15')
```

## Usage Examples

### Basic Date Queries

```sql
-- Orders placed after a specific date
SELECT * FROM orders WHERE order_date > DATE '2024-01-01'

-- Users registered before a specific time
SELECT * FROM users WHERE registered_at < TIMESTAMP '2024-12-31 23:59:59'

-- Today's activity logs
SELECT * FROM activity_logs WHERE created_at > CURRENT_DATE
```

### Date Range Queries

```sql
-- Orders from January 2024
SELECT * FROM orders 
WHERE order_date >= DATE '2024-01-01' 
  AND order_date < DATE '2024-02-01'

-- Recent events (last 24 hours)
SELECT * FROM events 
WHERE event_time > CURRENT_TIMESTAMP - INTERVAL '24' HOUR
ORDER BY event_time DESC
```

### Aggregation with Dates

```sql
-- Count orders by date
SELECT DATE(order_date) AS date, COUNT(*) AS order_count
FROM orders 
WHERE order_date >= DATE '2024-01-01'
GROUP BY DATE(order_date)
ORDER BY date DESC

-- Today's order total
SELECT SUM(amount) AS daily_total
FROM orders 
WHERE order_date >= CURRENT_DATE
```

### Complex Date Conditions

```sql
-- Active subscriptions
SELECT * FROM subscriptions
WHERE start_date <= CURRENT_DATE
  AND (end_date IS NULL OR end_date >= CURRENT_DATE)
  AND status = 'active'

-- Overdue tasks
SELECT * FROM tasks
WHERE due_date < CURRENT_TIMESTAMP
  AND completed_at IS NULL
ORDER BY due_date ASC
```

## Timezone Handling

- All date and time values default to **UTC timezone**
- If no timezone is specified, the system automatically adds `Z` suffix (UTC)
- For clarity and consistency, explicitly specify timezone in production

```sql
-- Recommended: Explicitly specify UTC
TIMESTAMP '2024-01-15T14:30:00Z'

-- Also supported: System automatically adds Z
TIMESTAMP '2024-01-15T14:30:00'

-- Date only: Auto-completes to 00:00:00 UTC
DATE '2024-01-15'
```

## Comparison: MongoDB vs Standard SQL

| MongoDB Syntax | Standard SQL Syntax | Benefit |
|----------------|---------------------|---------|
| `ISODate('2024-01-15T14:30:00.000Z')` | `TIMESTAMP '2024-01-15 14:30:00'` | More concise |
| `ISODate('2024-01-15')` | `DATE '2024-01-15'` | Clearer semantics |
| `new Date()` | `CURRENT_TIMESTAMP` or `NOW()` | SQL standard |
| N/A | `CURRENT_DATE` | Easier date comparison |

## Performance Considerations

- Type literals are converted to MongoDB `DateTime` objects during parsing
- Performance is equivalent to using `ISODate()` function
- For indexed fields, use precise timestamps to leverage indexes effectively
- Avoid complex date calculations in WHERE clauses when possible

```sql
-- Good: Uses index efficiently
SELECT * FROM orders 
WHERE order_date >= DATE '2024-01-01'
  AND order_date < DATE '2024-02-01'

-- Less efficient: Date function prevents index usage
SELECT * FROM orders 
WHERE EXTRACT(MONTH FROM order_date) = 1
```

## Error Handling

### Invalid Date Format

```sql
-- ❌ Error: Cannot parse date
SELECT * FROM orders WHERE order_date > DATE 'not-a-date'

-- Error message:
-- Invalid date string 'not-a-date'. Expected ISO 8601 format 
-- (e.g., '2024-01-15T14:30:00Z', '2024-01-15 14:30:00', or '2024-01-15')
```

### Missing Quotes

```sql
-- ❌ Error: Date value must be quoted
SELECT * FROM orders WHERE order_date > DATE 2024-01-15

-- ✅ Correct
SELECT * FROM orders WHERE order_date > DATE '2024-01-15'
```

### Invalid Timestamp

```sql
-- ❌ Error: Invalid time component
SELECT * FROM orders WHERE order_date > TIMESTAMP '2024-01-15 25:00:00'

-- Error message:
-- Invalid timestamp: Hour value 25 is out of range (0-23)
```

## Best Practices

1. **Use Type Literals**
   - Prefer `DATE '...'` and `TIMESTAMP '...'` over `ISODate()`
   - More aligned with SQL standards
   - Better for SQL-to-MongoDB migrations

2. **Specify Timezones**
   - Always use `Z` suffix for UTC in production
   - Document timezone conventions in your team
   - Consider storing all times in UTC

3. **Use CURRENT_TIMESTAMP**
   - Use `CURRENT_TIMESTAMP` instead of constructing `new Date()`
   - More portable across databases
   - Clearer intent in queries

4. **Maintain Format Consistency**
   - Standardize on one date format across your application
   - Recommended: ISO 8601 full format `YYYY-MM-DDTHH:MM:SS.sssZ`
   - Document your chosen format in coding standards

5. **Index Optimization**
   - Create indexes on frequently queried date fields
   - Use range queries to leverage indexes
   - Test query performance with `EXPLAIN`

```sql
-- Create index for better performance
db.orders.createIndex({ order_date: 1 })

-- Efficient range query using index
SELECT * FROM orders 
WHERE order_date >= DATE '2024-01-01'
  AND order_date < DATE '2024-02-01'
```

## Common Use Cases

### E-commerce Orders

```sql
-- Today's revenue
SELECT SUM(total_amount) AS daily_revenue
FROM orders 
WHERE order_date >= CURRENT_DATE
  AND status = 'completed'

-- Monthly sales report
SELECT 
    DATE_TRUNC('month', order_date) AS month,
    COUNT(*) AS order_count,
    SUM(total_amount) AS revenue
FROM orders 
WHERE order_date >= DATE '2024-01-01'
GROUP BY DATE_TRUNC('month', order_date)
ORDER BY month DESC
```

### User Activity Tracking

```sql
-- Active users in the last 30 days
SELECT COUNT(DISTINCT user_id) AS active_users
FROM user_activity 
WHERE last_seen >= CURRENT_DATE - INTERVAL '30' DAY

-- User registration trend
SELECT 
    DATE(registered_at) AS date,
    COUNT(*) AS new_users
FROM users 
WHERE registered_at >= DATE '2024-01-01'
GROUP BY DATE(registered_at)
ORDER BY date DESC
```

### Subscription Management

```sql
-- Expiring subscriptions (next 7 days)
SELECT user_id, plan_name, end_date
FROM subscriptions 
WHERE end_date >= CURRENT_DATE
  AND end_date <= CURRENT_DATE + INTERVAL '7' DAY
  AND auto_renew = false
ORDER BY end_date ASC

-- Active premium subscribers
SELECT COUNT(*) AS premium_count
FROM subscriptions 
WHERE plan_type = 'premium'
  AND start_date <= CURRENT_DATE
  AND (end_date IS NULL OR end_date >= CURRENT_DATE)
```

## Future Enhancements

The following features are planned for future releases:

### INTERVAL Expressions

```sql
-- Calculate date ranges dynamically
SELECT * FROM orders 
WHERE order_date > CURRENT_TIMESTAMP - INTERVAL '7' DAY

SELECT * FROM events 
WHERE event_time BETWEEN 
    CURRENT_DATE - INTERVAL '1' MONTH 
    AND CURRENT_DATE
```

### Date Extraction Functions

```sql
-- Extract date components
SELECT 
    EXTRACT(YEAR FROM order_date) AS year,
    EXTRACT(MONTH FROM order_date) AS month,
    COUNT(*) AS order_count
FROM orders 
GROUP BY year, month

-- Date part functions
SELECT * FROM events 
WHERE YEAR(event_time) = 2024
  AND MONTH(event_time) = 1
```

### Timezone Conversion

```sql
-- Convert between timezones
SELECT * FROM events 
WHERE event_time AT TIME ZONE 'America/New_York' > '2024-01-01'

-- Display in different timezone
SELECT 
    event_name,
    event_time AT TIME ZONE 'Asia/Shanghai' AS beijing_time
FROM events
```

### Date Arithmetic

```sql
-- Add/subtract time units
SELECT * FROM reminders 
WHERE remind_at = due_date - INTERVAL '1' DAY

-- Calculate date differences
SELECT 
    order_id,
    DATE_DIFF('day', order_date, delivery_date) AS delivery_days
FROM orders
```

## Related Documentation

- [Query Explain](./query-explain.md) - Analyze date query performance
- [Array Indexing](./array-indexing.md) - Working with array fields
- [Arithmetic Operations](./arithmetic-operations.md) - Numeric calculations
- [MongoDB BSON DateTime Type](https://www.mongodb.com/docs/manual/reference/bson-types/#date)
- [ISO 8601 Format](https://en.wikipedia.org/wiki/ISO_8601)

## Reference

### Supported Functions

| Function | Description | Example |
|----------|-------------|---------|
| `DATE` | Create date literal | `DATE '2024-01-15'` |
| `TIMESTAMP` | Create timestamp literal | `TIMESTAMP '2024-01-15 14:30:00'` |
| `CURRENT_TIMESTAMP` | Current date and time | `CURRENT_TIMESTAMP` |
| `CURRENT_DATE` | Current date (time = 00:00:00) | `CURRENT_DATE` |
| `NOW()` | Current date and time | `NOW()` |
| `ISODate()` | MongoDB date (legacy) | `ISODate('2024-01-15')` |

### Date Format Patterns

| Pattern | Format | Example |
|---------|--------|---------|
| ISO 8601 | `YYYY-MM-DDTHH:MM:SS.sssZ` | `2024-01-15T14:30:00.123Z` |
| ISO No MS | `YYYY-MM-DDTHH:MM:SSZ` | `2024-01-15T14:30:00Z` |
| Space Sep | `YYYY-MM-DD HH:MM:SS` | `2024-01-15 14:30:00` |
| Date Only | `YYYY-MM-DD` | `2024-01-15` |
