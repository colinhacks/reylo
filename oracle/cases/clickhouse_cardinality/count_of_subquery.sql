SELECT count() AS groups, max(n) AS largest FROM (SELECT plan, count() AS n FROM users GROUP BY plan)
