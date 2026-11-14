select p.id, author_of(p) as author, comment_count(p) as n from post p where p.id = :id::int8
