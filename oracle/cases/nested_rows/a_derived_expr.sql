select p.id, array(select c from (select id, big + 1 as more, upper(body) as loud, count(*) over () as n from comments where post_id = p.id) c) as comments from posts p
