select p.id, array(select x from (select c.id, array(select r from (select note from replies where comment_id = c.id) r) as rs from comments c where c.post_id = p.id) x) as cs from posts p
