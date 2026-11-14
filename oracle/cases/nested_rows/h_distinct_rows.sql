select distinct array(select c from (select id from comments) c) as rows from posts
