select array(select c from (select x.id, case when x.id > 1 then array(select r from replies r where r.comment_id = x.id) end as rs from comments x) c) as arr
