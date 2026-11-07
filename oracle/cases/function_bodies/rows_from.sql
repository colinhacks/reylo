select id, body from rows from (comments_for(:p::int8)) as c (id, body)
