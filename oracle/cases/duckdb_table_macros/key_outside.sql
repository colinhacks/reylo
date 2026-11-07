select id, body from comments_for(:p::int8) where id = :id::int8
