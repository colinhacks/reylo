insert into users (id, email, age) values (:id::int, :email::text, :age::int)
on conflict (id) do update set email = excluded.email
