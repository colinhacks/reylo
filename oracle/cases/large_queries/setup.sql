-- Statements of a hundred lines over a small shop: WITH queries over WITH queries, scalar
-- subqueries building JSON beside a window function, a table read again in a subquery beside an
-- outer join. Each is typed as precisely as the same constructs in a short statement, every probe
-- it needs being within the analysis's budget, and a probe that reads a WITH query carrying the
-- statement's WITH clause, placeholders and all. A chain of WITH queries past the budget
-- (`budget_spent.sql`) is left nullable where its probes stop, never typed wrong.
create type user_role as enum ('admin', 'member');
create type org_plan as enum ('free', 'pro', 'enterprise');
create type order_status as enum ('pending', 'paid', 'shipped', 'refunded', 'cancelled');
create table orgs (id serial primary key, name text not null, plan org_plan not null, created_at timestamptz not null default now());
create table users (
  id serial primary key,
  name text not null,
  email text unique,
  role user_role not null default 'member',
  created_at timestamptz not null default now(),
  org_id integer references orgs (id)
);
create table products (
  id serial primary key,
  org_id integer not null references orgs (id),
  sku text not null unique,
  name text not null,
  price_cents integer not null,
  tags text[] not null default '{}'
);
create table orders (
  id serial primary key,
  user_id integer not null references users (id),
  org_id integer not null references orgs (id),
  status order_status not null default 'pending',
  placed_at timestamptz not null default now(),
  shipped_at timestamptz
);
create table order_items (
  order_id integer not null references orders (id),
  product_id integer not null references products (id),
  quantity integer not null check (quantity > 0),
  unit_price_cents integer not null,
  primary key (order_id, product_id)
);
create table reviews (
  id serial primary key,
  product_id integer not null references products (id),
  user_id integer not null references users (id),
  rating smallint not null check (rating between 1 and 5),
  body text,
  created_at timestamptz not null default now()
);
