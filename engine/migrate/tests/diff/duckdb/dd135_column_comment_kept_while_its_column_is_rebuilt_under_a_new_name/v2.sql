create table t (id integer primary key, amount integer check (amount > 0)); comment on column t.amount is 'how many';
