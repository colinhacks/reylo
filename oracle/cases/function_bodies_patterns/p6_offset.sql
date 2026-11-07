select x.id, x.title from search_posts() x order by x.id limit :size::int8 offset :skip::int8
