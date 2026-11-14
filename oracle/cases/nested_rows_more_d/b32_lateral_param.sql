select l.s from posts p cross join lateral (select p.id + :k::bigint as s) l
