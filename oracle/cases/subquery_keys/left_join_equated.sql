select * from (select p.id, t.tag from posts p left join tags t on t.post_id = p.id) s where s.id = :id and s.tag = :tag
