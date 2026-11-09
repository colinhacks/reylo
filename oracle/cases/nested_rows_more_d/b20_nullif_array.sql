select nullif(array(select c from (select id from comments where false) c), '{}') as arr
