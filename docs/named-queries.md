# Named Queries

## Overview

Named Queries allow you to save frequently used queries with short, memorable names. This feature helps you create reusable query templates with optional parameter substitution, improving productivity and reducing repetitive typing.

## Commands

### List All Named Queries

Display all saved named queries:

```bash
query
```

Or explicitly:

```bash
query list
```

**Example Output:**

```
+-------------------+------------------------------------------------+
| Name              | Query                                          |
|-------------------|------------------------------------------------|
| active_users      | db.users.find({status: 'active'})              |
| user_by_email     | db.users.findOne({email: '$1'})                |
| orders_by_status  | db.orders.find({status: '$1'}).limit($2)       |
| recent_posts      | db.posts.find().sort({created_at: -1}).limit(10)|
+-------------------+------------------------------------------------+
```

### Execute a Named Query

Run a saved query by its name:

```bash
query <name> [args...]
```

**Examples:**

```bash
# Execute a query without parameters
> query active_users

# Execute a query with one parameter
> query user_by_email john@example.com

# Execute a query with multiple parameters
> query orders_by_status completed 50
```

### Save a Named Query

Save a new named query:

```bash
query save <name> <query>
```

**Examples:**

```bash
# Save a simple query
> query save active_users db.users.find({status: 'active'})

# Save a query with one parameter
> query save user_by_email db.users.findOne({email: '$1'})

# Save a query with multiple parameters
> query save user_age_range db.users.find({age: {$gte: $1, $lte: $2}})

# Save a complex aggregation query
> query save sales_summary db.orders.aggregate([{$match: {status: '$1'}}, {$group: {_id: '$product', total: {$sum: '$amount'}}}])
```

### Delete a Named Query

Delete an existing named query:

```bash
query delete <name>
```

**Example:**

```bash
> query delete active_users
active_users: Deleted
```

## Parameter Substitution

Named queries support shell-style parameter substitution, allowing you to create flexible, reusable query templates.

### Positional Parameters

Use `$1`, `$2`, `$3`, etc., to represent parameters that will be replaced when executing the query:

```bash
# Save query with positional parameters
> query save find_by_status db.orders.find({status: '$1'})

# Execute with parameter
> query find_by_status pending
# Executes: db.orders.find({status: 'pending'})
```

### Multiple Parameters

```bash
# Save query with multiple parameters
> query save price_range db.products.find({price: {$gte: $1, $lte: $2}})

# Execute with multiple parameters
> query price_range 10 50
# Executes: db.products.find({price: {$gte: 10, $lte: 50}})
```

### Parameter Types

Parameters are automatically converted to appropriate types:

```bash
# Numeric parameters
> query save top_users db.users.find().sort({score: -1}).limit($1)
> query top_users 10
# $1 is converted to number: limit(10)

# String parameters
> query save user_by_name db.users.findOne({name: '$1'})
> query user_by_name Alice
# $1 remains as string: {name: 'Alice'}

# Mixed parameters
> query save orders_filter db.orders.find({status: '$1', amount: {$gt: $2}})
> query orders_filter completed 100
# Executes: db.orders.find({status: 'completed', amount: {$gt: 100}})
```

## Common Use Cases

### User Management Queries

```bash
# Find users by status
> query save users_by_status db.users.find({status: '$1'})

# Find user by email
> query save user_by_email db.users.findOne({email: '$1'})

# Update user status
> query save update_user_status db.users.updateOne({_id: ObjectId('$1')}, {$set: {status: '$2'}})

# Execute
> query users_by_status active
> query user_by_email admin@example.com
> query update_user_status 507f1f77bcf86cd799439011 inactive
```

### Order Management Queries

```bash
# Find orders by date range
> query save orders_by_date db.orders.find({order_date: {$gte: ISODate('$1'), $lte: ISODate('$2')}})

# Find customer orders
> query save customer_orders db.orders.find({customer_id: '$1'}).sort({order_date: -1})

# Orders by status with limit
> query save recent_orders db.orders.find({status: '$1'}).sort({created_at: -1}).limit($2)

# Execute
> query orders_by_date 2024-01-01 2024-01-31
> query customer_orders CUST12345
> query recent_orders shipped 20
```

### Analytics Queries

```bash
# Product sales aggregation
> query save product_sales db.orders.aggregate([{$match: {status: 'completed'}}, {$group: {_id: '$product_id', total: {$sum: '$amount'}, count: {$sum: 1}}}, {$sort: {total: -1}}, {$limit: $1}])

# User activity by date
> query save daily_activity db.activity.aggregate([{$match: {date: {$gte: ISODate('$1')}}}, {$group: {_id: '$user_id', actions: {$sum: 1}}}])

# Execute
> query product_sales 10
> query daily_activity 2024-01-01
```

