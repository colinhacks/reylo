create table orders (id integer primary key autoincrement not null, sku text not null, batch_id integer not null, label text, created_at integer default (unixepoch()) not null, delta integer default -5, raw blob default X'00ff');
create unique index orders_batch_id_unique on orders (batch_id);
create index orders_sku_idx on orders (sku);
create unique index orders_sku_unique on orders (sku);
