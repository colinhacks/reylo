select array(select c from (select id, lead(big) over (order by id) as nxt from comments qualify id > 1) c) as arr
