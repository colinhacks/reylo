create table orders (id integer primary key, amount integer not null);
create function b_fee() returns integer language plpgsql stable as $$ begin return 1; end $$;
create function c_base() returns integer language sql stable as $$ select coalesce(sum(amount), 0)::integer from orders $$;
create function a_total() returns integer language sql stable as $$ select c_base() + b_fee() $$;
