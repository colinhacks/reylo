select p.id, array(select c from (select x.id, array(select r from replies r where r.comment_id = x.id) as replies from comments x where x.post_id = p.id) c) as comments from posts p
