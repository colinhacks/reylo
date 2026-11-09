select array(select c from (select array(select r from (select id from replies) r) as rs union all select null) c) as arr
