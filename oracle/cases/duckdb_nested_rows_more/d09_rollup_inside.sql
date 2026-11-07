select array(select c from (select post_id, count(*) as k from comments group by rollup (post_id)) c) as arr
