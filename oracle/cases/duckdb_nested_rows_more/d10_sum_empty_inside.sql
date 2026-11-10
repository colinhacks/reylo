select array(select c from (select sum(big) as s, max(id) as m from comments where false) c) as arr
