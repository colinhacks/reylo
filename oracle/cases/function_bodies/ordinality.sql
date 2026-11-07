select id, body, n from comments_for(:p::int8) with ordinality as c (id, body, n)
