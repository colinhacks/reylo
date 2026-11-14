with made as (insert into published (id, title) values (:id, :title) returning published_at, status) select published_at, status from made
