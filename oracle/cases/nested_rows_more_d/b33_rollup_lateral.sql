select p.author_id, l.n from posts p cross join lateral (select p.author_id + 1 as n) l group by rollup (p.author_id, l.n)
