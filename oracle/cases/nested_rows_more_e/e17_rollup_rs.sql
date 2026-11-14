select l.rs from comments x cross join lateral (select array(select r from (select id from replies where comment_id = x.id) r) as rs) l group by rollup (l.rs)
