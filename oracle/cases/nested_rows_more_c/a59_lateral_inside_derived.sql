select array(select c from (select x.id, l.s from comments x cross join lateral (select sum(r.id) as s from replies r where r.comment_id = x.id) l) c) as arr
