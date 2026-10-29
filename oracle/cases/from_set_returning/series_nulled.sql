select p.id, g from people p left join generate_series(1, p.n) g on g > 1
