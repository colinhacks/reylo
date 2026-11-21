SELECT plan, count() AS n, sumIf(balance, age > 40) AS rich, anyLast(email) AS email FROM users GROUP BY plan ORDER BY plan
