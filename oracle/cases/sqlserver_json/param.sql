select (select p.id from posts p where p.score > @min for json path) as posts
