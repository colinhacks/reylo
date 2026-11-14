insert into published (id, title) values (:id, :title) returning id, title, published_at, status
