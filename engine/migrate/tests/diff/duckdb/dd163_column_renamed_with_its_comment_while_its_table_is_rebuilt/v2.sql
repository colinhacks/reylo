create table t (id integer primary key, remark varchar, n integer check (n > 0)); comment on column t.remark is 'free text';
