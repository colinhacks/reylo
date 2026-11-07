select p.title, c.body from one_comment(:x::int8) c join posts p on p.id = :id::int8
