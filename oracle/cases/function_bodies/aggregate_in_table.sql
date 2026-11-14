select c.n from pairs x cross join lateral counted_table(x.a) c
