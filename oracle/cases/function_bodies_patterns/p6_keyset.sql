select x.id, x.title, x.published_at from search_posts() x where (x.published_at, x.id) < (:at::timestamptz, :id::int8) order by x.published_at desc, x.id desc limit 2
