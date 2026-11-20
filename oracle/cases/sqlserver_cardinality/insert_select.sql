insert into customers (id, email) output inserted.id select id + 1000, concat(email, '.copy') from customers
