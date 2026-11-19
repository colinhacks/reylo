SELECT plan, count() AS n FROM users GROUP BY plan WITH TOTALS ORDER BY plan
