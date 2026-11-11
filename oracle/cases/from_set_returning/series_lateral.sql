select p.name, g from people p, generate_series(1, p.n) g
