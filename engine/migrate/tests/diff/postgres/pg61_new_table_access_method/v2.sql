create access method heap2 type table handler heap_tableam_handler;
create table users (id bigint primary key);
create table events (id bigint primary key, payload text) using heap2;
