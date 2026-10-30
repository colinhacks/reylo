select * from (select tag from tags where post_id = :p) s where s.tag = :t
