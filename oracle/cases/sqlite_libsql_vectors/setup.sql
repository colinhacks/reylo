create table docs (id integer primary key, title text not null, emb F32_BLOB(3));
create index docs_emb on docs (libsql_vector_idx(emb));
create table words (k text primary key, e F32_BLOB(2)) without rowid;
create index words_e on words (libsql_vector_idx(e, 'metric=l2'));
create table wide (id integer primary key, e F64_BLOB(2));
create index wide_e on wide (libsql_vector_idx(e));
