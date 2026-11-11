select id, note from comments_of_post(:p::int8) where id = :id::int8
