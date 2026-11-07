select array(select c from (select id, body from comments intersect select 2, null) c) as arr
