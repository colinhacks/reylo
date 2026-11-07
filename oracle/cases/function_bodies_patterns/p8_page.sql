select x.id, x.title, author_of(x) as author, tags_of(x) as tags from search_posts(q => :q::text) x order by x.id limit 2 offset :skip::int8
