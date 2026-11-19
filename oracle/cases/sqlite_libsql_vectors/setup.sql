create table docs (id integer primary key, title text not null, emb F32_BLOB(3));
create index docs_emb on docs (libsql_vector_idx(emb));
create table words (k text primary key, e F32_BLOB(2)) without rowid;
create index words_e on words (libsql_vector_idx(e, 'metric=l2'));
create table wide (id integer primary key, e F64_BLOB(2));
create index wide_e on wide (libsql_vector_idx(e));
-- A key of a WITHOUT ROWID table outside a STRICT one holds a BLOB as well as text.
insert into words values (x'6162', vector('[1,2]'));
