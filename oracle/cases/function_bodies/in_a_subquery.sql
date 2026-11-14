select s.id, s.body from (select id, body from comments_for(:p::int8)) s
