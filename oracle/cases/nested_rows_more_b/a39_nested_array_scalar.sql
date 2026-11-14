select array(select c from (select x.id, (select array(select r from (select id from replies where comment_id = x.id) r) where false) as rs from comments x) c) as arr
