select array(select c from (select x.id, (select sum(r.id) from replies r where r.comment_id = x.id) as s from comments x) c) as arr
