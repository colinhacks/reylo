-- `coalesce(x, …)` of JSON a statement builds keeps `x`'s shape only where `x` is never NULL or
-- an array is defaulted to '[]'; under any other default the value may be that default.
create table users (id bigint primary key, name text not null, meta jsonb);
create table posts (id bigint primary key, user_id bigint not null references users, title text not null);
