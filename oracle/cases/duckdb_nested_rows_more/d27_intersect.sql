select array(select c from (select id, big from comments intersect select 2, null) c) as arr
