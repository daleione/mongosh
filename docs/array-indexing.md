# Array Indexing

This document describes how to access array elements and slices in SQL queries using mongosh.

## Overview

mongosh SQL support allows you to access array elements using bracket notation with indexes and slices, similar to Python array syntax. These operations are automatically translated to MongoDB aggregation pipeline stages.

## Basic Array Indexing

### Positive Index Access

Access array elements using zero-based indexing:

```sql
-- Get the first element
SELECT tags[0] FROM posts

-- Get the third element
SELECT items[2] FROM orders

-- Get a specific comment
SELECT comments[5] FROM articles
```

### Negative Index Access

Access array elements from the end using negative indexes:

```sql
-- Get the last element
SELECT tags[-1] FROM posts

-- Get the second-to-last element
SELECT history[-2] FROM users

-- Get the third from the end
SELECT scores[-3] FROM games
```

## Nested Field Array Access

### Accessing Arrays in Nested Fields

```sql
-- Access first role in user object
SELECT user.roles[0] FROM accounts

-- Access last address in profile
SELECT profile.addresses[-1] FROM customers

-- Access specific item in nested structure
SELECT order.items[0].name FROM transactions
```

### Multi-Level Nesting

```sql
-- Navigate through multiple levels
SELECT data.results[0].items[1] FROM responses

-- Combine field and array access
SELECT user.activity[0].events[-1] FROM analytics
```

## Array Slicing

### Basic Slicing Syntax

Array slicing uses the format `[start:end]` or `[start:end:step]`:

```sql
-- Get first 5 elements
SELECT tags[0:5] FROM posts

-- Get elements from index 2 to end
SELECT comments[2:] FROM articles

-- Get first 10 elements
SELECT items[:10] FROM orders

-- Get elements from index 3 to 8
SELECT data[3:8] FROM experiments
```

### Slicing with Step

```sql
-- Get every other element
SELECT values[0:10:2] FROM datasets

-- Get every third element
SELECT samples[::3] FROM measurements
```

### Negative Indexes in Slices

```sql
-- Get last 5 elements
SELECT reviews[-5:] FROM products

-- Get all but the last element
SELECT items[:-1] FROM lists

-- Get middle elements
SELECT data[2:-2] FROM collections
```

## Using Array Indexes in WHERE Clauses

Filter documents based on array element values:

```sql
-- Find posts where first tag is 'javascript'
SELECT * FROM posts WHERE tags[0] = 'javascript'

-- Find users with specific last activity
SELECT * FROM users WHERE activities[-1].type = 'login'

-- Compare array elements
SELECT * FROM scores WHERE grades[0] > 90
```

**Note:** Using array indexes in WHERE clauses automatically triggers aggregation pipeline execution.

## Using Array Indexes in ORDER BY

Sort results based on array elements:

```sql
-- Sort by first tag alphabetically
SELECT * FROM posts ORDER BY tags[0] ASC

-- Sort by highest score
SELECT * FROM reviews ORDER BY scores[-1] DESC

-- Sort by nested array element
SELECT * FROM users ORDER BY profile.languages[0] ASC
```

## Using Array Indexes in GROUP BY

**Note:** Array index expressions in GROUP BY clauses are currently not supported in this version.

## Practical Examples

### Example 1: E-commerce Product Selection

```sql
SELECT 
    product_name, 
    images[0] AS primary_image,
    images[1:4] AS gallery_images,
    reviews[-5:] AS recent_reviews
FROM products
WHERE category = 'electronics'
  AND ratings[0] >= 4.5
ORDER BY created_at DESC
LIMIT 20
```

### Example 2: User Activity Analysis

```sql
SELECT 
    user_id,
    username,
    login_history[-1] AS last_login,
    login_history[-30:] AS recent_logins
FROM users
WHERE login_history[-1].timestamp > DATE '2024-01-01'
  AND status = 'active'
```

### Example 3: Blog Post Tags

```sql
SELECT 
    title,
    author.name AS author_name,
    tags[0] AS primary_tag,
    tags[1:] AS additional_tags,
    comments[-1] AS latest_comment
FROM articles
WHERE published = true
  AND tags[0] IN ('technology', 'programming', 'web-development')
ORDER BY publish_date DESC
```

