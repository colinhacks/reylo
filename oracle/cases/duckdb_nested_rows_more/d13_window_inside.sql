select array(select c from (select id, lag(big) over (order by id) as prev from comments) c) as arr
