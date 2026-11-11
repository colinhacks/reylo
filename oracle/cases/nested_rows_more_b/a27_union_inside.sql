select array(select c from (select id, body from comments union all select null, 'z') c) as arr
