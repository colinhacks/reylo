insert into public.users (id, name, email, score) values (:id, :name, :email, :score) on conflict (id) do nothing
