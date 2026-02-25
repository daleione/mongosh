# Mongosh Documentation

Welcome to the mongosh documentation. This guide covers the enhanced features and SQL capabilities of the MongoDB Shell.

## Table of Contents

### Getting Started
- [API Reference](./api-reference.md) - Comprehensive list of supported MongoDB methods and their status

### SQL Features

Mongosh provides SQL query capabilities on top of MongoDB, allowing you to use familiar SQL syntax alongside native MongoDB commands.

- [SQL Array Indexing](./array-indexing.md) - Access and slice array elements using SQL syntax
- [SQL Arithmetic Operations](./arithmetic-operations.md) - Perform calculations in SQL queries
- [SQL Date and Time Functions](./datetime-functions.md) - Work with dates and timestamps using standard SQL syntax
- [SQL Query Explain](./query-explain.md) - Analyze and optimize your queries with EXPLAIN

### Advanced Features
- [Named Queries](./named-queries.md) - Save and reuse frequently used queries
- [Shell Completion](./shell-completion.md) - Auto-complete commands and datasource names

## Quick Examples

### SQL Query
```sql
SELECT name, email FROM users WHERE age > 18 ORDER BY name LIMIT 10
```

### Array Access
```sql
SELECT tags[0] AS primary_tag FROM posts WHERE tags[-1] = 'featured'
```

### Date Filtering
```sql
SELECT * FROM orders WHERE created_at > DATE '2024-01-01'
```

### Named Query
```javascript
query save active_users db.users.find({status: 'active'})
query active_users
```

## SQL and MongoDB Compatibility

Mongosh seamlessly integrates SQL syntax with MongoDB's native commands. You can:
- Use SQL queries for read operations
- Fall back to MongoDB native syntax when needed
- Mix both syntaxes in your workflow

## Contributing

For implementation details and development documentation, see the project's main README.

## Need Help?

- Check the [API Reference](./api-reference.md) for method support status
- Review feature-specific guides for detailed usage instructions
- Refer to [MongoDB Documentation](https://docs.mongodb.com/) for core MongoDB concepts
