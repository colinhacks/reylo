select array(select c from (select post_id, big, count(*) as n from comments group by cube (post_id, big)) c) as arr
