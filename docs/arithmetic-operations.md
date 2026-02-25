# Arithmetic Operations

## Overview

mongosh supports arithmetic expressions in SQL queries, enabling calculations directly in your queries. All computations are executed on the MongoDB server using the aggregation pipeline, ensuring efficient processing of large datasets.

## Supported Operators

| Operator | Description | Example               |
|----------|-------------|-----------------------|
| `+`      | Addition    | `price + tax`         |
| `-`      | Subtraction | `total - discount`    |
| `*`      | Multiplication | `price * quantity` |
| `/`      | Division    | `total / count`       |
| `%`      | Modulo      | `id % 2`              |

## Supported Mathematical Functions

| Function       | Description                    | Example                  |
|----------------|--------------------------------|--------------------------|
| `ROUND(x, n)`  | Round to n decimal places      | `ROUND(price * 1.13, 2)` |
| `ABS(x)`       | Absolute value                 | `ABS(balance)`           |
| `CEIL(x)`      | Round up to nearest integer    | `CEIL(rating)`           |
| `FLOOR(x)`     | Round down to nearest integer  | `FLOOR(score)`           |
| `TRUNC(x, n)`  | Truncate to n decimal places   | `TRUNC(value, 2)`        |

## Usage Examples

### Arithmetic in WHERE Clause

Filter results using arithmetic expressions:

```sql
-- Find orders where total exceeds $100
SELECT * FROM orders WHERE price * quantity > 100

-- Calculate price with tax
SELECT * FROM products WHERE price * 1.13 > 50

-- Complex conditions with parentheses
SELECT * FROM orders WHERE (price + shipping) * quantity > 1000

-- Filter even-numbered IDs using modulo
SELECT * FROM items WHERE id % 2 = 0

-- Combine with mathematical functions
SELECT * FROM products WHERE ROUND(price * 1.13, 2) > 50
```

### Arithmetic in SELECT Clause

Calculate new values in query results:

```sql
-- Calculate order total
SELECT product_name, price * quantity AS total FROM orders

-- Apply discount
SELECT name, price * 0.85 AS sale_price FROM products

-- Multiple calculated fields
SELECT
    name,
    price * quantity AS subtotal,
    price * quantity * 0.13 AS tax,
    price * quantity * 1.13 AS total
FROM orders

-- Using mathematical functions
SELECT name, ROUND(price * 1.13, 2) AS price_with_tax FROM products
```

### Complex Expressions

Combine multiple operators and functions:

```sql
-- Nested parentheses
SELECT * FROM orders WHERE ((price * quantity) - discount) * 1.13 > 500

-- Multiple arithmetic conditions
SELECT * FROM products
WHERE price * quantity > 100
  AND price + shipping < 50

-- Calculate profit margin
SELECT
    product_name,
    revenue - cost AS profit,
    ROUND((revenue - cost) / revenue * 100, 2) AS margin_percent
FROM sales
WHERE revenue > 0
```

## Operator Precedence

Arithmetic operations follow standard mathematical precedence rules:

1. **Higher precedence**: `*` `/` `%` (multiplication, division, modulo)
2. **Lower precedence**: `+` `-` (addition, subtraction)

Use parentheses `()` to override the default order:

```sql
-- Multiplication before addition
SELECT * FROM t WHERE price + tax * quantity > 100
-- Equivalent to: price + (tax * quantity) > 100

-- Force addition first with parentheses
SELECT * FROM t WHERE (price + tax) * quantity > 100
-- Calculates (price + tax) first, then multiplies by quantity
```

## High-Precision Decimal Numbers (Decimal128)

For financial calculations and scenarios requiring high precision, use MongoDB's `NumberDecimal` type:

```javascript
// Insert high-precision values
db.products.insertOne({
  name: "Premium Widget",
  price: NumberDecimal("19.99"),
  tax_rate: NumberDecimal("0.13")
});

// Query with high precision
db.products.find({ price: NumberDecimal("19.99") });
```

**Precision Comparison:**

| Type            | Precision              | Use Case                    |
|-----------------|------------------------|-----------------------------|
| `Double`        | ~15 significant digits | Scientific calculations     |
| `NumberDecimal` | 34 significant digits  | Financial data, currency    |

## Important Notes

1. **Automatic Aggregation Pipeline**: Queries containing arithmetic expressions are automatically converted to MongoDB aggregation pipelines
2. **Field References**: Arithmetic expressions can reference document fields (e.g., `price`, `quantity`)
3. **Literals**: Numeric literals are supported (e.g., `1.13`, `100`)
4. **Aliases**: Use `AS` keyword to name calculated fields in SELECT clause
5. **NULL Handling**: If any operand is NULL, the result is NULL

## Common Use Cases

### E-commerce Order Calculations

```sql
SELECT
    order_id,
    price * quantity AS subtotal,
    price * quantity * 0.08 AS sales_tax,
    price * quantity * 1.08 AS total
FROM orders
WHERE price * quantity > 100
ORDER BY total DESC
```

### Grade Statistics

```sql
SELECT
    student_name,
    (math + english + science) / 3 AS average_grade
FROM scores
WHERE math + english + science > 240
```

### Inventory Alerts

```sql
SELECT product_name, stock_count
FROM inventory
WHERE stock_count < min_stock * 1.2
ORDER BY stock_count ASC
```

### Price Range Filtering

```sql
SELECT * FROM products
WHERE price * (1 - discount_rate) BETWEEN 10 AND 50
ORDER BY price ASC
```

### Revenue Analysis

```sql
SELECT
    product_category,
    SUM(price * quantity) AS total_revenue,
    ROUND(AVG(price * quantity), 2) AS avg_order_value
FROM orders
WHERE order_date > DATE '2024-01-01'
GROUP BY product_category
HAVING SUM(price * quantity) > 10000
ORDER BY total_revenue DESC
```

## Performance Considerations

- Arithmetic operations in WHERE clauses may prevent index usage
- For frequently calculated values, consider storing them as fields in your documents
- Use aggregation pipeline indexes when available for optimized performance
- Test query performance with EXPLAIN to understand execution plans

## Best Practices

1. **Use Parentheses for Clarity**: Make complex expressions easier to read
   ```sql
   SELECT * FROM orders WHERE (price + tax) * quantity > total_budget
   ```

2. **Alias Calculated Fields**: Give meaningful names to computed values
   ```sql
   SELECT price * quantity AS line_total FROM orders
   ```

3. **Consider Decimal128 for Money**: Avoid floating-point precision issues
   ```javascript
   price: NumberDecimal("19.99")
   ```

4. **Store Frequently Calculated Values**: If a calculation is used often, pre-compute and store it
   ```javascript
   { price: 19.99, quantity: 5, total: 99.95 }  // Store total
   ```

5. **Test with EXPLAIN**: Verify query performance before deploying
   ```sql
   EXPLAIN SELECT * FROM orders WHERE price * quantity > 100
   ```

## See Also

- [Query EXPLAIN](./query-explain.md) - Analyze query performance
- [Date and Time Functions](./datetime-functions.md) - Working with dates
- [Array Indexing](./array-indexing.md) - Accessing array elements
