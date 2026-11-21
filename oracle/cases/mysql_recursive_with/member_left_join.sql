with recursive r (n, name) as (select 1, cast('start' as char(255)) union all select r.n + 1, u.name from r left join users u on u.id = r.n + 5 where r.n < 3) select n, name from r
