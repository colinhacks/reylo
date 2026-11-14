insert into posts (id, title) values (:id, :title) returning id, title, published_at, status