### Example 4: Time Series Data

```sql
SELECT 
    sensor_id,
    readings[0] AS first_reading,
    readings[-1] AS last_reading,
    readings[::10] AS sampled_readings
FROM sensor_data
WHERE timestamp >= TIMESTAMP '2024-01-01 00:00:00'
  AND readings[-1].value > 100
```

## MongoDB Aggregation Pipeline Translation

Array indexing operations are translated to MongoDB aggregation pipeline operators:

### Positive Index Translation

**SQL:**
```sql
SELECT tags[2] FROM posts
```

**MongoDB Aggregation Pipeline:**
```javascript
[
  {
    $project: {
      tags: { $arrayElemAt: ["$tags", 2] }
    }
  }
]
```

### Negative Index Translation

**SQL:**
```sql
SELECT tags[-1] FROM posts
```

**MongoDB Aggregation Pipeline:**
```javascript
[
  {
    $project: {
      tags: { $arrayElemAt: ["$tags", -1] }
    }
  }
]
```

### Array Slice Translation

**SQL:**
```sql
SELECT tags[1:5] FROM posts
```

**MongoDB Aggregation Pipeline:**
```javascript
[
  {
    $project: {
      tags: { $slice: ["$tags", 1, 4] }
    }
  }
]
```

## Error Handling

### Empty Index

```sql
SELECT tags[] FROM posts
```

**Error:** `Empty array index. Use arr[0] for first element or arr[-1] for last element.`

### Invalid Index Type

```sql
SELECT tags[abc] FROM posts
```

**Error:** `Invalid array index 'abc'. Index must be a number.`

### Missing Closing Bracket

```sql
SELECT tags[0 FROM posts
```

**Error:** `Missing closing bracket ']' for array access.`

### Out of Bounds Access

Array indexes that are out of bounds return `null` rather than throwing an error, consistent with MongoDB behavior:

```sql
-- If tags has only 3 elements
SELECT tags[10] FROM posts  -- Returns null
```

## Performance Considerations

1. **Simple Query Optimization**: Queries without array access use MongoDB's `find()` command for optimal performance.

2. **Aggregation Pipeline**: Array indexing and slicing trigger aggregation pipeline execution, which may be slightly slower than simple `find()` queries.

3. **Index Utilization**: When using array elements in WHERE clauses, MongoDB array indexes can still be utilized effectively.

4. **Memory Usage**: Array slicing operations that extract large portions of arrays may consume more memory in the aggregation pipeline.

## Current Limitations

1. **GROUP BY Restrictions**: Array index expressions are not supported in GROUP BY clauses.

2. **Step Limitations**: Array slice step values have limited support for complex stepping patterns.

3. **Nesting Complexity**: Extremely complex nested array access may require additional pipeline stages.

4. **Computed Indexes**: Dynamic or computed array indexes (e.g., `arr[x+1]`) are not supported.

## Best Practices

1. **Use Descriptive Aliases**: Always use meaningful aliases for array access results:
   ```sql
   SELECT tags[0] AS primary_tag, tags[1:] AS secondary_tags FROM posts
   ```

2. **Leverage Negative Indexes**: Use negative indexes when accessing elements from the end:
   ```sql
   SELECT comments[-3:] AS latest_comments FROM articles
   ```

3. **Test Performance**: For critical queries, compare performance with and without array access.

4. **Consider Document Design**: If you frequently access specific array elements, consider promoting them to top-level fields in your schema.

5. **Use Slicing Wisely**: When working with large arrays, use slicing to limit the amount of data transferred:
   ```sql
   SELECT data[:100] AS sample FROM large_collections
   ```

## Related Resources

- [MongoDB $arrayElemAt Operator](https://docs.mongodb.com/manual/reference/operator/aggregation/arrayElemAt/)
- [MongoDB $slice Operator](https://docs.mongodb.com/manual/reference/operator/aggregation/slice/)
- [Aggregation Pipeline Overview](https://docs.mongodb.com/manual/core/aggregation-pipeline/)
