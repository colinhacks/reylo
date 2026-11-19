SELECT plan, role, count() AS n FROM users GROUP BY ROLLUP(plan, role) ORDER BY plan, role, n
