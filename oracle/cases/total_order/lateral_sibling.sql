select p.id from posts p, (select random() - 0.5 as r) x, lateral (select p.score * x.r as n) t order by t.n, p.id
