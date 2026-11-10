select array(select case when c.id > 1 then c end from (select id from comments) c) as els
