with s as (select id, body from comments) select array(select s from s) as rows
