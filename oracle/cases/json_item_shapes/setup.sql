-- A JSON column read straight from a WITH query or a derived table is the JSON that item's own
-- statement builds, as nullable as the item's result and the joins around it make it.
create table users (id bigint primary key, name text not null);
create table posts (id bigint primary key, user_id bigint not null references users, title text not null, published_at timestamptz);
-- JSON other than its null, so that a value of it in a document is never a null there.
create domain doc_value as jsonb check (value <> 'null'::jsonb);
