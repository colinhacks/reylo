select array(select c from (select x.id, case when x.id > 1 then array(select r from (select id from replies where comment_id = x.id) r) end as rs from comments x) c) as arr
