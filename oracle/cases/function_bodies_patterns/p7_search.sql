select x.id, x.title from search_posts(tag_name => :tag::text, since => :since::timestamptz) x
