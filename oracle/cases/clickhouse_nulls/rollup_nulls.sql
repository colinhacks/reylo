SELECT name, count() AS n FROM users GROUP BY name WITH ROLLUP ORDER BY n SETTINGS group_by_use_nulls = 1
