delete from orders where note is not null and user_id > 0 returning *
