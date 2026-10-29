insert into accounts (id, status, tier) values (:id, :status::text, :tier::varchar(3)) returning id
