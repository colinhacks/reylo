select case when false then array(select c from (select id from comments) c) end as arr
