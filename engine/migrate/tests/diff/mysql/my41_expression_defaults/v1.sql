create table users (id bigint not null primary key, name text not null default (cast('foo' as char charset latin1)));
