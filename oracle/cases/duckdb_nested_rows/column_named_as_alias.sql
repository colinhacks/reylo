select array(select body from (select id, body from comments) body) as rows
