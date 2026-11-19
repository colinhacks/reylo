create table users (id bigint not null primary key, name text not null default (cast('foo' as char charset ascii)), data blob default (cast('foo' as binary)));
create table tokens (id binary(16) not null default (unhex(replace(uuid(), '-', ''))), primary key (id));