### Search Queries

```bash
# Text search
> query save search_posts db.posts.find({$text: {$search: '$1'}})

# Regex search
> query save search_users db.users.find({name: {$regex: '$1', $options: 'i'}})

# Execute
> query search_posts "mongodb tutorial"
> query search_users ^john
```

## Best Practices

### 1. Use Descriptive Names

Choose clear, descriptive names that indicate what the query does:

```bash
# Good
> query save active_premium_users db.users.find({status: 'active', tier: 'premium'})
> query save monthly_revenue db.orders.aggregate([...])

# Avoid
> query save q1 db.users.find({status: 'active', tier: 'premium'})
> query save temp db.orders.aggregate([...])
```

### 2. Document Complex Queries

For complex queries, save them with comments or create a documentation file:

```bash
# Save the query
> query save sales_by_region db.orders.aggregate([{$match: {date: {$gte: ISODate('$1')}}}, {$group: {_id: '$region', total: {$sum: '$amount'}}}])

# Keep documentation separately
# sales_by_region: Aggregates sales by region for a given start date
# Parameters: $1 = start date (ISO format)
```

### 3. Test Queries Before Saving

Always test your query with actual parameters before saving:

```bash
# Test first
> db.users.find({age: {$gte: 18, $lte: 65}})

# Then save if it works
> query save users_age_range db.users.find({age: {$gte: $1, $lte: $2}})
```

### 4. Keep Queries Focused

Save queries that do one thing well rather than overly complex multi-purpose queries:

```bash
# Good: Focused queries
> query save active_users db.users.find({status: 'active'})
> query save premium_users db.users.find({tier: 'premium'})

# Less ideal: Overly complex
> query save complex_filter db.users.find({$or: [{status: '$1'}, {tier: '$2'}], age: {$gte: $3}})
```

### 5. Use Consistent Naming Conventions

Adopt a naming convention and stick to it:

```bash
# Convention: resource_action or resource_by_field
> query save users_find_active db.users.find({status: 'active'})
> query save users_by_email db.users.findOne({email: '$1'})
> query save orders_by_customer db.orders.find({customer_id: '$1'})
> query save products_by_category db.products.find({category: '$1'})
```

## Storage and Persistence

Named queries are stored in:

- Configuration file: `~/.mongoshrc` or `~/.config/mongosh/config.toml`
- Persisted across sessions
- Can be backed up by copying the configuration file

## Limitations

1. **Parameter Escaping**: Special characters in parameters may need proper escaping
2. **No Nested Substitution**: Parameters cannot reference other parameters
3. **Sequential Execution**: Queries are executed in the order parameters are provided
4. **No Conditional Logic**: Named queries don't support if/else or loops

## Troubleshooting

### Query Not Found

```bash
> query nonexistent_query
Error: Named query 'nonexistent_query' not found
```

**Solution**: Check available queries with `query list`

### Parameter Count Mismatch

```bash
> query user_age_range 18
Error: Expected 2 parameters, got 1
```

**Solution**: Provide all required parameters in the correct order

### Invalid Query Syntax

```bash
> query save bad_query db.users.find({invalid syntax})
Error: Invalid query syntax
```

**Solution**: Test the query syntax before saving

## Advanced Examples

### Chained Operations

```bash
# Save a query with multiple operations
> query save top_active_users db.users.find({status: 'active'}).sort({score: -1}).limit($1).projection({name: 1, email: 1})

# Execute
> query top_active_users 5
```

### Aggregation Pipelines

```bash
# Save complex aggregation
> query save revenue_by_month db.orders.aggregate([{$match: {year: $1}}, {$group: {_id: {month: {$month: '$date'}}, revenue: {$sum: '$amount'}}}, {$sort: {_id: 1}}])

# Execute
> query revenue_by_month 2024
```

### Update Operations

```bash
# Bulk update with parameters
> query save update_prices db.products.updateMany({category: '$1'}, {$mul: {price: $2}})

# Execute: Increase prices by 10% for electronics
> query update_prices electronics 1.1
```

## See Also

- [Shell Completion](./shell-completion.md) - Auto-complete datasource names
- [Query EXPLAIN](./query-explain.md) - Analyze query performance
- [API Reference](./api-reference.md) - Available MongoDB methods
